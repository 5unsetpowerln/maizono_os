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
mod segment;
mod serial;
mod timer;
mod types;
mod util;

use core::arch::naked_asm;
use core::panic::PanicInfo;
use core::{arch::asm, ptr::null_mut};

use alloc::vec;
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
use pc_keyboard::DecodedKey;
use spin::mutex::Mutex;
use timer::Timer;

use crate::graphic::layer::LAYER_MANAGER;
use crate::graphic::{create_canvas_and_layer, layer};

use self::{
    message::Message,
    segment::{KERNEL_CS, KERNEL_SS},
    util::read_cr3_raw,
};

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

fn main(boot_info: &BootInfo) -> ! {
    logger::init();

    paging::init();
    gdt::init();
    frame_manager::init(&boot_info.memory_map);
    allocator::init();

    let _layer_ids = init_graphic(boot_info);

    pci::devices()
        .unwrap()
        .init()
        .unwrap_or_else(|err| error!("{:#?}", err));

    unsafe { acpi::init(boot_info.rsdp) };

    ps2::init(true, false);
    x86_64::instructions::interrupts::disable();
    interrupts::init();
    x86_64::instructions::interrupts::enable();

    timer::init_lagic_timer();
    timer::TIMER_MANAGER.lock().add_timer(Timer::new(100, 1));

    let task_b_stack = vec![0u64; 1024 * 128];
    let task_b_stack_end =
        &task_b_stack[1024 * 128 - 1] as *const u64 as u64 + size_of::<u64>() as u64;
    {
        let mut task_b_ctx = TASK_B_CTX.lock();
        task_b_ctx.0.rip = task_b as *const fn(u32, u32) as u64;

        task_b_ctx.0.rdi = 1;
        task_b_ctx.0.rsi = 42;
        task_b_ctx.0.cr3 = unsafe { read_cr3_raw() };

        task_b_ctx.0.rflags = 0x202;
        task_b_ctx.0.cs = KERNEL_CS;
        task_b_ctx.0.ss = KERNEL_SS;
        task_b_ctx.0.rsp = (task_b_stack_end & !0xf) - 8;

        unsafe {
            let mut ptr = &task_b_ctx.0.fxsave_area[24] as *const u8 as *mut u32;
            *ptr = 0x1f80;
        }
    }

    #[cfg(test)]
    test_main();

    loop {
        layer::LAYER_MANAGER.lock().draw();
        if message::count() > 0 {
            let mut message_opt: Option<Message> = None;

            x86_64::instructions::interrupts::without_interrupts(|| {
                message_opt = message::QUEUE.lock().dequeue();
            });

            if let Some(message) = message_opt {
                match message {
                    message::Message::PS2KeyboardInterrupt(result) => {
                        if let Ok(scancode) = result {
                            if let Some(key_code) = ps2::read_key_event(scancode) {
                                match key_code {
                                    DecodedKey::Unicode(character) => kprint!("{}", character),
                                    DecodedKey::RawKey(key) => kprint!("{:?}", key),
                                }
                            }
                        }
                    }
                    message::Message::LocalAPICTimerInterrupt => {}
                    message::Message::TimerTimeout(timer) => {
                        info!("timer timeout: {}, {}", timer.timeout, timer.value);

                        timer::TIMER_MANAGER
                            .lock()
                            .add_timer(Timer::new(timer.timeout + 100, timer.value + 1));
                    }
                    message::Message::PS2MouseInterrupt => {
                        error!("PS2 mouse is disabled but the interrupt occured.");
                    }
                }
            }
        } else {
            let mut current_ctx_ptr = null_mut();
            let mut next_ctx_ptr = null_mut();
            x86_64::instructions::interrupts::without_interrupts(|| {
                let (task_a_ctx, task_b_ctx) = { (&*TASK_A_CTX.lock(), &*TASK_B_CTX.lock()) };
                current_ctx_ptr = task_a_ctx as *const TaskContext as *mut TaskContext;
                next_ctx_ptr = task_b_ctx as *const TaskContext as *mut TaskContext;
            });
            unsafe {
                switch_context(next_ctx_ptr, current_ctx_ptr);
                asm!("hlt");
            }
        }
    }
}

#[naked]
unsafe extern "C" fn switch_context(next_ctx: *mut TaskContext, current_ctx: *const TaskContext) {
    unsafe {
        naked_asm!(
            // asm!(
            "mov [rsi + 0x40], rax",
            "mov [rsi + 0x48], rbx",
            "mov [rsi + 0x50], rcx",
            "mov [rsi + 0x58], rdx",
            "mov [rsi + 0x60], rdi",
            "mov [rsi + 0x68], rsi",
            "lea rax, [rsp + 8]",
            "mov [rsi + 0x70], rax", // RSP
            "mov [rsi + 0x78], rbp",
            "mov [rsi + 0x80], r8",
            "mov [rsi + 0x88], r9",
            "mov [rsi + 0x90], r10",
            "mov [rsi + 0x98], r11",
            "mov [rsi + 0xa0], r12",
            "mov [rsi + 0xa8], r13",
            "mov [rsi + 0xb0], r14",
            "mov [rsi + 0xb8], r15",
            "mov rax, cr3",
            "mov [rsi + 0x00], rax", // CR3
            "mov rax, [rsp]",
            "mov [rsi + 0x08], rax", // RIP
            "pushfq",
            "pop qword ptr [rsi + 0x10]", // RFLAGS
            "xor rax, rax",
            "mov ax, cs",
            "mov [rsi + 0x20], rax",
            "mov ax, ss",
            "mov [rsi + 0x28], rax",
            "mov ax, fs",
            "mov [rsi + 0x30], rax",
            "mov ax, gs",
            "mov [rsi + 0x38], rax",
            "fxsave [rsi + 0xc0]",
            // iret 用のスタックフレーム
            "push qword ptr [rdi + 0x28]", // SS
            "push qword ptr [rdi + 0x70]", // RSP
            "push qword ptr [rdi + 0x10]", // RFLAGS
            "push qword ptr [rdi + 0x20]", // CS
            "push qword ptr [rdi + 0x08]", // RIP
            // コンテキストの復帰
            "fxrstor [rdi + 0xc0]",
            "mov rax, [rdi + 0x00]",
            "mov cr3, rax",
            "mov rax, [rdi + 0x30]",
            "mov fs, ax",
            "mov rax, [rdi + 0x38]",
            "mov gs, ax",
            "mov rax, [rdi + 0x40]",
            "mov rbx, [rdi + 0x48]",
            "mov rcx, [rdi + 0x50]",
            "mov rdx, [rdi + 0x58]",
            "mov rsi, [rdi + 0x68]",
            "mov rbp, [rdi + 0x78]",
            "mov r8,  [rdi + 0x80]",
            "mov r9,  [rdi + 0x88]",
            "mov r10, [rdi + 0x90]",
            "mov r11, [rdi + 0x98]",
            "mov r12, [rdi + 0xa0]",
            "mov r13, [rdi + 0xa8]",
            "mov r14, [rdi + 0xb0]",
            "mov r15, [rdi + 0xb8]",
            "mov rdi, [rdi + 0x60]",
            "iretq",
            // options(nostack)
        )
    }
}

#[repr(C, packed)]
#[derive(Debug)]
struct TaskContextInner {
    // offset 0x00
    cr3: u64,
    rip: u64,
    rflags: u64,
    reserved_1: u64,
    // offset: 0x20
    cs: u64,
    ss: u64,
    fs: u64,
    gs: u64,
    // offset: 0x40
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rdi: u64,
    rsi: u64,
    rsp: u64,
    rbp: u64,
    // offset: 0x80
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,
    // offset: 0xc0
    fxsave_area: [u8; 512],
}

#[derive(Debug)]
#[repr(align(16))]
struct TaskContext(TaskContextInner);

impl TaskContext {
    pub const fn zero() -> Self {
        Self(TaskContextInner {
            cr3: 0,
            rip: 0,
            rflags: 0,
            reserved_1: 0,
            cs: 0,
            ss: 0,
            fs: 0,
            gs: 0,
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rdi: 0,
            rsi: 0,
            rsp: 0,
            rbp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            fxsave_area: [0; 512],
        })
    }
}

static TASK_A_CTX: Mutex<TaskContext> = Mutex::new(TaskContext::zero());
static TASK_B_CTX: Mutex<TaskContext> = Mutex::new(TaskContext::zero());

fn task_b(task_id: u32, data: u32) {
    let mut count = 0;
    loop {
        let mut current_ctx_ptr: *mut TaskContext = null_mut();
        let mut next_ctx_ptr: *mut TaskContext = null_mut();
        x86_64::instructions::interrupts::without_interrupts(|| {
            let (task_a_ctx, task_b_ctx) = { (&*TASK_A_CTX.lock(), &*TASK_B_CTX.lock()) };
            current_ctx_ptr = task_b_ctx as *const TaskContext as *mut TaskContext;
            next_ctx_ptr = task_a_ctx as *const TaskContext as *mut TaskContext;
        });
        unsafe {
            info!("task B");
            switch_context(next_ctx_ptr, current_ctx_ptr);
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
