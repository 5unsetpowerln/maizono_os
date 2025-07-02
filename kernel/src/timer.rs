use spin::Mutex;

use crate::interrupts;

const INITIAL_COUNT: u32 = 0x1000000;

pub static TIMER_MANAGER: Mutex<TimerManager> = Mutex::new(TimerManager::new());

pub struct TimerManager {
    tick: u64,
}

impl TimerManager {
    const fn new() -> Self {
        Self { tick: 0 }
    }

    pub fn increment_tick(&mut self) {
        self.tick += 1;
    }

    pub fn get_current_tick(&self) -> u64 {
        self.tick
    }
}

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

// pub fn start_local_apic_timer() {
//     interrupts::LOCAL_APIC
//         .wait()
//         .write_initial_count_register_for_timer(COUNT_MAX);
// }
//
// pub fn local_apic_timer_elapsed() -> u32 {
//     COUNT_MAX
//         - interrupts::LOCAL_APIC
//             .wait()
//             .read_current_count_register_for_timer()
// }
//
// pub fn stop_local_apic_timer() {
//     interrupts::LOCAL_APIC
//         .wait()
//         .write_initial_count_register_for_timer(0);
// }
