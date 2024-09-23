use crate::{error::Result, printk};
use common::graphic::{GraphicInfo, Pixel, PixelFormat, RgbColor};
use spin::{Mutex, MutexGuard};

use super::font::{self, CHARACTER_WIDTH, GARBLED_FONT, U8_FONT};

static mut FRAME_BUF: Mutex<Option<FrameBuf>> = Mutex::new(None);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameBufferError {
    UnsupportedPixelFormatError,
    NotInitializedError,
    OutsideBufferError,
    FrameBufferLockError,
    UnsupportedCharacterError,
}

#[derive(Clone, Debug)]
struct FrameBuf {
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
    fn new(graphic_info: &GraphicInfo) -> Self {
        Self {
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
        }
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
            self.write_pixel(x_inner, y, color.into());
            self.write_pixel(x_inner, y + height - 1, color.into());
        }
        for y_inner in y..y + height {
            self.write_pixel(x, y_inner, color.into());
            self.write_pixel(x + width - 1, y_inner, color.into());
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
                    self.write_pixel(x + dx, y + dy, fg.into());
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

fn write_pixel_bgr(self_: &mut FrameBuf, x: usize, y: usize, pixel: Pixel) -> Result<()> {
    if !self_.is_inside_buffer(x, y) {
        return Err(FrameBufferError::OutsideBufferError.into());
    }
    let offset = (y * self_.width + x) * self_.bytes_per_pixel;
    let pixel_ref = (self_.framebuf_addr + offset as u64) as *mut u32;
    let mut pixel = pixel;
    pixel.bgr();

    unsafe {
        *pixel_ref = pixel.le();
    };
    // }
    Ok(())
}

fn lock_framebuf<'a>() -> Result<MutexGuard<'a, Option<FrameBuf>>> {
    unsafe { FRAME_BUF.try_lock() }.ok_or(FrameBufferError::FrameBufferLockError.into())
}

pub fn init(graphic_info: &GraphicInfo, bg: RgbColor) -> Result<()> {
    let mut frame_buf = FrameBuf::new(graphic_info);
    frame_buf.fill(bg);
    lock_framebuf()?.replace(frame_buf);
    Ok(())
}

pub fn write_pixel(x: usize, y: usize, pixel: Pixel) -> Result<()> {
    lock_framebuf()?
        .as_mut()
        .ok_or(FrameBufferError::NotInitializedError)?
        .write_pixel(x, y, pixel)?;
    Ok(())
}

pub fn write_char(x: usize, y: usize, c: char, fg: RgbColor) -> Result<()> {
    lock_framebuf()?
        .as_mut()
        .ok_or(FrameBufferError::NotInitializedError)?
        .write_char(x, y, c, fg)?;
    Ok(())
}

pub fn write_string(x: usize, y: usize, s: &str, fg: RgbColor) -> Result<()> {
    lock_framebuf()?
        .as_mut()
        .ok_or(FrameBufferError::NotInitializedError)?
        .write_string(x, y, s, fg)?;
    Ok(())
}

pub fn fill(color: RgbColor) -> Result<()> {
    lock_framebuf()?
        .as_mut()
        .ok_or(FrameBufferError::NotInitializedError)?
        .fill(color)?;
    Ok(())
}

pub fn fill_rect(x: usize, y: usize, width: usize, height: usize, color: RgbColor) -> Result<()> {
    lock_framebuf()?
        .as_mut()
        .ok_or(FrameBufferError::NotInitializedError)?
        .fill_rect(x, y, width, height, color)?;
    Ok(())
}

pub fn draw_rect(x: usize, y: usize, width: usize, height: usize, color: RgbColor) -> Result<()> {
    lock_framebuf()?
        .as_mut()
        .ok_or(FrameBufferError::NotInitializedError)?
        .draw_rect(x, y, width, height, color)?;
    Ok(())
}

pub fn width() -> Result<usize> {
    Ok(lock_framebuf()?
        .as_mut()
        .ok_or(FrameBufferError::NotInitializedError)?
        .get_width())
}

pub fn height() -> Result<usize> {
    Ok(lock_framebuf()?
        .as_mut()
        .ok_or(FrameBufferError::NotInitializedError)?
        .get_height())
}
