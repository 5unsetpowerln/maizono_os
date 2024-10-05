use uefi::boot::MemoryType;

pub const UEFI_PAGE_SIZE: usize = 1024 * 4;

#[inline]
pub fn is_available(memory_type: MemoryType) -> bool {
    memory_type == MemoryType::BOOT_SERVICES_CODE
        || memory_type == MemoryType::BOOT_SERVICES_DATA
        || memory_type == MemoryType::CONVENTIONAL
}
