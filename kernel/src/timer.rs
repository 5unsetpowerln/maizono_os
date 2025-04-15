use core::{cmp::Ordering, ptr::write_volatile};

use alloc::collections::{BinaryHeap, vec_deque::VecDeque};
use spin::{Lazy, Mutex};

use crate::{interrupts, kprintln, message::Message};

const COUNT_MAX: u32 = 0xffffffff;

static TIMER_MANAGER: Lazy<Mutex<TimerManager>> = Lazy::new(|| Mutex::new(TimerManager::new()));

struct TimerManager {
    timers: BinaryHeap<Timer>,
    tick: u64,
}

impl TimerManager {
    fn new() -> Self {
        let mut timers = BinaryHeap::new();
        timers.push(Timer {
            value: -1,
            timeout: u64::MAX,
        });
        Self { timers, tick: 0 }
    }

    fn tick(&mut self) {
        self.tick += 1;
    }

    fn current_tick(&self) -> u64 {
        self.tick
    }

    fn add_timer(&mut self, timer: Timer) {
        self.timers.push(timer);
    }
}

#[derive(Eq, PartialEq)]
struct Timer {
    value: i32,
    timeout: u64,
}

impl Timer {
    fn timeout(&self) -> u64 {
        self.timeout
    }

    fn value(&self) -> i32 {
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
        Some(self.cmp(&other))
    }
}

pub fn local_apic_timer_on_interrupt() {
    TIMER_MANAGER.lock().tick();
}

pub fn current_tick() -> u64 {
    TIMER_MANAGER.lock().current_tick()
}

/// - divide: 1:1
/// - not-masked
/// - mode: periodic
pub fn init_local_apic_timer() {
    // divide 1:1
    interrupts::LOCAL_APIC
        .wait()
        .write_divide_config_register_for_timer(0b1011);
    // not-masked, periodic
    interrupts::LOCAL_APIC.wait().write_lvt_timer_register(
        (0b010 << 16) | interrupts::InterruptVector::LocalAPICTimer as u32,
    );
    // not-masked, periodic
    interrupts::LOCAL_APIC
        .wait()
        .write_initial_count_register_for_timer(0x1000000);
}

pub fn start_local_apic_timer() {
    interrupts::LOCAL_APIC
        .wait()
        .write_initial_count_register_for_timer(COUNT_MAX);
}

pub fn local_apic_timer_elapsed() -> u32 {
    return COUNT_MAX
        - interrupts::LOCAL_APIC
            .wait()
            .read_current_count_register_for_timer();
}

pub fn stop_local_apic_timer() {
    interrupts::LOCAL_APIC
        .wait()
        .write_initial_count_register_for_timer(0);
}
