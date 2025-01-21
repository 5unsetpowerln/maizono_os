use crate::error::Result;
use common::graphic::{GraphicInfo, Pixel, PixelFormat, RgbColor};
use spin::{Mutex, MutexGuard};
use thiserror_no_std::Error;

use super::font::{self, GARBLED_FONT, U8_FONT};

static FRAME_BUF: Mutex<FrameBuf> = Mutex::new(FrameBuf::new());

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
pub struct FrameBuf {
    width: usize,
    height: usize,
    bytes_per_pixel: usize,
    stride: usize,
    pixel_format: PixelFormat,
    framebuf_addr: u64,
    framebuf_size: usize,
    write_pixel: fn(&mut FrameBuf, usize, usize, Pixel) -> Result<()>,
}

impl FrameBuf {
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

    fn write_pixel(&mut self, x: usize, y: usize, pixel: Pixel) -> Result<()> {
        (self.write_pixel)(self, x, y, pixel)
    }

    fn fill(&mut self, color: RgbColor) -> Result<()> {
        for x in 0..self.width {
            for y in 0..self.height {
                match self.write_pixel(x, y, color.into()) {
                    Ok(_) => continue,
                    Err(err) => return Err(err),
                }
            }
        }
        Ok(())
    }

    fn fill_rect(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        color: RgbColor,
    ) -> Result<()> {
        for x_inner in x..x + width {
            for y_inner in y..y + height {
                self.write_pixel(x_inner, y_inner, color.into())?;
            }
        }
        Ok(())
    }

    fn draw_rect(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        color: RgbColor,
    ) -> Result<()> {
        for x_inner in x..x + width {
            self.write_pixel(x_inner, y, color.into())?;
            self.write_pixel(x_inner, y + height - 1, color.into())?;
        }
        for y_inner in y..y + height {
            self.write_pixel(x, y_inner, color.into())?;
            self.write_pixel(x + width - 1, y_inner, color.into())?;
        }
        Ok(())
    }

    fn write_char(&mut self, x: usize, y: usize, ascii: char, fg: RgbColor) -> Result<()> {
        let glyph_index = ascii as usize;
        let glyph = {
            if glyph_index >= U8_FONT.len() {
                GARBLED_FONT
            } else {
                U8_FONT[glyph_index]
            }
        };

        for (dy, row) in glyph.iter().enumerate() {
            for dx in 0..font::CHARACTER_WIDTH {
                if (row >> 7 - dx) & 1 == 1 {
                    self.write_pixel(x + dx, y + dy, fg.into())?;
                }
            }
        }
        Ok(())
    }

    fn write_string(&mut self, x: usize, y: usize, ascii_s: &str, fg: RgbColor) -> Result<()> {
        for (i, c) in ascii_s.chars().enumerate() {
            self.write_char(x + i * font::CHARACTER_WIDTH * 2, y, c, fg)?;
        }
        Ok(())
    }

    fn is_inside_buffer(&mut self, x: usize, y: usize) -> bool {
        !(x >= self.width || y >= self.height)
    }

    fn get_width(&self) -> usize {
        self.width
    }

    fn get_height(&self) -> usize {
        self.height
    }
}

fn write_pixel_rgb(self_: &mut FrameBuf, x: usize, y: usize, pixel: Pixel) -> Result<()> {
    if !self_.is_inside_buffer(x, y) {
        return Err(FrameBufferError::OutsideBufferError.into());
    }
    let offset = (y * self_.width + x) * self_.bytes_per_pixel;
    let pixel_ref = (self_.framebuf_addr + offset as u64) as *mut u32;

    unsafe {
        *pixel_ref = pixel.le();
    };
    Ok(())
}

fn write_pixel_bgr(self_: &mut FrameBuf, x: usize, y: usize, mut pixel: Pixel) -> Result<()> {
    if !self_.is_inside_buffer(x, y) {
        return Err(FrameBufferError::OutsideBufferError.into());
    }

    let offset = (y * self_.width + x) * self_.bytes_per_pixel;
    let pixel_ref = (self_.framebuf_addr + offset as u64) as *mut u32;
    pixel.bgr();

    unsafe {
        *pixel_ref = pixel.le();
    };
    Ok(())
}

pub fn frame_buf() -> Result<MutexGuard<'static, FrameBuf>> {
    FRAME_BUF
        .try_lock()
        .ok_or(FrameBufferError::FrameBufferLockError.into())
}

pub fn write_pixel(x: usize, y: usize, pixel: Pixel) -> Result<()> {
    frame_buf()?.write_pixel(x, y, pixel)?;
    Ok(())
}

pub fn write_char(x: usize, y: usize, c: char, fg: RgbColor) -> Result<()> {
    frame_buf()?.write_char(x, y, c, fg)?;
    Ok(())
}

pub fn write_string(x: usize, y: usize, s: &str, fg: RgbColor) -> Result<()> {
    frame_buf()?.write_string(x, y, s, fg)?;
    Ok(())
}

pub fn fill(color: RgbColor) -> Result<()> {
    frame_buf()?.fill(color)?;
    Ok(())
}

pub fn fill_rect(x: usize, y: usize, width: usize, height: usize, color: RgbColor) -> Result<()> {
    frame_buf()?.fill_rect(x, y, width, height, color)?;
    Ok(())
}

pub fn draw_rect(x: usize, y: usize, width: usize, height: usize, color: RgbColor) -> Result<()> {
    frame_buf()?.draw_rect(x, y, width, height, color)?;
    Ok(())
}

pub fn width() -> Result<usize> {
    Ok(frame_buf()?.get_width())
}

pub fn height() -> Result<usize> {
    Ok(frame_buf()?.get_height())
}
