use core::fmt;

use super::{font::FONT, RgbColor};

pub type Error = super::error::FrameBufferError;

struct FrameBufferInfo {
    width: usize,
    height: usize,
    bytes_per_pixel: usize,
    byte_length: usize,
    stride: usize,
    pixel_format: PixelFormat,
}

impl FrameBufferInfo {
    pub fn from_bootloader_api(info: bootloader_api::info::FrameBufferInfo) -> Result<Self, Error> {
        Ok(Self {
            width: info.width,
            height: info.height,
            bytes_per_pixel: info.bytes_per_pixel,
            byte_length: info.byte_len,
            stride: info.stride,
            pixel_format: match info.pixel_format {
                bootloader_api::info::PixelFormat::Rgb => PixelFormat::Rgb,
                bootloader_api::info::PixelFormat::Bgr => PixelFormat::Bgr,
                _ => return Err(Error::UnsupportedPixelFormat),
            },
        })
    }
}

enum PixelFormat {
    Rgb,
    Bgr,
}

pub struct FrameBufferWriter<'a> {
    info: FrameBufferInfo,
    buffer: &'a mut [u8],
    pixel_write: fn(&mut [u8], &FrameBufferInfo, usize, usize, &RgbColor),
}

impl<'a> FrameBufferWriter<'a> {
    pub fn from_bootloader_api(
        framebuffer: &'a mut bootloader_api::info::FrameBuffer,
    ) -> Result<Self, Error> {
        let info = FrameBufferInfo::from_bootloader_api(framebuffer.info())?;
        let buffer = framebuffer.buffer_mut();
        let pixel_write = match info.pixel_format {
            PixelFormat::Rgb => pixel_write_rgb,
            PixelFormat::Bgr => pixel_write_bgr,
        };

        Ok(Self {
            info,
            buffer,
            pixel_write,
        })
    }

    pub fn pixel_write(&mut self, x: usize, y: usize, color: &RgbColor) {
        (self.pixel_write)(self.buffer, &self.info, x, y, color);
    }

    pub fn width(&self) -> usize {
        self.info.width
    }

    pub fn height(&self) -> usize {
        self.info.height
    }
}

fn pixel_write_rgb(
    buffer: &mut [u8],
    info: &FrameBufferInfo,
    x: usize,
    y: usize,
    color: &RgbColor,
) {
    let pixel_position = info.stride * y + x;
    let byte_base_position = pixel_position * info.bytes_per_pixel;
    let pixel_array = [color.red, color.green, color.blue, 0x0];

    buffer[byte_base_position..(byte_base_position + info.bytes_per_pixel)]
        .copy_from_slice(pixel_array[0..info.bytes_per_pixel].as_ref());
}

fn pixel_write_bgr(
    buffer: &mut [u8],
    info: &FrameBufferInfo,
    x: usize,
    y: usize,
    color: &RgbColor,
) {
    let pixel_position = info.stride * y + x;
    let byte_base_position = pixel_position * info.bytes_per_pixel;
    let pixel_array = [color.blue, color.green, color.red, 0x0];

    buffer[byte_base_position..(byte_base_position + info.bytes_per_pixel)]
        .copy_from_slice(pixel_array[0..info.bytes_per_pixel].as_ref());
}
