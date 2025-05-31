use core::ptr;

use crate::{error::Result, serial_println};
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
    pixel_format: PixelFormat,
    buffer_addr: u64,
    buffer_size: u64,
    write_pixel: fn(&mut FrameBuffer, U64Vec2, RgbColor) -> Result<()>,
}

impl FrameBuffer {
    pub const fn new_empty() -> Self {
        Self {
            width: 0,
            height: 0,
            pixel_format: PixelFormat::Bgr,
            buffer_addr: 0,
            buffer_size: 0,
            write_pixel: write_pixel_bgr,
        }
    }

    fn is_inside_buffer(&self, position: U64Vec2) -> bool {
        !(position.x >= self.width || position.y >= self.height)
    }

    pub fn new(width: u64, height: u64, buffer_ptr: *mut u32) -> Self {
        Self {
            width,
            height,
            pixel_format: PixelFormat::Bgr,
            buffer_addr: buffer_ptr as u64,
            buffer_size: width * height * 4,
            write_pixel: write_pixel_bgr,
        }
    }

    pub fn from_graphic_info(&mut self, graphic_info: &GraphicInfo) {
        if graphic_info.bytes_per_pixel != 4 {
            panic!("Unsupported pixel size: {}", graphic_info.bytes_per_pixel);
        }

        if graphic_info.frame_buffer_size as u64 != graphic_info.width * graphic_info.height * 4 {
            panic!(
                "invalid size: graphic_info.frame_buffer_size != graphic_info.width * graphic_info.height * graphic_info.bytes_per_pixel"
            );
        }

        serial_println!(
            "size: {}, width * height * bytes_per_pixel: {}",
            graphic_info.frame_buffer_size,
            graphic_info.width * graphic_info.height * graphic_info.bytes_per_pixel
        );

        *self = Self {
            width: graphic_info.width,
            height: graphic_info.height,
            pixel_format: graphic_info.pixel_format,
            buffer_addr: graphic_info.frame_buffer_addr,
            buffer_size: graphic_info.frame_buffer_size as u64,
            write_pixel: match graphic_info.pixel_format {
                PixelFormat::Bgr => write_pixel_bgr,
                PixelFormat::Rgb => write_pixel_rgb,
            },
        };
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

    let offset = position.y * self_.width + position.x;
    let buffer_ptr = self_.buffer_addr as *const u32 as *mut u32;
    let ptr = unsafe { buffer_ptr.add(offset as usize) };
    unsafe { *ptr = pixel.le() };

    Ok(())
}

fn write_pixel_bgr(self_: &mut FrameBuffer, position: U64Vec2, mut pixel: RgbColor) -> Result<()> {
    if !self_.is_inside_buffer(position) {
        return Err(FrameBufferError::OutsideBufferError.into());
    }

    let offset = position.y * self_.width + position.x;
    let buffer_ptr = self_.buffer_addr as *const u32 as *mut u32;
    let ptr = unsafe { buffer_ptr.add(offset as usize) };
    pixel.bgr();
    unsafe { *ptr = pixel.le() };

    Ok(())
}

pub fn init(graphic_info: &GraphicInfo) -> Result<()> {
    let mut frame_buffer = FrameBuffer::new_empty();
    frame_buffer.from_graphic_info(graphic_info);

    FRAME_BUFFER_WIDTH.call_once(|| frame_buffer.width() as usize);
    FRAME_BUFFER_HEIGHT.call_once(|| frame_buffer.height() as usize);
    FRAME_BUFFER.call_once(|| Arc::new(Mutex::new(frame_buffer)));
    Ok(())
}
