use crate::error::Result;
use alloc::{sync::Arc, vec::Vec};
use common::graphic::{GraphicInfo, PixelFormat, RgbColor};
use glam::U64Vec2;
use log::info;
use spin::{Mutex, Once};
use thiserror_no_std::Error;

use super::{PixelWriter, PixelWriterCopyable};

pub static FRAME_BUFFER: Once<Arc<Mutex<FrameBuffer>>> = Once::new();
pub static FRAME_BUFFER_WIDTH: Once<usize> = Once::new();
pub static FRAME_BUFFER_HEIGHT: Once<usize> = Once::new();
pub static PIXEL_FORMAT: Once<PixelFormat> = Once::new();
pub static BYTES_PER_PIXEL: Once<u64> = Once::new();

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
    graphic_info: GraphicInfo,
    buffer: Vec<u8>,
    write_pixel: fn(&mut FrameBuffer, U64Vec2, RgbColor) -> Result<()>,
}

impl FrameBuffer {
    pub const fn new_empty() -> Self {
        Self {
            graphic_info: GraphicInfo::new_empty(),
            buffer: Vec::new(),
            write_pixel: write_pixel_bgr,
        }
    }

    fn is_inside_buffer(&self, position: U64Vec2) -> bool {
        !(position.x >= self.graphic_info.width || position.y >= self.graphic_info.height)
    }

    pub fn init(&mut self, graphic_info: &GraphicInfo) {
        if graphic_info.bytes_per_pixel != 4 {
            panic!("Unsupported pixel size: {}", graphic_info.bytes_per_pixel);
        }

        let mut buffer = Vec::new();
        let mut graphic_info = graphic_info.clone();

        if graphic_info.frame_buffer_addr.is_some() {
            buffer.resize(0, 0);
        } else {
            buffer.resize(
                (graphic_info.width * graphic_info.height * graphic_info.bytes_per_pixel) as usize,
                0,
            );
            graphic_info
                .frame_buffer_addr
                .replace(buffer.as_ptr() as u64);
        }

        *self = Self {
            buffer,
            write_pixel: match graphic_info.pixel_format {
                PixelFormat::Bgr => write_pixel_bgr,
                PixelFormat::Rgb => write_pixel_rgb,
            },
            graphic_info,
        };
    }

    pub unsafe fn copy(&mut self, position: U64Vec2, src: &Self) {
        let dst_width = self.graphic_info.width;
        let dst_height = self.graphic_info.height;
        let src_width = src.graphic_info.width;
        let src_height = self.graphic_info.height;
        let copy_start_dst_x = position.x.max(0);
        let copy_start_dst_y = position.y.max(0);
        let copy_end_dst_x = (position.x + src_width).min(dst_width);
        let copy_end_dst_y = (position.y + src_height).min(dst_height);

        let bytes_per_copy_row =
            self.graphic_info.bytes_per_pixel * (copy_end_dst_x - copy_start_dst_x);

        let dst_addr = self.graphic_info.frame_buffer_addr.unwrap()
            + self.graphic_info.bytes_per_pixel
                * (self.graphic_info.width * copy_start_dst_y + copy_start_dst_x);
        let src_addr = src.graphic_info.frame_buffer_addr.unwrap();

        let mut dst_ptr = dst_addr as *mut u8;
        let mut src_ptr = src_addr as *const u8;

        for _ in 0..copy_end_dst_y - copy_start_dst_y {
            unsafe {
                dst_ptr.copy_from_nonoverlapping(src_ptr, bytes_per_copy_row as usize);
                dst_ptr = dst_ptr
                    .add((self.graphic_info.bytes_per_pixel * self.graphic_info.width) as usize);
                src_ptr = src_ptr
                    .add((self.graphic_info.bytes_per_pixel * src.graphic_info.width) as usize);
            }
        }
    }
}

impl PixelWriter for FrameBuffer {
    fn write_pixel(&mut self, position: U64Vec2, pixel: RgbColor) -> Result<()> {
        (self.write_pixel)(self, position, pixel)
    }

    fn width(&self) -> u64 {
        self.graphic_info.width
    }

    fn height(&self) -> u64 {
        self.graphic_info.height
    }
}

fn write_pixel_rgb(self_: &mut FrameBuffer, position: U64Vec2, pixel: RgbColor) -> Result<()> {
    if !self_.is_inside_buffer(position) {
        return Err(FrameBufferError::OutsideBufferError.into());
    }

    let offset =
        (position.y * self_.graphic_info.width + position.x) * self_.graphic_info.bytes_per_pixel;
    let dst_addr = self_.graphic_info.frame_buffer_addr.unwrap() + offset;
    let dst_ptr = dst_addr as *mut u8 as *mut u32;

    unsafe { *dst_ptr = pixel.le() };

    Ok(())
}

fn write_pixel_bgr(self_: &mut FrameBuffer, position: U64Vec2, mut pixel: RgbColor) -> Result<()> {
    if !self_.is_inside_buffer(position) {
        return Err(FrameBufferError::OutsideBufferError.into());
    }

    let offset =
        (position.y * self_.graphic_info.width + position.x) * self_.graphic_info.bytes_per_pixel;
    let dst_addr = self_.graphic_info.frame_buffer_addr.unwrap() + offset;
    let dst_ptr = dst_addr as *mut u8 as *mut u32;

    pixel.bgr();

    unsafe { *dst_ptr = pixel.le() };

    Ok(())
}

pub fn init(graphic_info: &GraphicInfo) -> Result<()> {
    let mut frame_buffer = FrameBuffer::new_empty();
    frame_buffer.init(graphic_info);

    FRAME_BUFFER_WIDTH.call_once(|| frame_buffer.width() as usize);
    FRAME_BUFFER_HEIGHT.call_once(|| frame_buffer.height() as usize);
    FRAME_BUFFER.call_once(|| Arc::new(Mutex::new(frame_buffer)));
    PIXEL_FORMAT.call_once(|| graphic_info.pixel_format);
    BYTES_PER_PIXEL.call_once(|| graphic_info.bytes_per_pixel);

    Ok(())
}
