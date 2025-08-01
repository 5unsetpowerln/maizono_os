use core::cmp::Ordering;

use alloc::collections::binary_heap::BinaryHeap;
use log::debug;
use spin::{Mutex, Once};
use x86_64::structures::idt::InterruptStackFrame;

use crate::task::{TASK_MANAGER, TaskManager, switch_context};
use crate::{
    acpi,
    interrupts::{self, LAPIC},
    message::{self, Message},
    task::{self, TASK_TIMER_PERIOD},
};

const INITIAL_COUNT: u32 = 0x1000000;

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
            message::enqueue(message);
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

#[derive(Debug, PartialEq, Eq)]
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
#[derive(Debug, PartialEq, Eq)]
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

// pub fn local_apic_timer_interrupt_hook() {
//     TIMER_MANAGER.lock().increment_tick();
// }

static TEST_COUNTER: Mutex<u64> = Mutex::new(0);
pub extern "x86-interrupt" fn interrupt_handler(_stack_frame: InterruptStackFrame) {
    message::enqueue(Message::LocalAPICTimerInterrupt);
    let is_preemptive_multitask_timeout = TIMER_MANAGER.lock().increment_tick();

    // if is_preemptive_multitask_timeout {
    // }

    interrupts::notify_end_of_interrupt();

    if is_preemptive_multitask_timeout {
        let (next_ctx, current_ctx) = {
            let mut task_manager = task::TASK_MANAGER.wait().lock();
            task_manager.get_contexts_for_task_switching()
        };
        debug!("{:p} -> {:p}", current_ctx, next_ctx);
        unsafe {
            switch_context(next_ctx, current_ctx);
        }
    }
}
