use core::ptr::write_volatile;

use spin::Mutex;

use crate::{interrupts, kprintln};

const COUNT_MAX: u32 = 0xffffffff;

static TIMER_MANAGER: Mutex<TimerManager> = Mutex::new(TimerManager::new());

struct TimerManager {
    tick: u64,
}

impl TimerManager {
    const fn new() -> Self {
        Self { tick: 0 }
    }

    fn tick(&mut self) {
        self.tick += 1;
    }

    fn current_tick(&self) -> u64 {
        self.tick
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
