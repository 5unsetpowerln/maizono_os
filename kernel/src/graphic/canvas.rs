use alloc::sync::Arc;
use common::graphic::{GraphicInfo, RgbColor};
use glam::{U64Vec2, u64vec2};

use crate::{
    error::Result,
    graphic::{
        PixelWriter, Rectangle,
        frame_buffer::{self, FrameBuffer},
    },
};

#[derive(Debug)]
pub struct Canvas {
    width: u64,
    height: u64,
    consider_transparent: bool,
    shadow_buffer: FrameBuffer,
}

impl Canvas {
    pub const fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            consider_transparent: false,
            shadow_buffer: FrameBuffer::new_empty(),
        }
    }

    pub fn init(&mut self, width: u64, height: u64, consider_transparent: bool) {
        let mut shadow_buffer = FrameBuffer::new_empty();
        let graphic_info = GraphicInfo {
            width,
            height,
            stride: width as usize,
            pixel_format: frame_buffer::PIXEL_FORMAT.wait().clone(),
            bytes_per_pixel: frame_buffer::BYTES_PER_PIXEL.wait().clone(),
            frame_buffer_addr: None,
            frame_buffer_size: 0,
        };
        shadow_buffer.init(&graphic_info);

        *self = Self {
            width,
            height,
            consider_transparent,
            shadow_buffer,
        };
    }

    pub fn draw_to<'a>(&self, frame_buffer: &mut FrameBuffer, pos: U64Vec2) {
        if self.consider_transparent {
            for y in 0..self.height {
                for x in 0..self.width {
                    let c = self.shadow_buffer.at(u64vec2(x, y));

                    if !c.is_transparent() {
                        frame_buffer
                            .write_pixel(pos + u64vec2(x, y), c)
                            .expect("Failed to write a pixel to the frame buffer.");
                    }
                }
            }
        } else {
            unsafe { frame_buffer.copy(pos, &self.shadow_buffer) };
        }
    }

    pub fn move_rect(&mut self, dst_pos: U64Vec2, src_rect: Rectangle) {
        unsafe {
            self.shadow_buffer.move_rect(dst_pos, src_rect);
        }
    }
}

impl PixelWriter for Canvas {
    fn width(&self) -> u64 {
        self.width
    }

    fn height(&self) -> u64 {
        self.height
    }

    fn write_pixel(&mut self, position: U64Vec2, color: RgbColor) -> Result<()> {
        self.shadow_buffer.write_pixel(position, color)?;
        Ok(())
    }
}
