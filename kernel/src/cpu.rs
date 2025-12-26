use core::mem::MaybeUninit;
use core::ptr::{addr_of, addr_of_mut};

use acpi::madt::MadtEntry;
use spin::once::Once;

use crate::acpi::get_madt;
use crate::x64::{IA32_APIC_BASE_MSR, IA32_X2APIC_APICID, mmio_read_u32, read_msr};

const MAX_CPU_COUNT: usize = 0x100;

// for debug
static INITIALIZED: Once<()> = Once::new();

static mut LOCAL_APIC_INFOS: [LocalApicInfo; MAX_CPU_COUNT] =
    [LocalApicInfo::empty(); MAX_CPU_COUNT];
static mut IO_APIC_INFOS: [IoApicInfo; MAX_CPU_COUNT] = [IoApicInfo::empty(); MAX_CPU_COUNT];
static mut APIC_ID_IDX_MAP: [usize; MAX_CPU_COUNT * 2] = [0; MAX_CPU_COUNT * 2];
static mut IDX_APIC_ID_MAP: [u8; MAX_CPU_COUNT] = [0; MAX_CPU_COUNT];

static mut APIC_COUNT: usize = 0;

#[derive(Debug, Clone, Copy)]
pub struct LocalApicInfo {
    pub processor_id: u8,
    pub apic_id: u8,
    pub flags: u32,
}

impl LocalApicInfo {
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

#[derive(Debug, Clone, Copy)]
pub struct IoApicInfo {
    pub io_apic_id: u8,
    pub io_apic_address: u32,
    pub global_system_interrupt_base: u32,
}

impl IoApicInfo {
    const fn empty() -> Self {
        Self {
            io_apic_address: 0,
            io_apic_id: 0,
            global_system_interrupt_base: 0,
        }
    }

    const fn new(io_apic_id: u8, io_apic_address: u32, global_system_interrupt_base: u32) -> Self {
        Self {
            io_apic_id,
            io_apic_address,
            global_system_interrupt_base,
        }
    }
}

pub fn init() {
    let madt = get_madt();

    let mut local_apic_infos = [LocalApicInfo::empty(); MAX_CPU_COUNT];
    let mut io_apic_infos = [IoApicInfo::empty(); MAX_CPU_COUNT];
    let mut apic_id_idx_map = [0; MAX_CPU_COUNT * 2];
    let mut idx_apic_id_map = [0; MAX_CPU_COUNT];

    let mut local_apic_count = 0;
    let mut io_apic_count = 0;

    for madt_entry in madt.entries() {
        if local_apic_count >= local_apic_infos.len() || io_apic_count >= io_apic_infos.len() {
            break;
        }

        if let MadtEntry::LocalApic(local_apic_entry) = madt_entry {
            debug_assert!((local_apic_entry.apic_id as usize) < apic_id_idx_map.len());

            local_apic_infos[local_apic_count] = LocalApicInfo::new(
                local_apic_entry.processor_id,
                local_apic_entry.apic_id,
                local_apic_entry.flags,
            );

            apic_id_idx_map[local_apic_entry.apic_id as usize] = local_apic_count;
            idx_apic_id_map[local_apic_count] = local_apic_entry.apic_id;

            local_apic_count += 1;
            continue;
        }

        if let MadtEntry::IoApic(io_apic_entry) = madt_entry {
            io_apic_infos[io_apic_count] = IoApicInfo::new(
                io_apic_entry.io_apic_id,
                io_apic_entry.io_apic_address,
                io_apic_entry.global_system_interrupt_base,
            );

            io_apic_count += 1;
            continue;
        }
    }

    unsafe {
        addr_of_mut!(LOCAL_APIC_INFOS).write(local_apic_infos);
        addr_of_mut!(IO_APIC_INFOS).write(io_apic_infos);
        addr_of_mut!(APIC_ID_IDX_MAP).write(apic_id_idx_map);
        addr_of_mut!(IDX_APIC_ID_MAP).write(idx_apic_id_map);
        *addr_of_mut!(APIC_COUNT) = local_apic_count;
        INITIALIZED.call_once(|| {});
    }
}

pub fn get_local_apic_id() -> u8 {
    let apic_base = unsafe { read_msr(IA32_APIC_BASE_MSR) } & 0xFFFF_F000;
    let id_reg = unsafe { mmio_read_u32(apic_base + 0x20) };
    ((id_reg >> 24) & 0xFF) as u8
}

pub fn get_local_apic_info_by_idx(idx: usize) -> LocalApicInfo {
    debug_assert!(INITIALIZED.is_completed());

    let infos = unsafe { &*addr_of!(LOCAL_APIC_INFOS) };

    debug_assert!(infos.len() > idx);

    unsafe { *infos.get_unchecked(idx) }
}

pub fn get_io_apic_info_by_idx(idx: usize) -> IoApicInfo {
    debug_assert!(INITIALIZED.is_completed());

    let infos = unsafe { &*addr_of!(IO_APIC_INFOS) };

    debug_assert!(infos.len() > idx);

    unsafe { *infos.get_unchecked(idx) }
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
