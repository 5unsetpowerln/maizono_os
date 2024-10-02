use thiserror_no_std::Error;

use super::{device_manager::DeviceManagerError, memory::MemoryError};

#[derive(Debug, Clone, PartialEq, Error)]
pub enum UsbError {
    #[error(transparent)]
    MemoryError(#[from] MemoryError),
    #[error(transparent)]
    DeviceManagerError(#[from] DeviceManagerError),
}

pub type UsbResult<T> = Result<T, UsbError>;
