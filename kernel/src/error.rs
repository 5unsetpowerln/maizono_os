use crate::graphic::{console::ConsoleError, framebuffer::FrameBufferError};

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    FrameBufferError(FrameBufferError),
    ConsoleError(ConsoleError),
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

pub type Result<T> = core::result::Result<T, Error>;
