use thiserror_no_std::Error;

use crate::framebuffer;

#[derive(Error, Debug)]
pub enum Error {
    FrameBufferError(framebuffer::Error),
}
