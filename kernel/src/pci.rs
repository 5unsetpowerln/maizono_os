use core::arch::asm;

use arrayvec::ArrayVec;
// use common::arrayvec::ArrayVec;
use spin::{Mutex, MutexGuard};

use crate::{
    error::Result,
    printk,
    usb::{self, xhci::init_host_controller},
};

/// Address of CONFIG_ADDRESS register in IO Address Space
const CONFIG_ADDRESS_ADDRESS: u16 = 0x0cf8;
/// Address of CONFIG_DATA register in IO Address Space
const CONFIG_DATA_ADDRESS: u16 = 0x0cfc;

const DEVICE_CAPACITY: usize = 32;
type Devices = ArrayVec<Device, DEVICE_CAPACITY>;
static mut DEVICES: Mutex<Option<Devices>> = Mutex::new(None);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PciError {
    DeviceCapacityError,
    UninitializedError,
    DeviceLockError,
    BaseAddressRegisterIndexOutOfRangeError,
}

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
    fn new(base: u8, sub: u8, interface: u8) -> Self {
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
}

pub fn init() -> Result<()> {
    let devices = ArrayVec::<Device, DEVICE_CAPACITY>::new();
    lock_devices()?.replace(devices);
    Ok(())
}

pub fn scan_all_bus() -> Result<()> {
    let bus0_host_bridge_header_type = read_header_type(0, 0, 0);
    if is_single_function_device(bus0_host_bridge_header_type) {
        return scan_bus(0);
    }

    for func in 0..8 {
        if read_vendor_id(0, 0, func) == 0xffff {
            continue;
        }
        scan_bus(func)?;
    }

    Ok(())
}

fn lock_devices<'a>() -> Result<MutexGuard<'a, Option<Devices>>> {
    unsafe { DEVICES.try_lock() }.ok_or(PciError::DeviceLockError.into())
}

pub fn get_devices() -> Result<ArrayVec<Device, DEVICE_CAPACITY>> {
    if let Some(devices) = lock_devices()?.as_ref() {
        return Ok(devices.clone());
    } else {
        return Err(PciError::UninitializedError.into());
    }
}

fn scan_bus(bus: u8) -> Result<()> {
    for device in 0..32 {
        if read_vendor_id(bus, device, 0) == 0xffff {
            continue;
        }
        scan_device(bus, device)?;
    }
    Ok(())
}

fn scan_device(bus: u8, device: u8) -> Result<()> {
    scan_function(bus, device, 0)?;

    if is_single_function_device(read_header_type(bus, device, 0)) {
        return Ok(());
    }

    for func in 1..8 {
        if read_vendor_id(bus, device, func) == 0xffff {
            continue;
        }

        scan_function(bus, device, func)?;
    }
    Ok(())
}

fn scan_function(bus: u8, device: u8, func: u8) -> Result<()> {
    let header_type = read_header_type(bus, device, func);
    let class_code = read_class_code(bus, device, func);

    add_device(&Device::new(bus, device, func, header_type, &class_code))?;

    if class_code.base == 0x06 && class_code.sub == 0x04 {
        // standard PCI-PCI bridge
        let bus_numbers = read_bus_numbers(bus, device, func);
        let secondary_bus = (bus_numbers >> 8) & 0xff;
        scan_bus(secondary_bus as u8)?;
    }

    Ok(())
}

fn add_device(device: &Device) -> Result<()> {
    if let Some(devices) = lock_devices()?.as_mut() {
        devices.push(device.clone());
        Ok(())
    } else {
        Err(PciError::UninitializedError.into())
    }
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

pub fn xhci() -> Result<()> {
    scan_all_bus()?;
    let devices = get_devices()?;
    let mut xhci_device_opt = None;
    for device in devices.iter() {
        if device.class_code.is_match_all(0x0c, 0x03, 0x30) {
            xhci_device_opt.replace(device);
            break;
        }
    }

    if xhci_device_opt.is_none() {
        printk!("no xHCI device");
        return Ok(());
    }

    let xhci_device = xhci_device_opt.unwrap();
    printk!(
        "xHCI device: addr = {:X}:{:X}:{:X}",
        xhci_device.bus,
        xhci_device.device,
        xhci_device.func
    );

    let xhc_base_addr = xhci_device.read_base_addr(0)?;
    let xhc_mmio_base = xhc_base_addr & !(0xf as u64);
    printk!("xhc mmio base: 0x{:X}", xhc_mmio_base);

    let mut controller = unsafe { usb::xhci::Controller::new(xhc_mmio_base) };
    controller.init();
    // init_host_controller(xhc_mmio_base);

    // let a = xhci::Registers::

    // xhc = new usb::xhci::RealController{mmio_base};

    // if (auto err = xhc->Initialize(); err != usb::error::kSuccess)
    // {
    //     delete xhc;
    //     printk("failed to initialize xHCI controller: %d\n", err);
    //     return;
    // }

    // Configure MSI
    // let bsp_local_apic_id = {
    //     let ptr = (0xfee00020 as u64) as *const u32;
    //     unsafe {
    //         //
    //         *ptr >> 24
    //     }
    // };
    // printk!("bsp_local_apic_id: {}", bsp_local_apic_id);

    // let msi_msg_addr = 0xfee00000 | (bsp_local_apic_id << 12);

    Ok(())
}
