pub type Error = super::error::FrameBufferError;

struct FrameBufferInfo {
    width: usize,
    height: usize,
    bytes_per_pixel: usize,
    byte_length: usize,
    pixel_format: PixelFormat,
}

impl FrameBufferInfo {
    pub fn from_bootloader_api(info: bootloader_api::info::FrameBufferInfo) -> Result<Self, Error> {
        Ok(Self {
            width: info.width,
            height: info.height,
            bytes_per_pixel: info.bytes_per_pixel,
            byte_length: info.byte_len,
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

pub struct FrameBuffer<'a> {
    info: FrameBufferInfo,
    buffer: &'a mut [u8],
    pixel_write: fn(&mut [u8], &FrameBufferInfo, usize, usize, RgbColor),
}

impl<'a> FrameBuffer<'a> {
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

    pub fn pixel_write(&mut self, x: usize, y: usize, color: RgbColor) {
        (self.pixel_write)(self.buffer, &self.info, x, y, color);
    }

    pub fn width(&self) -> usize {
        self.info.width
    }

    pub fn height(&self) -> usize {
        self.info.height
    }
}

fn pixel_write_rgb(buffer: &mut [u8], info: &FrameBufferInfo, x: usize, y: usize, color: RgbColor) {
    let pixel_position = info.width * y + x;

    buffer[pixel_position * info.bytes_per_pixel] = color.red;
    buffer[pixel_position * info.bytes_per_pixel + 1] = color.green;
    buffer[pixel_position * info.bytes_per_pixel + 2] = color.blue;
}

fn pixel_write_bgr(buffer: &mut [u8], info: &FrameBufferInfo, x: usize, y: usize, color: RgbColor) {
    let pixel_position = info.width * y + x;

    buffer[pixel_position * info.bytes_per_pixel] = color.blue;
    buffer[pixel_position * info.bytes_per_pixel + 1] = color.green;
    buffer[pixel_position * info.bytes_per_pixel + 2] = color.red;
}
