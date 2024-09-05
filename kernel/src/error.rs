use thiserror_no_std::Error;

use crate::graphic::framebuffer;

#[derive(Error, Debug)]
pub enum Error {
    FrameBufferError(framebuffer::Error),
}
