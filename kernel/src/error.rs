use thiserror_no_std::Error;

use crate::{
    graphic::{console::ConsoleError, frame_buffer::FrameBufferError},
    pci::error::PciError,
};

#[derive(Debug, Clone, PartialEq, Error)]
pub enum Error {
    FrameBufferError(FrameBufferError),
    ConsoleError(ConsoleError),
    #[error(transparent)]
    PciError(#[from] PciError),
}

impl From<FrameBufferError> for Error {
    fn from(err: FrameBufferError) -> Self {
        Self::FrameBufferError(err)
    }
}

impl From<ConsoleError> for Error {
    fn from(err: ConsoleError) -> Self {
        Self::ConsoleError(err)
    }
}

// impl From<PciError> for Error {
//     fn from(err: PciError) -> Self {
//         Self::PciError(err)
//     }
// }

pub type Result<T> = core::result::Result<T, Error>;
