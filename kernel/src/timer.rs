use core::ptr::write_volatile;

use crate::interrupts;

const COUNT_MAX: u32 = 0xffffffff;
const LVT_TIMER: *mut u32 = 0xfee00320 as *mut u32;
const INITIAL_COUNT: *mut u32 = 0xfee00380 as *mut u32;
const CURRENT_COUNT: *mut u32 = 0xfee00390 as *mut u32;
const DIVIDE_CONFIG: *mut u32 = 0xfee003e0 as *mut u32;

/// - divide: 1:1
/// - not-masked
/// - mode: periodic
pub fn init_local_apic_timer() {
    unsafe {
        write_volatile(DIVIDE_CONFIG, 0b1011); // divide 1:1
        write_volatile(
            LVT_TIMER,
            (0b010 << 16) | interrupts::InterruptVector::LocalAPICTimer as u32,
        ); // not-masked, periodic
        write_volatile(INITIAL_COUNT, COUNT_MAX);
        // write_volatile(INITIAL_COUNT, 0xffff);
    }
}

pub fn start_local_apic_timer() {
    unsafe {
        write_volatile(INITIAL_COUNT, COUNT_MAX);
    }
}

pub fn local_apic_timer_elapsed() -> u32 {
    return unsafe { COUNT_MAX - *CURRENT_COUNT };
}

pub fn stop_local_apic_timer() {
    unsafe {
        write_volatile(INITIAL_COUNT, 0);
    }
}
