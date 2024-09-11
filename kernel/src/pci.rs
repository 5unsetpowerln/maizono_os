use core::arch::asm;

use common::arrayvec::ArrayVec;
use once_cell::sync::{Lazy, OnceCell};

use crate::error::Result;

// Address of CONFIG_ADDRESS register in IO Address Space
const CONFIG_ADDRESS_ADDRESS: u16 = 0x0cf8;
// Address of CONFIG_DATA register in IO Address Space
const CONFIG_DATA_ADDRESS: u16 = 0x0cfc;

const DEVICE_CAPACITY: usize = 32;
static mut DEVICES: Lazy<ArrayVec<Device, DEVICE_CAPACITY>> = Lazy::new(|| ArrayVec::new());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PciError {
    DeviceCapacityError,
    UninitializedError,
}

#[derive(Clone, Copy, Debug)]
pub struct Device {
    bus: u8,
    device: u8,
    function: u8,
    header_type: u8,
}

impl Device {
    fn new(bus: u8, device: u8, function: u8, header_type: u8) -> Self {
        Self {
            bus,
            device,
            function,
            header_type,
        }
    }
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

pub fn get_devices() -> Result<ArrayVec<Device, DEVICE_CAPACITY>> {
    unsafe {
        let a = &DEVICES.clone();
        // let a = DEVICES.ok_or(PciError::UninitializedError)?.clone();
        // let a = DEVICES.clone();
    }
    todo!()
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

    add_device(&Device::new(bus, device, func, header_type))?;

    let class_code = read_class_code(bus, device, func);
    let base = (class_code >> 24) & 0xff;
    let sub = (class_code >> 16) & 0xff;

    if base == 0x06 && sub == 0x04 {
        // standard PCI-PCI bridge
        let bus_numbers = read_bus_numbers(bus, device, func);
        let secondary_bus = (bus_numbers >> 8) & 0xff;
        scan_bus(secondary_bus as u8)?;
    }

    Ok(())
}

fn add_device(device: &Device) -> Result<()> {
    match unsafe { DEVICES.push(*device) } {
        Ok(_) => Ok(()),
        Err(_) => return Err(PciError::DeviceCapacityError.into()),
    }
}

fn is_single_function_device(header_type: u8) -> bool {
    (header_type & 0b10000000) == 0
}

// generates 32bit address for CONFIG_ADDRESS Register
fn make_address(bus: u8, device: u8, func: u8, register_addr: u8) -> u32 {
    // bit left
    let shl = |x: u32, bits: usize| return x << bits;
    shl(1, 31) // Bit to enable transporting CONFIG_DATA io to PCI Configuration Space (1bit)
        | shl(bus as u32, 16) // Bus number (8bit)
        | shl(device as u32, 11) // Device number (5 bit)
        | shl(func as u32, 8) // Function number (3 bit)
        | (register_addr as u32 & 0xfc) // Register offset (8bit) (2bit aligned)
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
fn read_class_code(bus: u8, device: u8, func: u8) -> u32 {
    write_address(make_address(bus, device, func, 0x08));
    read_data() >> 8
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
