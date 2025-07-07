use core::{
    marker::PhantomData,
    mem::MaybeUninit,
    pin::{Pin, pin},
    ptr::read_unaligned,
};

use acpi::{
    AcpiError,
    fadt::Fadt,
    madt::{IoApicEntry, LocalApicEntry, Madt},
    rsdp::Rsdp,
    sdt::SdtHeader,
};
use common::address::PhysPtr;
use log::{debug, error, info};
use spin::{Mutex, Once};
use x86_64::instructions::port::{Port, PortReadOnly};

trait Validate {
    fn is_valid(&self) -> bool;
}

impl Validate for Rsdp {
    fn is_valid(&self) -> bool {
        if let Err(err) = self.validate() {
            match err {
                AcpiError::RsdpIncorrectSignature => {
                    error!("invalid signature: {:?}", self.signature());
                    return false;
                }
                AcpiError::RsdpInvalidOemId => {
                    error!("invalid oem id: {}", self.oem_id());
                    return false;
                }
                AcpiError::RsdpInvalidChecksum => {
                    error!("invalid checksum.");
                    return false;
                }
                _ => {
                    error!("unreachable!");
                    panic!();
                }
            }
        }

        if self.revision() != 2 {
            debug!("ACPI revision must be 2: {}", self.revision());
            return false;
        }

        true
    }
}

#[repr(C, packed(1))]
pub struct Xsdt {
    header: SdtHeader,
}

impl Xsdt {
    fn count(&self) -> usize {
        (self.header.length as usize - size_of::<SdtHeader>()) / size_of::<u64>()
    }

    unsafe fn get(&self, i: usize) -> &SdtHeader {
        // following pointer is NOT 8byte-aligned
        let entry_ptrs = (&self.header as *const SdtHeader).add(1) as *const u64;
        let entry_ptr = read_unaligned(entry_ptrs.add(i)) as *const SdtHeader;
        &*entry_ptr
    }

    fn is_valid(&self) -> bool {
        if let Err(err) = self.header.validate(acpi::sdt::Signature::XSDT) {
            match err {
                AcpiError::SdtInvalidSignature(invalid_signature) => {
                    error!("invalid signature: {}", invalid_signature);
                    false
                }
                AcpiError::SdtInvalidOemId(_) => {
                    error!("invalid oem id");
                    false
                }
                AcpiError::SdtInvalidTableId(_) => {
                    error!("invalid table id");
                    false
                }
                AcpiError::SdtInvalidChecksum(_) => {
                    error!("invalid checksum:");
                    false
                }
                _ => {
                    error!("unreachable!");
                    panic!();
                }
            }
        } else {
            true
        }
    }
}

pub struct ApicInfo {
    local_apic_base: u32,
    local_apic: LocalApicEntry,
    io_apic: IoApicEntry,
}

impl ApicInfo {
    fn from_madt(madt: Pin<&Madt>) -> Self {
        // Find the entry about I/O APIC from the MADT.
        let io_apic_entry = madt
            .entries()
            .find_map(|entry| {
                if let acpi::madt::MadtEntry::IoApic(o) = entry {
                    Some(o)
                } else {
                    None
                }
            })
            .expect("The entry about the I/O APIC wasn't found from the MADT");

        // Find the entry about Local APIC from the MADT.
        let local_apic_entry = madt
            .entries()
            .find_map(|entry| {
                if let acpi::madt::MadtEntry::LocalApic(o) = entry {
                    Some(o)
                } else {
                    None
                }
            })
            .expect("The entry about the Local APIC wasn't found from the MADT");

        // Get from the base address of the Local APIC from the MADT.
        let local_apic_base = madt.local_apic_address;

        return Self {
            local_apic_base,
            local_apic: *local_apic_entry,
            io_apic: *io_apic_entry,
        };
    }

    #[inline]
    pub fn local_apic_base(&self) -> u32 {
        self.local_apic_base
    }

    #[inline]
    pub fn io_apic_base(&self) -> u32 {
        self.io_apic.io_apic_address
    }

    #[inline]
    pub fn io_apic_id(&self) -> u8 {
        self.io_apic.io_apic_id
    }

    pub fn processor_id(&self) -> u8 {
        self.local_apic.processor_id
    }
}

// static FADT: Once<Fadt> = Once::new();
static FADT: Once<Mutex<Fadt>> = Once::new();
static APIC_INFO: Once<ApicInfo> = Once::new();

pub unsafe fn init(rsdp: &'static Rsdp) {
    // let rsdp = unsafe { rsdp_addr.ref_::<Rsdp>() };
    // if !rsdp.is_valid() {
    //     error!("RSDP isn't valid.");
    //     panic!();
    // }

    let xsdt = unsafe { &*(rsdp.xsdt_address() as *const Xsdt) };
    if !xsdt.is_valid() {
        error!("XSDT is not valid.");
        panic!();
    }

    // Find FADT and MADT from XSDT
    let mut fadt_ptr = 0 as *const Fadt;
    let mut madt_ptr = 0 as *const Madt;

    for i in 0..xsdt.count() {
        let entry = unsafe { xsdt.get(i) };

        if entry.validate(acpi::sdt::Signature::FADT).is_ok() {
            fadt_ptr = entry as *const SdtHeader as *const Fadt;
        }

        if entry.validate(acpi::sdt::Signature::MADT).is_ok() {
            madt_ptr = entry as *const SdtHeader as *const Madt;
        }
    }

    if fadt_ptr.is_null() {
        error!("FADT isn't found.");
        panic!();
    }

    if madt_ptr.is_null() {
        error!("MADT isn't found.");
        panic!();
    }

    let madt = unsafe { Pin::new_unchecked(&*madt_ptr) };

    info!("FADT is found: 0x{:X}", fadt_ptr as u64);
    FADT.call_once(|| unsafe { Mutex::new(*fadt_ptr) });

    info!("MADT is found: 0x{:X}", madt_ptr as u64);
    APIC_INFO.call_once(|| ApicInfo::from_madt(madt));
}

const PM_TIMER_FREQ: u32 = 3579545;

pub fn wait_milli_secs(msec: u32) {
    let fadt = FADT.wait().lock();
    let fadt_flags = fadt.flags;
    let is_pm_timer_32_bit = fadt_flags.pm_timer_is_32_bit();

    let io_addr = fadt
        .pm_timer_block()
        .expect("Failed to get ACPI PM timer IO port address.")
        .expect("ACPI PM timer IO port is empty.")
        .address as u16;

    let mut io = PortReadOnly::<u32>::new(io_addr);
    let start = unsafe { io.read() };
    let mut end = start + PM_TIMER_FREQ * msec / 1000;

    if !is_pm_timer_32_bit {
        end &= 0x00ffffff;
    }

    if end < start {
        // オーバーフローしてたらカウントが0になるまで (すなわちstart以上の間) は待機する
        while unsafe { io.read() } >= start {}
    }
    while unsafe { io.read() } < end {}
}

// pub fn get_fadt() -> MutexGuard<'_, Fadt> {
//     let a = FADT.wait().lock();
//     // FADT.lock();
//     // let fadt = &*(FADT.lock().as_ptr());
//     // FADT.get()
//     //     .expect("acpi::get_fadt is called before calling acpi::init.")
// }

pub fn get_apic_info() -> &'static ApicInfo {
    APIC_INFO
        .get()
        .expect("acpi::get_apic_info is called before calling acpi::init.")
}
