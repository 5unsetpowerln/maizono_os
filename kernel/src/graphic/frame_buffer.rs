use crate::error::Result;
use alloc::sync::Arc;
use common::graphic::{GraphicInfo, PixelFormat, RgbColor};
use glam::U64Vec2;
use spin::{Mutex, Once};
use thiserror_no_std::Error;

use super::PixelWriter;

pub static FRAME_BUFFER: Once<Arc<Mutex<FrameBuffer>>> = Once::new();
pub static FRAME_BUFFER_WIDTH: Once<usize> = Once::new();
pub static FRAME_BUFFER_HEIGHT: Once<usize> = Once::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FrameBufferError {
    #[error("Unsupported pixel format.")]
    UnsupportedPixelFormatError,
    #[error("The frame buffer is not initialized yet.")]
    NotInitializedError,
    #[error("Attempted to write a pixel to outside of frame buffer.")]
    OutsideBufferError,
    #[error("Failed to lock the frame buffer.")]
    FrameBufferLockError,
    #[error("Unsupported character.")]
    UnsupportedCharacterError,
}

#[derive(Clone, Debug)]
pub struct FrameBuffer {
    width: u64,
    height: u64,
    bytes_per_pixel: u64,
    stride: usize,
    pixel_format: PixelFormat,
    framebuf_addr: u64,
    framebuf_size: usize,
    write_pixel: fn(&mut FrameBuffer, U64Vec2, RgbColor) -> Result<()>,
}

impl FrameBuffer {
    const fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            bytes_per_pixel: 0,
            stride: 0,
            pixel_format: PixelFormat::Bgr,
            framebuf_addr: 0,
            framebuf_size: 0,
            write_pixel: write_pixel_bgr,
        }
    }

    fn is_inside_buffer(&self, position: U64Vec2) -> bool {
        !(position.x >= self.width || position.y >= self.height)
    }

    pub fn init(&mut self, graphic_info: &GraphicInfo, bg_color: RgbColor) -> Result<()> {
        *self = Self {
            width: graphic_info.width,
            height: graphic_info.height,
            bytes_per_pixel: graphic_info.bytes_per_pixel,
            stride: graphic_info.stride,
            pixel_format: graphic_info.pixel_format,
            framebuf_addr: graphic_info.frame_buffer_addr,
            framebuf_size: graphic_info.size,
            write_pixel: match graphic_info.pixel_format {
                PixelFormat::Rgb => write_pixel_rgb,
                PixelFormat::Bgr => write_pixel_bgr,
            },
        };
        self.fill(bg_color)?;
        Ok(())
    }
}

impl PixelWriter for FrameBuffer {
    fn write_pixel(&mut self, position: U64Vec2, pixel: RgbColor) -> Result<()> {
        (self.write_pixel)(self, position, pixel)
    }

    fn width(&self) -> u64 {
        self.width
    }

    fn height(&self) -> u64 {
        self.height
    }
}

fn write_pixel_rgb(self_: &mut FrameBuffer, position: U64Vec2, pixel: RgbColor) -> Result<()> {
    if !self_.is_inside_buffer(position) {
        return Err(FrameBufferError::OutsideBufferError.into());
    }
    let offset = (position.y * self_.width + position.x) * self_.bytes_per_pixel;
    let pixel_ref = (self_.framebuf_addr + offset as u64) as *mut u32;

    unsafe {
        *pixel_ref = pixel.le();
    };
    Ok(())
}

fn write_pixel_bgr(self_: &mut FrameBuffer, position: U64Vec2, mut pixel: RgbColor) -> Result<()> {
    if !self_.is_inside_buffer(position) {
        return Err(FrameBufferError::OutsideBufferError.into());
    }

    let offset = (position.y * self_.width + position.x) * self_.bytes_per_pixel;
    let pixel_ref = (self_.framebuf_addr + offset as u64) as *mut u32;
    pixel.bgr();

    unsafe {
        *pixel_ref = pixel.le();
    };
    Ok(())
}

pub fn init(graphic_info: &GraphicInfo, bg_color: RgbColor) -> Result<()> {
    let mut frame_buffer = FrameBuffer::new();
    frame_buffer
        .init(graphic_info, bg_color)
        .expect("Failed to construct the FrameBuffer.");

    FRAME_BUFFER_WIDTH.call_once(|| frame_buffer.width() as usize);
    FRAME_BUFFER_HEIGHT.call_once(|| frame_buffer.height() as usize);
    FRAME_BUFFER.call_once(|| Arc::new(Mutex::new(frame_buffer)));
    Ok(())
}
