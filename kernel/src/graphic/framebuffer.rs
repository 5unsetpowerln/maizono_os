use once_cell::sync::Lazy;

use crate::error::Result;
use common::graphic::{GraphicInfo, Pixel, PixelFormat, RgbColor};

use super::font::FONT;

static mut FRAME_BUF: Lazy<Option<FrameBuf>> = Lazy::new(|| None);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameBufferError {
    UnsupportedPixelFormatError,
    NotInitializedError,
    OutsideBufferError,
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
            framebuf_addr: graphic_info.framebuf_addr,
            framebuf_size: graphic_info.framebuf_size,
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

    fn write_char(&mut self, x: usize, y: usize, c: char, color: RgbColor) -> Result<()> {
        for (y_offset, row) in FONT
            .font
            .get(c as usize)
            .unwrap_or(&FONT.unprintable)
            .iter()
            .enumerate()
        {
            for x_offset in 0..FONT.width {
                if (row >> x_offset) & 1 == 1 {
                    match self.write_pixel(
                        x + (super::font::FONT.width - x_offset),
                        y + y_offset,
                        color.into(),
                    ) {
                        Ok(_) => continue,
                        Err(err) => return Err(err),
                    }
                }
            }
        }
        Ok(())
    }

    fn write_string(&mut self, x: usize, y: usize, s: &str, color: RgbColor) -> Result<()> {
        for (i, c) in s.chars().enumerate() {
            match self.write_char(x + i * FONT.width, y, c, color) {
                Ok(_) => continue,
                Err(err) => return Err(err),
            }
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

pub fn init(graphic_info: &GraphicInfo, bg: RgbColor) {
    unsafe {
        let mut frame_buf = FrameBuf::new(graphic_info);
        frame_buf.fill(bg);
        FRAME_BUF.replace(frame_buf);
    }
}

pub fn write_pixel(x: usize, y: usize, pixel: Pixel) -> Result<()> {
    unsafe {
        FRAME_BUF
            .as_mut()
            .ok_or(FrameBufferError::NotInitializedError)?
            .write_pixel(x, y, pixel)?;
    }
    Ok(())
}

pub fn write_char(x: usize, y: usize, c: char, color: RgbColor) -> Result<()> {
    unsafe {
        FRAME_BUF
            .as_mut()
            .ok_or(FrameBufferError::NotInitializedError)?
            .write_char(x, y, c, color)?;
    }
    Ok(())
}

pub fn write_string(x: usize, y: usize, s: &str, color: RgbColor) -> Result<()> {
    unsafe {
        FRAME_BUF
            .as_mut()
            .ok_or(FrameBufferError::NotInitializedError)?
            .write_string(x, y, s, color)?;
    }
    Ok(())
}

pub fn fill(color: RgbColor) -> Result<()> {
    unsafe {
        FRAME_BUF
            .as_mut()
            .ok_or(FrameBufferError::NotInitializedError)?
            .fill(color)?;
    }
    Ok(())
}

pub fn fill_rect(x: usize, y: usize, width: usize, height: usize, color: RgbColor) -> Result<()> {
    unsafe {
        FRAME_BUF
            .as_mut()
            .ok_or(FrameBufferError::NotInitializedError)?
            .fill_rect(x, y, width, height, color)?;
    }
    Ok(())
}

pub fn draw_rect(x: usize, y: usize, width: usize, height: usize, color: RgbColor) -> Result<()> {
    unsafe {
        FRAME_BUF
            .as_mut()
            .ok_or(FrameBufferError::NotInitializedError)?
            .draw_rect(x, y, width, height, color)?;
    }
    Ok(())
}
