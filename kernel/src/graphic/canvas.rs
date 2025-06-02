use core::ptr::copy_nonoverlapping;

use alloc::{format, sync::Arc, vec::Vec};
use anyhow::Context;
use common::graphic::{GraphicInfo, RgbColor};
use glam::{U64Vec2, U64Vec4, u64vec2};
use log::debug;
use spin::Mutex;

use crate::{
    error::Result,
    graphic::{
        PixelWriter, PixelWriterCopyable, Rectangle,
        frame_buffer::{self, FrameBuffer},
    },
    serial_println,
};

type PixelWriterRef<'a> = &'a mut dyn PixelWriter;
type PixelWriterCopyableRef<'a> = &'a mut dyn PixelWriterCopyable;

#[derive(Debug)]
pub struct Canvas {
    width: u64,
    height: u64,
    // data: Vec<Vec<RgbColor>>,
    consider_transparent: bool,
    shadow_buffer: FrameBuffer,
}

impl Canvas {
    pub const fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            // data: Vec::new(),
            consider_transparent: false,
            shadow_buffer: FrameBuffer::new_empty(),
        }
    }

    pub fn init(&mut self, width: u64, height: u64, consider_transparent: bool) {
        // let mut data = Vec::new();

        // let mut row = Vec::new();
        // row.resize(width as usize, RgbColor::new());
        // let row_len = row.len();
        // data.resize(height as usize, row);

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

    pub fn draw_to<'a>(&self, frame_buffer: &Arc<Mutex<FrameBuffer>>, pos: U64Vec2) {
        if self.consider_transparent {
            for y in 0..self.height {
                let mut writer = frame_buffer.lock();
                for x in 0..self.width {
                    let c = self.shadow_buffer.at(u64vec2(x, y));

                    if !c.is_transparent() {
                        writer
                            .write_pixel(pos + u64vec2(x, y), c)
                            .expect("Failed to write a pixel to the frame buffer.");
                    } else {
                    }
                }
            }
        } else {
            unsafe { frame_buffer.lock().copy(pos, &self.shadow_buffer) };
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

pub fn create_arc_mutex_canvas(
    width: u64,
    height: u64,
    consider_transparent: bool,
) -> Arc<Mutex<Canvas>> {
    let mut canvas = Canvas::new();
    canvas.init(width, height, consider_transparent);
    Arc::new(Mutex::new(canvas))
}
