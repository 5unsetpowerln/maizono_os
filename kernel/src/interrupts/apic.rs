use core::arch::asm;
use core::ptr::{read_volatile, write_volatile};

pub unsafe fn read_msr(msr: u32) -> u64 {
    let high: u32;
    let low: u32;
    unsafe {
        asm!(
            "rdmsr",
            in("ecx") msr,
            out("edx") high,
            out("eax") low,
        );
    }
    ((high as u64) << 32) | (low as u64)
}

pub unsafe fn write_msr(msr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;

    unsafe {
        asm!(
            "wrmsr",
            in("ecx") msr,
            in("edx") high,
            in("eax") low,
        );
    }
}

// Pointer to local APIC is NOT thread-safe but, this field is private and all actions to write are done through the methods.
// Actions to write are done while this struct is locked therefore, this struct is thread-safe (probably).
#[derive(Debug, Clone, Copy)]
pub struct LocalApic {
    ptr: u32,
}

impl LocalApic {
    pub const fn new(base_addr: u32) -> Self {
        Self { ptr: base_addr }
    }

    fn write(&mut self, offset: usize, value: u32) {
        let ptr = unsafe { (self.ptr as *mut u32).add(offset) };
        unsafe {
            ptr.write_volatile(value);
        }
    }

    fn read(&self, offset: usize) -> u32 {
        let ptr = unsafe { (self.ptr as *mut u32).add(offset) };
        unsafe {
            return ptr.read_volatile();
        }
    }

    /// Volatile-write task priority register
    pub fn write_task_priority_register(&mut self, value: u32) {
        self.write(0x80 / 4, value);
    }

    pub fn write_end_of_interrupt_register(&mut self, value: u32) {
        self.write(0xb0 / 4, value);
    }

    pub fn write_spurious_interrupt_vector_register(&mut self, value: u32) {
        self.write(0xf0 / 4, value);
    }

    pub fn write_error_status_register(&mut self, value: u32) {
        self.write(0x280 / 4, value);
    }

    pub fn write_interrupt_command_register_low(&mut self, value: u32) {
        self.write(0x300 / 4, value);
    }

    /// Volatile-read interrupt command register
    pub fn read_interrupt_command_register_low(&self) -> u32 {
        return self.read(0x300 / 4);
    }

    pub fn write_interrupt_command_register_high(&mut self, value: u32) {
        self.write(0x310 / 4, value);
    }

    pub fn write_lvt_timer_register(&mut self, value: u32) {
        self.write(0x320 / 4, value);
    }

    pub fn write_lvt_performance_monitoring_counters_register(&mut self, value: u32) {
        self.write(0x340 / 4, value);
    }

    pub fn write_lvt_lint0_register(&mut self, value: u32) {
        self.write(0x350 / 4, value);
    }

    pub fn write_lvt_lint1_register(&mut self, value: u32) {
        self.write(0x360 / 4, value);
    }

    pub fn write_lvt_error_register(&mut self, value: u32) {
        self.write(0x370 / 4, value);
    }

    pub fn write_initial_count_register_for_timer(&mut self, value: u32) {
        self.write(0x380 / 4, value);
    }

    pub fn write_current_count_register_for_timer(&mut self, value: u32) {
        self.write(0x390 / 4, value);
    }

    pub fn read_current_count_register_for_timer(&self) -> u32 {
        return self.read(0x390 / 4);
    }

    pub fn write_divide_config_register_for_timer(&mut self, value: u32) {
        self.write(0x3e0 / 4, value);
    }
}

pub struct IoApic {
    ptr: *mut IoApicMmioInterface,
}

#[repr(C)]
struct IoApicMmioInterface {
    reg: u32,
    pad: [u32; 3],
    data: u32,
}

impl IoApic {
    pub const fn new(base_addr: u32) -> Self {
        Self {
            ptr: base_addr as *mut IoApicMmioInterface,
        }
    }

    unsafe fn read(&self, reg: u32) -> u32 {
        unsafe {
            write_volatile(&mut (*self.ptr).reg, reg);
            return read_volatile(&(*self.ptr).data);
        }
    }

    unsafe fn write(&self, reg: u32, data: u32) {
        unsafe {
            write_volatile(&mut (*self.ptr).reg, reg);
            write_volatile(&mut (*self.ptr).data, data);
        }
    }

    /// index: irq
    /// value: vector
    pub unsafe fn set_redirection_entry_at(&self, index: u32, value: u64) {
        unsafe {
            self.write(0x10 + 2 * index, value as u32);
            self.write(0x10 + 2 * index + 1, (value >> 32) as u32);
        }
    }

    // Get the maximum amount of redirection entries in bits 16-23. All other bits are reserved. Read only.
    pub fn get_max_amount_of_redirection_entries(&self) -> usize {
        (unsafe { self.read(0x1) >> 16 } & 0xff) as usize
    }

    // Get/set the IO APIC's id in bits 24-27. All other bits are reserved.
    pub fn get_id(&self) -> u8 {
        (unsafe { self.read(0x0) >> 24 } & 0xf) as u8
    }
}
