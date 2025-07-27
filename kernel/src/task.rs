use core::arch::naked_asm;
use core::ptr::{null, null_mut};

use spin::mutex::Mutex;
use x86_64::instructions::interrupts::without_interrupts;

use crate::timer::{self, TIMER_FREQ, Timer, TimerKind};

pub const TASK_TIMER_PERIOD: u64 = (TIMER_FREQ as u64 / 100) * 2;

pub fn init() {
    without_interrupts(|| {
        *IS_TASK_A.lock() = true;

        let mut timer_manager = timer::TIMER_MANAGER.lock();
        let current_tick = timer_manager.get_current_tick();
        timer_manager.add_timer(Timer::new(
            TASK_TIMER_PERIOD + current_tick,
            TimerKind::PreemptiveMultitask,
        ));
    });
}

#[naked]
unsafe extern "C" fn switch_context(next_ctx: *mut TaskContext, current_ctx: *const TaskContext) {
    unsafe {
        naked_asm!(
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
        )
    }
}

#[repr(C, packed)]
#[derive(Debug)]
pub struct TaskContextInner {
    // offset 0x00
    pub cr3: u64,
    pub rip: u64,
    pub rflags: u64,
    pub reserved_1: u64,
    // offset: 0x20
    pub cs: u64,
    pub ss: u64,
    pub fs: u64,
    pub gs: u64,
    // offset: 0x40
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rsp: u64,
    pub rbp: u64,
    // offset: 0x80
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    // offset: 0xc0
    pub fxsave_area: [u8; 512],
}

#[derive(Debug)]
#[repr(align(16))]
pub struct TaskContext(pub TaskContextInner);

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

static IS_TASK_A: Mutex<bool> = Mutex::new(true);
pub static TASK_A_CTX: Mutex<TaskContext> = Mutex::new(TaskContext::zero());
pub static TASK_B_CTX: Mutex<TaskContext> = Mutex::new(TaskContext::zero());

pub fn switch_task() {
    let mut _is_task_a = IS_TASK_A.lock();
    let is_task_a = *_is_task_a;
    *_is_task_a = !*_is_task_a;
    core::mem::drop(_is_task_a);

    if is_task_a {
        let mut task_a_ctx: *const TaskContext = null();
        let mut task_b_ctx: *mut TaskContext = null_mut();
        without_interrupts(|| {
            task_a_ctx = &*TASK_A_CTX.lock() as *const TaskContext;
            task_b_ctx = &*TASK_B_CTX.lock() as *const TaskContext as *mut TaskContext;
        });

        unsafe {
            switch_context(task_b_ctx, task_a_ctx);
        }
    } else {
        let mut task_b_ctx: *const TaskContext = null();
        let mut task_a_ctx: *mut TaskContext = null_mut();
        without_interrupts(|| {
            task_a_ctx = &*TASK_A_CTX.lock() as *const TaskContext as *mut TaskContext;
            task_b_ctx = &*TASK_B_CTX.lock() as *const TaskContext;
        });

        unsafe {
            switch_context(task_a_ctx, task_b_ctx);
        }
    }
}
