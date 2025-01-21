use core::ptr::write_volatile;

use spin::Mutex;

use crate::{interrupts, kprintln};

const COUNT_MAX: u32 = 0xffffffff;
// const LVT_TIMER: *mut u32 = 0xfee00320 as *mut u32;
// const INITIAL_COUNT: *mut u32 = 0xfee00380 as *mut u32;
// const CURRENT_COUNT: *mut u32 = 0xfee00390 as *mut u32;
// const DIVIDE_CONFIG: *mut u32 = 0xfee003e0 as *mut u32;

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
    // write_volatile(DIVIDE_CONFIG, 0b1011); // divide 1:1
    interrupts::get_local_apic().write_divide_config_register_for_timer(0b1011);
    // write_volatile(
    //     LVT_TIMER,
    //     (0b010 << 16) | interrupts::InterruptVector::LocalAPICTimer as u32,
    // ); // not-masked, periodic
    interrupts::get_local_apic()
        .set_lvt_timer_register((0b010 << 16) | interrupts::InterruptVector::LocalAPICTimer as u32);
    // not-masked, periodic
    // write_volatile(INITIAL_COUNT, 0x1000000);
    interrupts::get_local_apic().write_initial_count_register_for_timer(0x1000000);
}

pub fn start_local_apic_timer() {
    // unsafe {
    // write_volatile(INITIAL_COUNT, COUNT_MAX);
    // }
    interrupts::get_local_apic().write_initial_count_register_for_timer(COUNT_MAX);
}

pub fn local_apic_timer_elapsed() -> u32 {
    // return unsafe { COUNT_MAX - *CURRENT_COUNT };
    return COUNT_MAX - interrupts::get_local_apic().get_current_count_register_for_timer();
}

pub fn stop_local_apic_timer() {
    // unsafe {
    // write_volatile(INITIAL_COUNT, 0);
    // }
    interrupts::get_local_apic().write_initial_count_register_for_timer(0);
}
