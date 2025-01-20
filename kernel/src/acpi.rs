use core::{marker::PhantomData, ptr::read_unaligned};

use acpi::{
    fadt::Fadt,
    madt::{IoApicEntry, LocalApicEntry, Madt},
    rsdp::Rsdp,
    sdt::SdtHeader,
    AcpiError,
};
use common::address::PhysPtr;
use spin::{Mutex, Once};

use crate::printk;

trait Validate {
    fn is_valid(&self) -> bool;
}

impl Validate for Rsdp {
    fn is_valid(&self) -> bool {
        if let Err(err) = self.validate() {
            match err {
                AcpiError::RsdpIncorrectSignature => {
                    printk!("invalid signature: {:?}", self.signature());
                    return false;
                }
                AcpiError::RsdpInvalidOemId => {
                    printk!("invalid oem id: {}", self.oem_id());
                    return false;
                }
                AcpiError::RsdpInvalidChecksum => {
                    printk!("invalid checksum.");
                    return false;
                }
                _ => {
                    printk!("unreachable!");
                    panic!();
                }
            }
        }

        if self.revision() != 2 {
            printk!("ACPI revision must be 2: {}", self.revision());
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
                    printk!("invalid signature: {}", invalid_signature);
                    false
                }
                AcpiError::SdtInvalidOemId(_) => {
                    printk!("invalid oem id");
                    false
                }
                AcpiError::SdtInvalidTableId(_) => {
                    printk!("invalid table id");
                    false
                }
                AcpiError::SdtInvalidChecksum(_) => {
                    printk!("invalid checksum:");
                    false
                }
                _ => {
                    printk!("unreachable!");
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
    fn from_madt(madt: &Madt) -> Self {
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

static FADT: Once<Fadt> = Once::new();
static APIC_INFO: Once<ApicInfo> = Once::new();

pub unsafe fn init(rsdp_addr: PhysPtr) {
    let rsdp = unsafe { rsdp_addr.ref_::<Rsdp>() };
    if !rsdp.is_valid() {
        printk!("RSDP isn't valid.");
        panic!();
    }

    let xsdt = unsafe { &*(rsdp.xsdt_address() as *const Xsdt) };
    if !xsdt.is_valid() {
        printk!("XSDT is not valid.");
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
        printk!("FADT isn't found.");
        panic!();
    }

    if madt_ptr.is_null() {
        printk!("MADT isn't found.");
        panic!();
    }

    printk!("FADT is found: 0x{:X}", fadt_ptr as u64);
    FADT.call_once(|| *fadt_ptr);

    printk!("MADT is found: 0x{:X}", madt_ptr as u64);
    APIC_INFO.call_once(|| ApicInfo::from_madt(&*madt_ptr));
}

pub fn get_fadt() -> &'static Fadt {
    FADT.get()
        .expect("acpi::get_fadt is called before calling acpi::init.")
}

pub fn get_apic_info() -> &'static ApicInfo {
    APIC_INFO
        .get()
        .expect("acpi::get_apic_info is called before calling acpi::init.")
}
