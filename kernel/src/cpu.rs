use core::mem::MaybeUninit;
use core::ptr::{addr_of, addr_of_mut};

use acpi::madt::MadtEntry;
use spin::once::Once;

use crate::acpi::get_madt;
use crate::x64::{IA32_APIC_BASE_MSR, IA32_X2APIC_APICID, mmio_read_u32, read_msr};

const MAX_CPU_COUNT: usize = 0x100;

// for debug
static INITIALIZED: Once<()> = Once::new();

static mut APIC_INFOS: [ApicInfo; MAX_CPU_COUNT] = [ApicInfo::empty(); MAX_CPU_COUNT];
static mut APIC_ID_IDX_MAP: [usize; MAX_CPU_COUNT * 2] = [0; MAX_CPU_COUNT * 2];
static mut IDX_APIC_ID_MAP: [u8; MAX_CPU_COUNT] = [0; MAX_CPU_COUNT];

static mut APIC_COUNT: usize = 0;

#[derive(Debug, Clone, Copy)]
struct ApicInfo {
    pub processor_id: u8,
    pub apic_id: u8,
    pub flags: u32,
}

impl ApicInfo {
    const fn empty() -> Self {
        Self {
            processor_id: 0,
            apic_id: 0,
            flags: 0,
        }
    }

    const fn new(processor_id: u8, apic_id: u8, flags: u32) -> Self {
        Self {
            processor_id,
            apic_id,
            flags,
        }
    }
}

pub fn init() {
    let madt = get_madt();

    let mut apic_infos = [ApicInfo::empty(); MAX_CPU_COUNT];
    let mut apic_id_idx_map = [0; MAX_CPU_COUNT * 2];
    let mut idx_apic_id_map = [0; MAX_CPU_COUNT];

    let mut count = 0;
    for madt_entry in madt.entries() {
        if count >= MAX_CPU_COUNT {
            break;
        }

        if let MadtEntry::LocalApic(apic_entry) = madt_entry {
            debug_assert!((apic_entry.apic_id as usize) < apic_id_idx_map.len());

            apic_infos[count] = ApicInfo::new(
                apic_entry.processor_id,
                apic_entry.apic_id,
                apic_entry.flags,
            );

            apic_id_idx_map[apic_entry.apic_id as usize] = count;
            idx_apic_id_map[count] = apic_entry.apic_id;

            count += 1;
            continue;
        }
    }

    unsafe {
        addr_of_mut!(APIC_INFOS).write(apic_infos);
        addr_of_mut!(APIC_ID_IDX_MAP).write(apic_id_idx_map);
        addr_of_mut!(IDX_APIC_ID_MAP).write(idx_apic_id_map);
        *addr_of_mut!(APIC_COUNT) = count;
        INITIALIZED.call_once(|| {});
    }
}

pub fn get_local_apic_id() -> u8 {
    let apic_base = unsafe { read_msr(IA32_APIC_BASE_MSR) } & 0xFFFF_F000;
    let id_reg = unsafe { mmio_read_u32(apic_base + 0x20) };
    ((id_reg >> 24) & 0xFF) as u8
}

#[inline(always)]
pub fn get_apic_count() -> usize {
    debug_assert!(INITIALIZED.is_completed());

    unsafe { *addr_of!(APIC_COUNT) }
}

#[inline(always)]
pub const fn get_apic_count_max() -> usize {
    return MAX_CPU_COUNT;
}

#[inline(always)]
pub fn apic_id_to_idx(apic_id: u8) -> usize {
    debug_assert!(INITIALIZED.is_completed());

    unsafe { (*addr_of!(APIC_ID_IDX_MAP))[apic_id as usize] }
}

#[inline(always)]
pub fn idx_to_apic_id(idx: usize) -> u8 {
    debug_assert!(INITIALIZED.is_completed());

    unsafe { (*addr_of!(IDX_APIC_ID_MAP))[idx] }
}
