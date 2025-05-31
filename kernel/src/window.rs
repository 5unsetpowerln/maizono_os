use alloc::{format, vec::Vec};
use anyhow::Context;
use common::graphic::{GraphicInfo, RgbColor};
use glam::U64Vec2;

use crate::{
    error::Result,
    graphic::{PixelWriter, frame_buffer::FrameBuffer},
    serial_println,
};

type PixelWriterRef<'a> = &'a mut dyn PixelWriter;

#[derive(Debug)]
pub struct Window {
    width: u64,
    height: u64,
    data: Vec<Vec<RgbColor>>,
    transparent_color: Option<RgbColor>,
    shadow_buffer: FrameBuffer,
}

impl Window {
    pub const fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            data: Vec::new(),
            transparent_color: None,
            shadow_buffer: FrameBuffer::new_empty(),
        }
    }

    pub fn init(&mut self, width: u64, height: u64, transparent_color: Option<RgbColor>) {
        let mut data = Vec::new();

        let mut row = Vec::new();
        row.resize(width as usize, RgbColor::new());
        let row_len = row.len();
        data.resize(height as usize, row);

        let mut shadow_buffer_data = Vec::new();
        shadow_buffer_data.resize((width * height * 4) as usize, 0);
        let shadow_buffer = FrameBuffer::new(width, height, shadow_buffer_data.as_mut_ptr());

        assert_eq!(data.len(), height as usize);
        assert_eq!(row_len, width as usize);

        *self = Self {
            width,
            height,
            data,
            transparent_color,
            shadow_buffer,
        };
    }

    pub fn draw_to<'a>(&self, writer: PixelWriterRef<'a>, position: U64Vec2) {
        if let Some(transparent_color) = self.transparent_color {
            for y in 0..self.height {
                for x in 0..self.width {
                    let c: RgbColor = self.data[y as usize][x as usize];
                    if c != transparent_color {
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
            for y in 0..self.height {
                for x in 0..self.width {
                    writer
                        .write_pixel(
                            U64Vec2 {
                                x: position.x + x,
                                y: position.y + y,
                            },
                            self.data[y as usize][x as usize],
                        )
                        .expect("Failed to write a pixel to the frame buffer.");
                }
            }
        }
    }

    pub fn set_transparent_color(&mut self, color: RgbColor) {
        self.transparent_color = Some(color);
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
        // serial_println!(
        //     "{:?} / {} * {}",
        //     position,
        //     self.data[0].len(),
        //     self.data.len()
        // );
        self.data[position.y as usize][position.x as usize] = color;
        // let ptr = self
        //     .data
        //     .get_mut(position.y as usize)
        //     .with_context(|| {
        //         format!(
        //             "Failed to get a mutable reference of data[{}] / data[{}][{}]",
        //             position.y, self.height, self.width
        //         )
        //     })
        //     .unwrap()
        //     .get_mut(position.x as usize)
        //     .with_context(|| {
        //         format!(
        //             "Failed to get a mutable reference of data[{}][{}] / data[{}][{}].",
        //             position.y, position.x, self.height, self.width
        //         )
        //     })
        //     .unwrap();
        // *ptr = color;
        Ok(())
    }
}
