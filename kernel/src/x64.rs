use core::arch::asm;
use core::ptr::read_volatile;

// MSR

pub const IA32_APIC_BASE_MSR: u32 = 0x1B;
pub const IA32_X2APIC_APICID: u32 = 0x802;

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

// MMIO

pub unsafe fn mmio_read_u32(addr: u64) -> u32 {
    unsafe { read_volatile(addr as *const u32) }
}
