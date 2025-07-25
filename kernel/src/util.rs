use core::arch::asm;

pub fn u32_from_slice(slice: &[u8]) -> u32 {
    let mut bytes = [0; 4];
    bytes.copy_from_slice(slice);

    u32::from_le_bytes(bytes)
}

#[inline]
pub unsafe fn read_cr3_raw() -> u64 {
    let value: u64;
    unsafe {
        asm!(
            "mov {}, cr3",
            out(reg) value,
            options(nostack, nomem, preserves_flags)
        );
    }
    value
}
