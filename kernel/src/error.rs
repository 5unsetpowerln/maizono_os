use crate::graphic::{console::ConsoleError, framebuffer::FrameBufferError, mouse::MouseError};

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    FrameBufferError(FrameBufferError),
    ConsoleError(ConsoleError),
    MouseError(MouseError),
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

pub type Result<T> = core::result::Result<T, Error>;
