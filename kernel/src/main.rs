#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(ascii_char)]
#![feature(ascii_char_variants)]

extern crate alloc;

mod acpi;
mod allocator;
mod device;
mod error;
mod frame_manager;
mod gdt;
mod graphic;
mod interrupts;
mod layer;
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
mod window;

use core::arch::asm;
use core::panic::PanicInfo;

use alloc::sync::Arc;
use common::{boot::BootInfo, graphic::RgbColor};
use device::ps2;
use glam::u64vec2;
use graphic::{
    console::{self},
    frame_buffer::{self},
};
use layer::Layer;
use spin::{Lazy, Mutex, Once};
use window::Window;

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
    frame_buffer::init(&boot_info.graphic_info, RgbColor::from(0x28282800))
        .expect("Failed to initialize the frame buffer.");

    let create_window = |width: u64, height: u64, transparent_color: Option<RgbColor>| {
        let mut window = Window::new();
        window.init(width, height, transparent_color);
        Arc::new(Mutex::new(window))
    };

    // background
    let bg_window = create_window(
        *frame_buffer::FRAME_BUFFER_WIDTH.wait() as u64,
        *frame_buffer::FRAME_BUFFER_HEIGHT.wait() as u64,
        None,
    );
    let bg_layer = Layer::new(bg_window);

    // console
    let console_window = create_window(console::WIDTH as u64, console::HEIGHT as u64, None);
    let console_layer = Layer::new(console_window.clone());
    console::init(
        console_window,
        RgbColor::from(0x3c383600),
        RgbColor::from(0xebdbb200),
    )
    .expect("Failed to initialize the console.");

    // mouse
    let mouse_window = create_window(
        mouse::MOUSE_CURSOR_WIDTH as u64,
        mouse::MOUSE_CURSOR_HEIGHT as u64,
        Some(mouse::MOUSE_TRANSPARENT_COLOR),
    );
    mouse::draw_mouse_cursor(mouse_window.clone(), u64vec2(0, 0));
    let mouse_layer = Layer::new(mouse_window);

    let mut layer_manager = layer::LAYER_MANAGER.lock();
    layer_manager.init(frame_buffer::FRAME_BUFFER.wait().clone());
    let bg_layer_id = layer_manager.add_layer(bg_layer);
    let console_layer_id = layer_manager.add_layer(console_layer);
    let mouse_layer_id = layer_manager.add_layer(mouse_layer);

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
    paging::init();
    gdt::init();
    frame_manager::init(&boot_info.memory_map);
    allocator::init();

    let layer_ids = init_graphic(boot_info);

    pci::devices()
        .unwrap()
        .init()
        .unwrap_or_else(|err| kprintln!("{:#?}", err));

    let rsdp_addr = boot_info.rsdp_addr.unwrap_or_else(|| {
        kprintln!("RSDP adderss wan't found. The kernel will panic.");
        panic!();
    });
    kprintln!("rsdp_addr: 0x{:X}", rsdp_addr.get());

    unsafe { acpi::init(rsdp_addr) };

    ps2::init(true, true);
    x86_64::instructions::interrupts::disable();
    interrupts::init();
    x86_64::instructions::interrupts::enable();

    timer::init_local_apic_timer();

    #[cfg(test)]
    test_main();

    kprintln!("It didn't crash:)");
    loop {
        if message::count() > 0 {
            x86_64::instructions::interrupts::disable();
            if let Some(message) = message::QUEUE.lock().dequeue() {
                match message {
                    message::Message::PS2KeyboardInterrupt => {
                        // must receive data to prevent the block
                        let data = unsafe { ps2::keyboard().lock().read_data() };
                        kprintln!("{:?}", data);
                    }
                    message::Message::PS2MouseInterrupt => {
                        let event = unsafe { ps2::mouse().lock().receive_events() }
                            .expect("Failed to receive events");

                        match event {
                            mouse::MouseEvent::Move { displacement } => {
                                // timer::start_local_apic_timer();

                                layer::LAYER_MANAGER
                                    .lock()
                                    .move_relative(layer_ids.mouse_layer_id, displacement);
                                layer::LAYER_MANAGER.lock().draw();

                                // let elapsed = timer::local_apic_timer_elapsed();
                                // timer::stop_local_apic_timer();
                                // kprintln!("elapsed: {}", elapsed);
                            }
                            _ => {}
                        }
                    }
                    message::Message::LocalAPICTimerInterrupt => {
                        kprintln!("local apic timer interrupt occured!");
                    }
                }
            }
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
    kprintln!("Running {} tests", tests.len());

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
    serial_println!("[panic]");
    serial_println!("{}", info);
    loop {
        unsafe { asm!("hlt") }
    }
}
