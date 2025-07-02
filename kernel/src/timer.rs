const COUNT_MAX: u32 = u32::MAX;

use crate::interrupts;

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
        (0b010 << 16) | interrupts::InterruptVector::LocalAPICTimer.as_u8() as u32,
    );

    interrupts::LOCAL_APIC
        .wait()
        .write_initial_count_register_for_timer(COUNT_MAX);
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
