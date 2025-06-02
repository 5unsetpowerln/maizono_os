use crate::{error::Result, serial_println, util::u32_from_slice};
use alloc::{sync::Arc, vec::Vec};
use common::graphic::{GraphicInfo, PixelFormat, RgbColor, rgb};
use glam::{U64Vec2, U64Vec4, u64vec2};
use log::{debug, info};
use spin::{Mutex, Once};
use thiserror_no_std::Error;

use super::{PixelWriter, PixelWriterCopyable, Rectangle};

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
    pub graphic_info: GraphicInfo,
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
            // graphic_info: graphic_info.clone(),
            graphic_info,
        };
    }

    pub unsafe fn copy(&mut self, pos: U64Vec2, src: &Self) {
        let dst_width = self.graphic_info.width;
        let dst_height = self.graphic_info.height;
        let src_width = src.graphic_info.width;
        let src_height = src.graphic_info.height;

        assert!(dst_width >= pos.x + src_width);
        assert!(dst_height >= pos.x + src_height);
        assert!(src_width * src_height * 4 == src.buffer.len() as u64);

        let bytes_per_src_width = self.graphic_info.bytes_per_pixel * src_width;
        let bytes_per_dst_width = self.graphic_info.bytes_per_pixel * dst_width;

        let mut dst_ptr = get_hw_frame_buffer_mut_ptr_at(pos, &self.graphic_info);
        let mut src_ptr = src.graphic_info.frame_buffer_addr.unwrap() as *mut u8;

        for _ in 0..src_height {
            unsafe {
                dst_ptr.copy_from(src_ptr, bytes_per_src_width as usize);
                dst_ptr = dst_ptr.add(bytes_per_dst_width as usize);
                src_ptr = src_ptr.add(bytes_per_src_width as usize);
            }
        }
    }

    pub unsafe fn move_rect(&mut self, dst_pos: U64Vec2, src_rect: Rectangle) {
        let bytes_per_pixel = self.graphic_info.bytes_per_pixel;
        let bytes_per_scan_line = bytes_per_pixel * self.graphic_info.width;

        assert!(src_rect.pos.x + src_rect.width <= self.graphic_info.width);
        assert!(src_rect.pos.y + src_rect.height <= self.graphic_info.height);
        assert!(dst_pos.x + src_rect.width <= self.graphic_info.width);
        assert!(dst_pos.y + src_rect.height <= self.graphic_info.height);

        if dst_pos.y < src_rect.pos.y {
            let mut dst_ptr = get_hw_frame_buffer_mut_ptr_at(dst_pos, &self.graphic_info);
            let mut src_ptr = get_hw_frame_buffer_ptr_at(src_rect.pos, &self.graphic_info);

            for _ in 0..src_rect.height {
                unsafe {
                    dst_ptr.copy_from(src_ptr, (src_rect.width * bytes_per_pixel) as usize);
                    dst_ptr = dst_ptr.add(bytes_per_scan_line as usize);
                    src_ptr = src_ptr.add(bytes_per_scan_line as usize);
                };
            }
        } else {
            let mut dst_ptr = get_hw_frame_buffer_mut_ptr_at(
                dst_pos + u64vec2(0, src_rect.height),
                &self.graphic_info,
            );
            let mut src_ptr = get_hw_frame_buffer_ptr_at(
                src_rect.pos + u64vec2(0, src_rect.height),
                &self.graphic_info,
            );

            for _ in 0..src_rect.height {
                unsafe {
                    dst_ptr.copy_from_nonoverlapping(
                        src_ptr,
                        (src_rect.width * bytes_per_pixel) as usize,
                    );
                    dst_ptr = dst_ptr.sub(bytes_per_scan_line as usize);
                    src_ptr = src_ptr.sub(bytes_per_scan_line as usize);
                }
            }
        }
    }

    pub fn at(&self, pos: U64Vec2) -> RgbColor {
        if self.graphic_info.frame_buffer_addr.is_some() {
            let ptr = get_hw_frame_buffer_ptr_at(pos, &self.graphic_info) as *const u32;
            let value = unsafe { *ptr };

            RgbColor::from_bgr_le(value)
        } else {
            unreachable!()
        }
    }
}

fn get_hw_frame_buffer_ptr_at(pos: U64Vec2, graphic_info: &GraphicInfo) -> *const u8 {
    let addr = graphic_info.frame_buffer_addr.unwrap()
        + graphic_info.bytes_per_pixel * (graphic_info.width * pos.y + pos.x);
    addr as *const u8
}

fn get_hw_frame_buffer_mut_ptr_at(pos: U64Vec2, graphic_info: &GraphicInfo) -> *mut u8 {
    let addr = graphic_info.frame_buffer_addr.unwrap()
        + graphic_info.bytes_per_pixel * (graphic_info.width * pos.y + pos.x);
    addr as *mut u8
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

fn write_pixel_rgb(self_: &mut FrameBuffer, pos: U64Vec2, color: RgbColor) -> Result<()> {
    if !self_.is_inside_buffer(pos) {
        return Err(FrameBufferError::OutsideBufferError.into());
    }

    let dst_ptr = get_hw_frame_buffer_mut_ptr_at(pos, &self_.graphic_info) as *mut u32;

    unsafe { *dst_ptr = color.get_rgb_le() };

    Ok(())
}

fn write_pixel_bgr(self_: &mut FrameBuffer, pos: U64Vec2, color: RgbColor) -> Result<()> {
    if !self_.is_inside_buffer(pos) {
        return Err(FrameBufferError::OutsideBufferError.into());
    }

    let dst_ptr = get_hw_frame_buffer_mut_ptr_at(pos, &self_.graphic_info) as *mut u32;

    // serial_println!("{:x}", color.get_bgr_le());
    unsafe { *dst_ptr = color.get_bgr_le() };

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
