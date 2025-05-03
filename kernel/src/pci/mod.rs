pub mod error;

use core::{arch::asm, marker::PhantomData};

use arrayvec::ArrayVec;
use error::PciError;
use spin::{Mutex, MutexGuard};

use crate::error::Result;

/// Address of CONFIG_ADDRESS register in IO Address Space
const CONFIG_ADDRESS_ADDRESS: u16 = 0x0cf8;
/// Address of CONFIG_DATA register in IO Address Space
const CONFIG_DATA_ADDRESS: u16 = 0x0cfc;

const DEVICE_CAPACITY: usize = 32;
static DEVICES: Mutex<Devices<DEVICE_CAPACITY>> = Mutex::new(Devices::new());

#[derive(Clone, Copy, Debug)]
pub struct Device {
    bus: u8,
    device: u8,
    func: u8,
    header_type: u8,
    class_code: ClassCode,
}

impl Device {
    fn new(bus: u8, device: u8, func: u8, header_type: u8, class_code: &ClassCode) -> Self {
        Self {
            bus,
            device,
            func,
            header_type,
            class_code: *class_code,
        }
    }

    pub fn vendor_id(&self) -> u16 {
        read_vendor_id(self.bus, self.device, self.func)
    }

    pub fn is_xhc(&self) -> bool {
        self.class_code.is_match_all(0x0c, 0x03, 0x30)
    }

    pub fn is_intel(&self) -> bool {
        self.vendor_id() == 0x8086
    }

    pub fn get_bus(&self) -> u8 {
        self.bus
    }

    pub fn get_device(&self) -> u8 {
        self.device
    }

    pub fn get_func(&self) -> u8 {
        self.func
    }

    pub fn get_class_code(&self) -> ClassCode {
        self.class_code
    }

    pub fn get_header_type(&self) -> u8 {
        self.header_type
    }

    fn read_pci_config_space(&self, offset_in_pci_config_space: u8) -> u32 {
        write_address(make_address(
            self.bus,
            self.device,
            self.func,
            offset_in_pci_config_space,
        ));
        read_data()
    }

    pub fn read_base_addr(&self, index: usize) -> Result<u64> {
        if index >= 6 {
            return Err(PciError::BaseAddressRegisterIndexOutOfRangeError.into());
        }

        let offset_in_pci_config_space = (0x10 + 4 * index) as u8;
        let lower_base_addr = self.read_pci_config_space(offset_in_pci_config_space);

        // According to following reference, 1 and 2 bit in a base address register is flag about type.
        // If 2 bit is enabled, stored address is 64bit.
        // PCI Local Bus Specification Revision 3.0 (https://lekensteyn.nl/files/docs/PCI_SPEV_V3_0.pdf)

        // if address is 32bit
        if (lower_base_addr & 4) == 0 {
            return Ok(lower_base_addr as u64);
        }

        // if address is 64bit
        if index >= 5 {
            return Err(PciError::BaseAddressRegisterIndexOutOfRangeError.into());
        }
        let upper_base_addr = self.read_pci_config_space(offset_in_pci_config_space + 4) as u64;

        Ok(upper_base_addr << 32 | lower_base_addr as u64)
    }
}

// fn read_config_register

// index can be 0 ~ 5
// fn calc_base_addr_register_address(index: usize) -> u8 {
//     (0x10 + 4 * index) as u8
// }

#[derive(Clone, Copy, Debug)]
pub struct ClassCode {
    base: u8,
    sub: u8,
    interface: u8,
}

impl ClassCode {
    const fn new(base: u8, sub: u8, interface: u8) -> Self {
        Self {
            base,
            sub,
            interface,
        }
    }

    fn is_match_base(&self, b: u8) -> bool {
        self.base == b
    }

    fn is_match_base_sub(&self, b: u8, s: u8) -> bool {
        self.is_match_base(b) && self.sub == s
    }

    fn is_match_all(&self, b: u8, s: u8, i: u8) -> bool {
        self.is_match_base_sub(b, s) && self.interface == i
    }

    pub fn get_base(&self) -> u8 {
        self.base
    }

    pub fn get_sub(&self) -> u8 {
        self.sub
    }

    pub fn get_interface(&self) -> u8 {
        self.interface
    }
}

pub struct Devices<'a, const CAP: usize> {
    array: ArrayVec<Device, CAP>,
    _marker: PhantomData<&'a ()>,
}

impl<'a, const CAP: usize> Devices<'a, CAP> {
    const fn new() -> Self {
        Self {
            array: ArrayVec::<Device, CAP>::new_const(),
            _marker: PhantomData,
        }
    }

    pub fn as_ref_inner(&'a self) -> &'a ArrayVec<Device, CAP> {
        &self.array
    }

    /// Initialize devices. Scan all devices and store them.
    pub fn init(&mut self) -> Result<()> {
        self.scan_all_bus()?;
        Ok(())
    }

    #[inline]
    fn add_device(&mut self, device: Device) {
        self.array.push(device);
    }

    fn scan_all_bus(&mut self) -> Result<()> {
        let bus0_host_bridge_header_type = read_header_type(0, 0, 0);
        if is_single_function_device(bus0_host_bridge_header_type) {
            return self.scan_bus(0);
        }

        for func in 0..8 {
            if read_vendor_id(0, 0, func) == 0xffff {
                continue;
            }
            self.scan_bus(func)?;
        }

        Ok(())
    }

    fn scan_bus(&mut self, bus: u8) -> Result<()> {
        for device in 0..32 {
            if read_vendor_id(bus, device, 0) == 0xffff {
                continue;
            }
            self.scan_device(bus, device)?;
        }
        Ok(())
    }

    fn scan_device(&mut self, bus: u8, device: u8) -> Result<()> {
        self.scan_function(bus, device, 0)?;

        if is_single_function_device(read_header_type(bus, device, 0)) {
            return Ok(());
        }

        for func in 1..8 {
            if read_vendor_id(bus, device, func) == 0xffff {
                continue;
            }

            self.scan_function(bus, device, func)?;
        }
        Ok(())
    }

    fn scan_function(&mut self, bus: u8, device: u8, func: u8) -> Result<()> {
        let header_type = read_header_type(bus, device, func);
        let class_code = read_class_code(bus, device, func);

        self.add_device(Device::new(bus, device, func, header_type, &class_code));

        if class_code.base == 0x06 && class_code.sub == 0x04 {
            // standard PCI-PCI bridge
            let bus_numbers = read_bus_numbers(bus, device, func);
            let secondary_bus = (bus_numbers >> 8) & 0xff;
            self.scan_bus(secondary_bus as u8)?;
        }

        Ok(())
    }
}

pub fn devices() -> Result<MutexGuard<'static, Devices<'static, DEVICE_CAPACITY>>> {
    DEVICES.try_lock().ok_or(PciError::DeviceLockError.into())
}

fn is_single_function_device(header_type: u8) -> bool {
    (header_type & 0b10000000) == 0
}

// generates 32bit address for CONFIG_ADDRESS Register
fn make_address(bus: u8, device: u8, func: u8, offset_in_pci_config_space: u8) -> u32 {
    // bit left
    let shl = |x: u32, bits: usize| return x << bits;
    shl(1, 31) // Bit to enable transporting CONFIG_DATA io to PCI Configuration Space (1bit)
        | shl(bus as u32, 16) // Bus number (8bit)
        | shl(device as u32, 11) // Device number (5 bit)
        | shl(func as u32, 8) // Function number (3 bit)
        | (offset_in_pci_config_space as u32 & 0xfc) // Register offset (8bit) (2bit aligned)
}

/// writes an address of PCI Configuration Space to CONFIG_ADDRESS register to read/write it via CONFIG_DATA register.
fn write_address(addr: u32) {
    io_out_32(CONFIG_ADDRESS_ADDRESS, addr);
}

/// writes a data to the PCI Configuration Space which is specified at CONFIG_ADDRESS register.
fn write_data(data: u32) {
    io_out_32(CONFIG_DATA_ADDRESS, data);
}

/// reads a data from the PCI Configuration Space which is specified at CONFIG_ADDRESS register.
fn read_data() -> u32 {
    io_in_32(CONFIG_DATA_ADDRESS)
}

// functions to read informations from PCI Configuration Space
/// Length of Vendor ID is 16 bit.
fn read_vendor_id(bus: u8, device: u8, func: u8) -> u16 {
    write_address(make_address(bus, device, func, 0x00));
    (read_data() & 0xffff) as u16
}

/// Length of Device ID is 16 bit.
fn read_device_id(bus: u8, device: u8, func: u8) -> u16 {
    write_address(make_address(bus, device, func, 0x00));
    (read_data() >> 16) as u16
}

/// Length of Header Type is 8 bit.
fn read_header_type(bus: u8, device: u8, func: u8) -> u8 {
    write_address(make_address(bus, device, func, 0x0c));
    ((read_data() >> 16) & 0xff) as u8
}

/// Length of Class Code is 24 bit.
fn read_class_code(bus: u8, device: u8, func: u8) -> ClassCode {
    write_address(make_address(bus, device, func, 0x08));
    let class_code_raw = read_data() >> 8;
    let base = ((class_code_raw >> 16) & 0xff) as u8;
    let sub = ((class_code_raw >> 8) & 0xff) as u8;
    let interface = (class_code_raw & 0xff) as u8;

    ClassCode::new(base, sub, interface)
}

fn read_bus_numbers(bus: u8, device: u8, func: u8) -> u32 {
    write_address(make_address(bus, device, func, 0x18));
    read_data()
}

fn io_out_32(addr: u16, data: u32) {
    unsafe {
        asm!(
            "out dx, eax",  // *dx (IO Address Space) = eax
            in("dx") addr,
            in("eax") data,
        );
    }
}

fn io_in_32(addr: u16) -> u32 {
    let mut data: u32;
    unsafe {
        asm!(
            "in eax, dx", // eax = *dx (IO Address Space)
            in("dx") addr,
            out("eax") data
        )
    }
    data
}
