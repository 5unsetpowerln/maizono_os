// use super::usb::memory::MemoryError;

use thiserror_no_std::Error;

use super::usb::error::UsbError;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PciError {
    DeviceCapacityError,
    UninitializedError,
    DeviceLockError,
    BaseAddressRegisterIndexOutOfRangeError,
    #[error(transparent)]
    UsbError(#[from] UsbError),
}
