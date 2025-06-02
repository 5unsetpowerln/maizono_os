#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(ascii_char)]
#![feature(ascii_char_variants)]
#![feature(ptr_as_ref_unchecked)]

extern crate alloc;

mod acpi;
mod allocator;
mod device;
mod error;
mod frame_manager;
mod gdt;
mod graphic;
mod interrupts;
mod logger;
mod memory_map;
mod message;
mod mouse;
mod paging;
mod pci;
mod qemu;
mod serial;
mod timer;
mod types;
mod util;

use core::arch::asm;
use core::panic::PanicInfo;

use alloc::format;
use common::boot::BootInfo;
use common::graphic::rgb;
use device::ps2::{self, controller::ControllerError, mouse::MouseError};
use glam::u64vec2;
use graphic::{
    PixelWriter,
    console::{self},
    frame_buffer::{self},
};
use log::{debug, error, info};

use crate::graphic::{create_canvas_and_layer, layer};
use crate::graphic::{layer::LAYER_MANAGER, window::draw_window};

const KERNEL_STACK_SIZE: usize = 1024 * 1024;
static KERNEL_STACK: KernelStack = KernelStack::new();
#[repr(align(16))]
struct KernelStack([u8; KERNEL_STACK_SIZE]);
impl KernelStack {
    const fn new() -> Self {
        Self([0; KERNEL_STACK_SIZE])
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }
}

fn switch_to_kernel_stack(new_entry: fn(&BootInfo) -> !, boot_info: &BootInfo) -> ! {
    unsafe {
        asm!(
            "mov rdi, {}",
            "mov rsp, {}",
            "call {}",
            in(reg) boot_info, in(reg) KERNEL_STACK.as_ptr() as u64 + KERNEL_STACK.len() as u64,
            in(reg) new_entry
        );
    }
    loop {
        unsafe { asm!("hlt") }
    }
}

/// kernel entrypoint
#[unsafe(no_mangle)]
pub extern "sysv64" fn _start(boot_info: &BootInfo) -> ! {
    switch_to_kernel_stack(main, boot_info);
}

struct LayerIDs {
    mouse_layer_id: usize,
    console_layer_id: usize,
    bg_layer_id: usize,
}

fn init_graphic(boot_info: &BootInfo) -> LayerIDs {
    frame_buffer::init(&boot_info.graphic_info).expect("Failed to initialize the frame buffer.");

    // background
    let (bg_canvas, bg_layer) = create_canvas_and_layer(
        *frame_buffer::FRAME_BUFFER_WIDTH.wait() as u64,
        *frame_buffer::FRAME_BUFFER_HEIGHT.wait() as u64,
        false,
    );
    bg_canvas
        .lock()
        .fill_rect(
            u64vec2(0, 0),
            frame_buffer::FRAME_BUFFER_WIDTH.wait().clone() as u64,
            frame_buffer::FRAME_BUFFER_HEIGHT.wait().clone() as u64,
            rgb(0xcc241d),
        )
        .unwrap();

    // console
    let (console_canvas, console_layer) =
        create_canvas_and_layer(console::WIDTH as u64, console::HEIGHT as u64, false);
    console::init(console_canvas, rgb(0x3c3836), rgb(0xebdbb2))
        .expect("Failed to initialize the console.");

    // mouse
    let (mouse_canvas, mouse_layer) = create_canvas_and_layer(
        mouse::MOUSE_CURSOR_WIDTH as u64,
        mouse::MOUSE_CURSOR_HEIGHT as u64,
        true,
    );
    mouse::draw_mouse_cursor(mouse_canvas, u64vec2(0, 0));

    let mut layer_manager = layer::LAYER_MANAGER.lock();
    layer_manager.init(frame_buffer::FRAME_BUFFER.wait().clone());

    let bg_layer_id = layer_manager.add_layer(bg_layer);
    let console_layer_id = layer_manager.add_layer(console_layer);
    let mouse_layer_id = layer_manager.add_layer(mouse_layer);

    debug!("bg_layer: {}", bg_layer_id);
    debug!("console_layer: {}", console_layer_id);
    debug!("mouse_layer: {}", mouse_layer_id);
    layer_manager.up_or_down(bg_layer_id, 0);
    layer_manager.up_or_down(console_layer_id, 1);
    layer_manager.up_or_down(mouse_layer_id, 2);
    layer_manager.draw();

    LayerIDs {
        mouse_layer_id,
        console_layer_id,
        bg_layer_id,
    }
}

fn main(boot_info: &BootInfo) -> ! {
    logger::init();
    paging::init();
    gdt::init();
    frame_manager::init(&boot_info.memory_map);
    allocator::init();

    let layer_ids = init_graphic(boot_info);

    pci::devices()
        .unwrap()
        .init()
        .unwrap_or_else(|err| error!("{:#?}", err));

    let rsdp_addr = boot_info.rsdp_addr.unwrap_or_else(|| {
        error!("RSDP adderss wan't found. The kernel will panic.");
        panic!();
    });
    info!("rsdp_addr: 0x{:X}", rsdp_addr.get());

    unsafe { acpi::init(rsdp_addr) };

    ps2::init(true, true);
    x86_64::instructions::interrupts::disable();
    interrupts::init();
    x86_64::instructions::interrupts::enable();

    timer::init_local_apic_timer();

    #[cfg(test)]
    test_main();

    // main canvas
    let (main_window_canvas, main_window_canvas_layer) = create_canvas_and_layer(160, 68, false);
    draw_window(main_window_canvas.clone(), "Hello Window");
    let main_window_layer_id = LAYER_MANAGER.lock().add_layer(main_window_canvas_layer);
    LAYER_MANAGER
        .lock()
        .move_absolute(main_window_layer_id, u64vec2(300, 100));
    LAYER_MANAGER.lock().up_or_down(main_window_layer_id, 2);

    let mut loop_count: u64 = 0;

    LAYER_MANAGER.lock().draw();

    loop {
        {
            let loop_count_str = format!("{}", loop_count);
            let mut locked_main_window_canvas = main_window_canvas.lock();
            locked_main_window_canvas
                .fill_rect(u64vec2(24, 28), 8 * 10, 16, rgb(0xc6c6c6))
                .unwrap();
            locked_main_window_canvas
                .write_string(u64vec2(24, 28), &loop_count_str, rgb(0x000000))
                .unwrap();
        }

        loop_count += 1;

        if message::count() > 0 {
            x86_64::instructions::interrupts::disable();
            if let Some(message) = message::QUEUE.lock().dequeue() {
                match message {
                    message::Message::PS2KeyboardInterrupt => {
                        // must receive data to prevent the block
                        let data = unsafe { ps2::keyboard().lock().read_data() };
                        debug!("{:?}", data);
                    }
                    message::Message::PS2MouseInterrupt => {
                        let event = unsafe { ps2::mouse().lock().receive_events() };

                        match event {
                            Ok(mouse::MouseEvent::Move { displacement }) => {
                                let mut layer_manager = layer::LAYER_MANAGER.lock();
                                layer_manager.move_relative(layer_ids.mouse_layer_id, displacement);

                                timer::start_local_apic_timer();

                                layer_manager.draw();
                                let elapsed = timer::local_apic_timer_elapsed();
                                timer::stop_local_apic_timer();

                                debug!("elapsed: {}", elapsed);
                            }
                            Err(err) => match err {
                                MouseError::ControllerError(ControllerError::Timeout) => {
                                    error!("mouse timeout")
                                }
                                _ => {
                                    panic!("{:?}", err);
                                }
                            },
                            _ => {}
                        }
                    }
                    message::Message::LocalAPICTimerInterrupt => {
                        debug!("local apic timer interrupt occured!");
                    }
                }
            }

            layer::LAYER_MANAGER.lock().draw();
            x86_64::instructions::interrupts::enable();
        } else {
            unsafe { asm!("hlt") }
        }
    }
}

trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) -> () {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

#[cfg(test)]
fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());

    for test in tests {
        test.run();
    }

    qemu::exit_qemu(qemu::QemuExitCode::Success);
}

#[test_case]
fn tribial_assertion() {
    assert_eq!(1, 1);
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use qemu::exit_qemu;

    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(qemu::QemuExitCode::Failed);
    loop {
        unsafe { asm!("hlt") }
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("{}", info);
    loop {
        unsafe { asm!("hlt") }
    }
}
