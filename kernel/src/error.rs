use crate::{
    graphic::{console::ConsoleError, framebuffer::FrameBufferError, mouse::MouseError},
    pci::PciError,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    FrameBufferError(FrameBufferError),
    ConsoleError(ConsoleError),
    MouseError(MouseError),
    PciError(PciError),
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

impl From<MouseError> for Error {
    fn from(err: MouseError) -> Self {
        Self::MouseError(err)
    }
}

impl From<PciError> for Error {
    fn from(err: PciError) -> Self {
        Self::PciError(err)
    }
}

pub type Result<T> = core::result::Result<T, Error>;
