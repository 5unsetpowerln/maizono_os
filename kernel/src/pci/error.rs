// use super::usb::memory::MemoryError;

use thiserror_no_std::Error;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum PciError {
    DeviceCapacityError,
    UninitializedError,
    DeviceLockError,
    BaseAddressRegisterIndexOutOfRangeError,
}
