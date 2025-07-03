use core::cmp::Ordering;

use alloc::collections::binary_heap::BinaryHeap;
use spin::Mutex;

use crate::{
    interrupts,
    message::{self, Message},
};

const INITIAL_COUNT: u32 = 0x1000000;

pub static TIMER_MANAGER: Mutex<TimerManager> = Mutex::new(TimerManager::new());

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

    pub fn increment_tick(&mut self) {
        // self.tick++
        let current_tick = self.tick;
        let tick_ptr = &mut self.tick as *mut u64;

        unsafe {
            tick_ptr.write_volatile(current_tick + 1);
        }

        // timeout process
        while let Some(top_timer) = self.timers.peek() {
            if top_timer.timeout > self.tick {
                break;
            }

            let timer = self.timers.pop().unwrap();
            let message = Message::TimerTimeout(timer);
            message::enqueue(message);
        }
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
    pub value: i64,
}

impl Timer {
    pub fn new(timeout: u64, value: i64) -> Self {
        Self { timeout, value }
    }

    #[inline]
    pub fn get_timeout(&self) -> u64 {
        self.timeout
    }

    #[inline]
    pub fn get_value(&self) -> i64 {
        self.value
    }
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
pub fn init_local_apic_timer() {
    // divide 1:1
    interrupts::LOCAL_APIC
        .wait()
        .write_divide_config_register_for_timer(0b1011);

    // not-masked, periodic
    interrupts::LOCAL_APIC.wait().write_lvt_timer_register(
        (0b010 << 16) | interrupts::InterruptVector::LocalAPICTimer.as_u8() as u32,
    );

    interrupts::LOCAL_APIC
        .wait()
        .write_initial_count_register_for_timer(INITIAL_COUNT);
}

pub fn local_apic_timer_interrupt_hook() {
    TIMER_MANAGER.lock().increment_tick();
}
