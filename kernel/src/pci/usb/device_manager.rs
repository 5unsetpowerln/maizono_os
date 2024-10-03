use common::address::AlignedAddress64;
use thiserror_no_std::Error;
use xhci::{context::Device32Byte, Registers};

use super::{error::UsbResult, memory::alloc_array};

#[derive(Debug, Clone, PartialEq, Error)]
pub enum DeviceManagerError {
    #[error("Unsupported device context size (only 32bit is supported).")]
    UnsupportedDeviceContextSize,
}

#[repr(C, align(64))]
pub struct DeviceContext(Device32Byte);

pub struct DeviceManager {
    device_context_pointers_ptr: AlignedAddress64, // can be used as DCBAAP's value.
    max_slots: usize,
    // device: Device
}

impl DeviceManager {
    pub fn new(max_slots: usize) -> Self {
        Self {
            max_slots,
            device_context_pointers_ptr: AlignedAddress64::new(0).unwrap(),
        }
    }

    pub fn init(&mut self) -> UsbResult<()> {
        // let is_device_context_size_64_bit = registers
        //     .capability
        //     .hccparams1
        //     .read_volatile()
        //     .context_size();
        // if is_device_context_size_64_bit {
        //     return Err(DeviceManagerError::UnsupportedDeviceContextSize.into());
        // }

        let device_context_pointers_ptr =
            alloc_array::<DeviceContext>(self.max_slots as usize + 1, 64, 4096)?;
        self.device_context_pointers_ptr = device_context_pointers_ptr;

        Ok(())
    }

    pub fn device_context_pointers_ptr(&self) -> AlignedAddress64 {
        self.device_context_pointers_ptr
    }
}
