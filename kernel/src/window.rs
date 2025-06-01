use core::ptr::copy_nonoverlapping;

use alloc::{format, sync::Arc, vec::Vec};
use anyhow::Context;
use common::graphic::{GraphicInfo, RgbColor};
use glam::U64Vec2;
use log::debug;
use spin::Mutex;

use crate::{
    error::Result,
    graphic::{
        PixelWriter, PixelWriterCopyable,
        frame_buffer::{self, FrameBuffer},
    },
};

type PixelWriterRef<'a> = &'a mut dyn PixelWriter;
type PixelWriterCopyableRef<'a> = &'a mut dyn PixelWriterCopyable;

#[derive(Debug)]
pub struct Window {
    width: u64,
    height: u64,
    data: Vec<Vec<RgbColor>>,
    // transparent_color: Option<RgbColor>,
    consider_transparent: bool,
    shadow_buffer: FrameBuffer,
}

impl Window {
    pub const fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            data: Vec::new(),
            consider_transparent: false,
            shadow_buffer: FrameBuffer::new_empty(),
        }
    }

    pub fn init(&mut self, width: u64, height: u64, consider_transparent: bool) {
        let mut data = Vec::new();

        let mut row = Vec::new();
        row.resize(width as usize, RgbColor::new());
        let row_len = row.len();
        data.resize(height as usize, row);

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

        assert_eq!(data.len(), height as usize);
        assert_eq!(row_len, width as usize);

        *self = Self {
            width,
            height,
            data,
            consider_transparent,
            shadow_buffer,
        };
    }

    pub fn draw_to<'a>(&self, writer: &Arc<Mutex<FrameBuffer>>, position: U64Vec2) {
        if self.consider_transparent {
            for y in 0..self.height {
                let mut writer = writer.lock();
                for x in 0..self.width {
                    let c: RgbColor = self.data[y as usize][x as usize];
                    if !c.is_transparent {
                        writer
                            .write_pixel(
                                U64Vec2 {
                                    x: position.x + x,
                                    y: position.y + y,
                                },
                                c,
                            )
                            .expect("Failed to write a pixel to the frame buffer.");
                    }
                }
            }
        } else {
            unsafe { writer.lock().copy(position, &self.shadow_buffer) };
        }
    }

    pub fn set_transparent_color(&mut self, value: bool) {
        self.consider_transparent = value;
    }
}

impl PixelWriter for Window {
    fn width(&self) -> u64 {
        self.width
    }

    fn height(&self) -> u64 {
        self.height
    }

    fn write_pixel(&mut self, position: U64Vec2, color: RgbColor) -> Result<()> {
        *self
            .data
            .get_mut(position.y as usize)
            .unwrap()
            .get_mut(position.x as usize)
            .unwrap() = color;
        self.shadow_buffer.write_pixel(position, color)?;
        Ok(())
    }
}
