#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(ascii_char)]
#![feature(ascii_char_variants)]
#![feature(naked_functions)]
#![feature(ptr_as_ref_unchecked)]
#![feature(default_field_values)]

extern crate alloc;

mod acpi;
mod allocator;
mod cpu;
mod device;
mod error;
mod fat;
mod frame_manager;
mod gdt;
mod graphic;
mod interrupts;
mod logger;
mod memory_map;
mod message;
mod mouse;
mod mutex;
mod paging;
mod pci;
mod qemu;
mod segment;
mod serial;
mod task;
mod terminal;
mod timer;
mod types;
mod util;
mod x64;

use core::arch::asm;
use core::panic::PanicInfo;

use common::boot::BootInfo;
use common::graphic::rgb;
use device::ps2::{self};
use glam::u64vec2;
use graphic::{
    PixelWriter,
    console::{self},
    frame_buffer::{self},
};
use log::{debug, error, info};
use spin::once::Once;

use crate::graphic::{create_canvas_and_layer, layer};
use task::TaskManagerTrait;

use self::{graphic::layer::LAYER_MANAGER, task::TASK_MANAGER};

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
    console_layer_id: usize,
    bg_layer_id: usize,
}

static LAYER_IDS: Once<LayerIDs> = Once::new();

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
            *frame_buffer::FRAME_BUFFER_WIDTH.wait() as u64,
            *frame_buffer::FRAME_BUFFER_HEIGHT.wait() as u64,
            rgb(0x42484e),
        )
        .unwrap();

    // console
    let (console_canvas, console_layer) =
        create_canvas_and_layer(console::WIDTH as u64, console::HEIGHT as u64, false);
    console::init(console_canvas, rgb(0x1a2026), rgb(0xbebebe))
        .expect("Failed to initialize the console.");

    let mut layer_manager = layer::LAYER_MANAGER.lock();
    layer_manager.init(frame_buffer::FRAME_BUFFER.wait().clone());

    let bg_layer_id = layer_manager.add_layer(bg_layer);
    let console_layer_id = layer_manager.add_layer(console_layer);

    debug!("bg_layer: {}", bg_layer_id);
    debug!("console_layer: {}", console_layer_id);
    layer_manager.up_or_down(bg_layer_id, 0);
    layer_manager.up_or_down(console_layer_id, 1);
    layer_manager.draw();

    LayerIDs {
        console_layer_id,
        bg_layer_id,
    }
}

pub struct TaskIDs {
    terminal_task_id: u64,
    draw_layer_task_id: u64,
}

pub static TASK_IDS: Once<TaskIDs> = Once::new();

fn main(boot_info: &BootInfo) -> ! {
    logger::init();

    paging::init();

    acpi::init(boot_info.rsdp);

    cpu::init();

    gdt::init();

    frame_manager::init(&boot_info.memory_map);

    allocator::init();

    fat::init(boot_info.volume_image);

    let layer_ids = init_graphic(boot_info);
    LAYER_IDS.call_once(|| layer_ids);

    for i in 0..16 {
        kprint!("{:04x}:", i * 16);
        for j in 0..8 {
            kprint!(" {:02x}", boot_info.volume_image[16 * i + j]);
        }

        kprint!(" ");

        for j in 8..16 {
            kprint!(" {:02x}", boot_info.volume_image[16 * i + j]);
        }

        kprint!("\n");
    }

    ps2::init(true, false);

    interrupts::init();

    timer::init_lagic_timer();

    task::init();

    terminal::init();

    {
        let mut task_manager = task::TASK_MANAGER.wait().lock();

        let terminal_task_id = task_manager
            .new_task()
            .init_context(terminal::terminal_task, 45)
            .get_id();
        task_manager
            .wakeup(terminal_task_id, None)
            .expect("Failed to wake up a task.");

        let draw_layer_task_id = task_manager
            .new_task()
            .init_context(draw_layer_task, 56)
            .get_id();
        task_manager
            .wakeup(draw_layer_task_id, None)
            .expect("Failed to wake up a task.");

        let idle_task_id = task_manager.new_task().init_context(task_idle, 54).get_id();
        task_manager
            .wakeup(idle_task_id, None)
            .expect("Failed to wake up a task");

        TASK_IDS.call_once(|| TaskIDs {
            terminal_task_id,
            draw_layer_task_id,
        });
    }

    let terminal_task_id = TASK_IDS.wait().terminal_task_id;

    #[cfg(test)]
    test_main();

    loop {
        let message_opt = task::TASK_MANAGER
            .wait()
            .lock()
            .receive_message_from_task(1)
            .expect("Failed to get a message of main task.");

        if let Some(message) = message_opt {
            match message {
                message::Message::PS2KeyboardInterrupt(result) => {
                    if let Ok(scancode) = result {
                        if let Some(key_code) = ps2::read_key_event(scancode) {
                            TASK_MANAGER
                                .wait()
                                .lock()
                                .send_message_to_task(
                                    terminal_task_id,
                                    &message::Message::KeyInput(key_code),
                                )
                                .unwrap();
                        } else {
                        }
                    } else {
                    }
                }
                message::Message::LocalAPICTimerInterrupt => {}
                message::Message::TimerTimeout(timer) => {
                    info!("timeout: ({:?}, {})", timer.kind, timer.timeout);
                }
                message::Message::PS2MouseInterrupt => {
                    error!("PS2 mouse is disabled but the interrupt occured.");
                }
                _ => {}
            }
        } else {
            task::TASK_MANAGER
                .wait()
                .sleep(1)
                .expect("Failed to sleep main task.");

            continue;
        }
    }
}

fn draw_layer_task(task_id: u64, _data: u64) {
    loop {
        if let Some(message::Message::DrawLayer) = TASK_MANAGER
            .wait()
            .lock()
            .receive_message_from_task(task_id)
            .unwrap()
        {
            LAYER_MANAGER.lock().draw();
        } else {
            TASK_MANAGER.wait().sleep(task_id).unwrap();
            continue;
        }

        unsafe { asm!("hlt") }
    }
}

fn task_idle(task_id: u64, data: u64) {
    info!("TaskIdle: task_id={task_id}, data={data}");
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

trait Testable {
    fn run(&self);
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
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

    serial_emergency_println!("[failed]\n");
    serial_emergency_println!("Error: {}\n", info);
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
