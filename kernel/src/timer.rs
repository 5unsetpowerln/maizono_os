use core::cmp::Ordering;

use alloc::collections::binary_heap::BinaryHeap;
use core::arch::{asm, naked_asm};
use log::debug;
use spin::Once;
use x86_64::structures::idt::InterruptStackFrame;

use crate::mutex::Mutex;
use crate::task::{TASK_MANAGER, TaskContext};
use crate::{
    acpi,
    interrupts::{self, LAPIC},
    message::Message,
    task::{self, TASK_TIMER_PERIOD},
};
use task::TaskManagerTrait;

pub static TIMER_MANAGER: Mutex<TimerManager> = Mutex::new(TimerManager::new());
pub static LAPIC_TIMER_FREQ: Once<u32> = Once::new();
pub const TIMER_FREQ: u32 = 100;

pub struct TimerManager {
    tick: u64,
    timers: BinaryHeap<Timer>,
}

impl TimerManager {
    const fn new() -> Self {
        Self {
            tick: 0,
            timers: BinaryHeap::new(),
        }
    }

    pub fn increment_tick(&mut self) -> bool {
        // self.tick++
        let current_tick = self.tick;
        let tick_ptr = &mut self.tick as *mut u64;

        unsafe {
            tick_ptr.write_volatile(current_tick + 1);
        }

        let mut is_preemptive_multitask_timeout = false;

        // timeout process
        while let Some(top_timer) = self.timers.peek() {
            if top_timer.timeout > self.tick {
                break;
            }

            let timer = self.timers.pop().unwrap();

            if let TimerKind::PreemptiveMultitask = timer.kind {
                is_preemptive_multitask_timeout = true;

                self.add_timer(Timer::new(
                    TASK_TIMER_PERIOD + timer.timeout,
                    TimerKind::PreemptiveMultitask,
                ));

                continue;
            }

            let message = Message::TimerTimeout(timer);
            task::TASK_MANAGER
                .wait()
                .lock()
                .send_message_to_task(1, &message)
                .expect("Failed to send a message to main task.");
        }

        is_preemptive_multitask_timeout
    }

    pub fn get_current_tick(&self) -> u64 {
        self.tick
    }

    pub fn add_timer(&mut self, timer: Timer) {
        self.timers.push(timer);
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Timer {
    pub timeout: u64,
    pub kind: TimerKind,
}

impl Timer {
    pub fn new(timeout: u64, kind: TimerKind) -> Self {
        Self { timeout, kind }
    }
}

#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum TimerKind {
    PreemptiveMultitask = 0,
    Other = 1,
}

impl Ord for Timer {
    fn cmp(&self, other: &Self) -> Ordering {
        other.timeout.cmp(&self.timeout)
    }
}

impl PartialOrd for Timer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// pub fn init() {
//     TIMER_MANAGER.lock().timers.push(Timer::new(u64::MAX, 0));
// }

/// - divide: 1:1
/// - not-masked
/// - mode: periodic
/// start local apic timer with periodic mode
pub fn init_lagic_timer() {
    debug!("Initializing lapic timer");

    let mut lapic = LAPIC.lock();

    // lapicの周波数を計測
    lapic.write_divide_config_register_for_timer(0b1011); // divide 1:1
    lapic.write_lvt_timer_register(0b001 << 16); // masked, one-shot

    lapic.write_initial_count_register_for_timer(u32::MAX); // start lapic timer
    acpi::wait_milli_secs(100); // 100ミリ秒待機
    let elapsed = u32::MAX - lapic.read_current_count_register_for_timer();
    lapic.write_initial_count_register_for_timer(0);

    LAPIC_TIMER_FREQ.call_once(|| elapsed * 10);

    // lapicを周期モードでスタート
    lapic.write_divide_config_register_for_timer(0b1011); // divide 1:1
    lapic.write_lvt_timer_register(
        (0b010 << 16) | interrupts::InterruptVector::LocalAPICTimer.as_u8() as u32,
    ); // not-masked, periodic

    lapic.write_initial_count_register_for_timer(LAPIC_TIMER_FREQ.wait() / TIMER_FREQ); // lapicの周波数 * 割り込み周期

    debug!("Initialized lapic timer.")
}

// pub extern "x86-interrupt" fn interrupt_handler(interrupt_stack_frame: InterruptStackFrame) {
fn on_interrupt(ctx: &TaskContext) {
    task::TASK_MANAGER
        .wait()
        .lock()
        .send_message_to_task(1, &Message::LocalAPICTimerInterrupt)
        .expect("Failed to send a message to main task.");

    let is_preemptive_multitask_timeout = { TIMER_MANAGER.lock().increment_tick() };

    interrupts::notify_end_of_interrupt();

    if is_preemptive_multitask_timeout {
        TASK_MANAGER.wait().switch_task(ctx);
    }
}

#[naked]
pub extern "x86-interrupt" fn interrupt_handler(interrupt_stack_frame: InterruptStackFrame) {
    // rsp -> | TaskContext |
    //        |             |
    // rbp -> | rbp         |
    //        | rip         | StackFrame
    //        | cs          |
    //        | rflags      |
    //        | rsp         |
    //        | ss          |
    unsafe {
        naked_asm!(
            "push rbp",
            "mov rbp, rsp",
            "sub rsp, 512",
            "fxsave [rsp]",
            // general registers
            "push r15",
            "push r14",
            "push r13",
            "push r12",
            "push r11",
            "push r10",
            "push r9",
            "push r8",
            "push qword ptr [rbp]", // rbp
            "push qword ptr [rbp+0x20]", // rsp
            "push rsi",
            "push rdi",
            "push rdx",
            "push rcx",
            "push rbx",
            "push rax",
            // segment
            "mov ax, fs",
            "mov bx, gs",
            "mov rcx, cr3",
            "push rbx",                // gs
            "push rax",                // fs
            "push qword ptr [rbp + 0x28]", // ss
            "push qword ptr [rbp + 0x10]", // cs
            "push rbp",                // reserved1
            "push qword ptr [rbp + 0x18]", // 10
            "push qword ptr [rbp + 0x08]", // 08
            "push rcx", // 00
            // on_interrupt
            "mov rdi, rsp",
            "lea rax, [rip + {on_interrupt}]",
            "call rax",
            // 状態の復元
            "add rsp, 8 * 8", // cr3 ~ gsを無視
            "pop rax",
            "pop rbx",
            "pop rcx",
            "pop rdx",
            "pop rdi",
            "pop rsi",
            "add rsp, 0x10", // rsp, rbpを無視
            "pop r8",
            "pop r9",
            "pop r10",
            "pop r11",
            "pop r12",
            "pop r13",
            "pop r14",
            "pop r15",
            "fxrstor [rsp]",
            "mov rsp, rbp",
            "pop rbp",
            "iretq",
            on_interrupt = sym on_interrupt
        );
    }
}
