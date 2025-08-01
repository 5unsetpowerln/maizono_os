use core::arch::naked_asm;
use core::mem::MaybeUninit;
use core::ptr::{null, null_mut};

use alloc::vec::Vec;
use log::debug;
use spin::Once;
use spin::mutex::Mutex;
use x86_64::instructions::interrupts::without_interrupts;

use crate::segment::{KERNEL_CS, KERNEL_SS};
use crate::timer::{self, TIMER_FREQ, Timer, TimerKind};
use crate::util::read_cr3_raw;

pub const TASK_TIMER_PERIOD: u64 = (TIMER_FREQ as u64 / 100) * 10;
// pub const TASK_TIMER_PERIOD: u64 = TIMER_FREQ as u64;
pub static TASK_MANAGER: Once<Mutex<TaskManager>> = Once::new();

pub fn init() {
    TASK_MANAGER.call_once(|| Mutex::new(TaskManager::new()));

    without_interrupts(|| {
        let mut timer_manager = timer::TIMER_MANAGER.lock();
        let current_tick = timer_manager.get_current_tick();
        timer_manager.add_timer(Timer::new(
            TASK_TIMER_PERIOD + current_tick,
            TimerKind::PreemptiveMultitask,
        ));
    });
}

#[naked]
pub unsafe extern "C" fn switch_context(
    next_ctx: *const TaskContext,
    current_ctx: *mut TaskContext,
) {
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

pub type TaskFunc = fn(u64, u64);

const DEFAULT_STACK_SIZE: u64 = 128 * 8 * 1024;

#[derive(Debug)]
pub struct Task {
    stack_size: u64,
    id: u64,
    stack: Vec<u64>,
    context: TaskContext,
}

impl Task {
    pub fn new(id: u64) -> Self {
        Self {
            stack_size: DEFAULT_STACK_SIZE,
            id,
            stack: Vec::new(),
            context: TaskContext::zero(),
        }
    }

    pub fn init_context(&mut self, f: TaskFunc, data: u64) {
        let stack_size = self.stack_size / size_of::<u64>() as u64;
        self.stack.resize(stack_size as usize, 0);

        let stack_end_ref: &u64 = &self.stack[self.stack.len() - 1];
        let stack_end = stack_end_ref as *const u64 as u64 + size_of::<u64>() as u64;

        self.context.0.cr3 = unsafe { read_cr3_raw() };
        self.context.0.rflags = 0x202;
        self.context.0.cs = KERNEL_CS;
        self.context.0.ss = KERNEL_SS;
        self.context.0.rsp = (stack_end & !0xf) - 8;
        self.context.0.rip = f as u64;
        self.context.0.rdi = self.id;
        self.context.0.rsi = data;

        unsafe {
            let mut ptr = &self.context.0.fxsave_area[24] as *const u8 as *mut u32;
            *ptr = 0x1f80;
        }
    }

    pub fn get_context<'a>(&'a self) -> &'a TaskContext {
        &self.context
    }
}

#[derive(Debug)]
pub struct TaskManager {
    tasks: Vec<Task>,
    latest_id: u64 = 0,
    current_task_index: usize,
}

impl TaskManager {
    pub fn new() -> Self {
        let mut self_ = Self {
            tasks: Vec::new(),
            latest_id: 0,
            current_task_index: 0,
        };
        self_.new_task();
        self_
    }

    pub fn new_task(&mut self) -> &mut Task {
        self.latest_id += 1;
        self.tasks.push(Task::new(self.latest_id));
        let last_index = self.tasks.len() - 1;
        &mut self.tasks[last_index]
    }

    pub fn get_contexts_for_task_switching(&mut self) -> (*const TaskContext, *mut TaskContext) {
        // pub fn switch_task(&mut self) {
        // pub fn switch_task(self: Mutex<Self>) {
        let mut next_task_index = self.current_task_index + 1;
        if next_task_index >= self.tasks.len() {
            next_task_index = 0;
        }

        let current_context = self.tasks[self.current_task_index].get_context()
            as *const TaskContext as *mut TaskContext;
        let next_context = self.tasks[next_task_index].get_context() as *const TaskContext;
        self.current_task_index = next_task_index;

        (next_context, current_context)
        // debug!(
        //     "switching task... next_context: {next_context:?}, current_context: {current_context:?}"
        // );
        // core::mem::drop(*self);
        // unsafe {
        //     switch_context(next_context, current_context);
        // }
    }
}
