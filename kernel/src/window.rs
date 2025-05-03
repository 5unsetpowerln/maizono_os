use alloc::vec::Vec;
use common::{graphic::RgbColor, matrix::Vec2};

use crate::{allocator::Locked, error::Result, graphic::PixelWriter};

type PixelWriterRef<'a> = &'a mut dyn PixelWriter;

pub struct Window {
    width: usize,
    height: usize,
    data: Vec<Vec<RgbColor>>,
    transparent_color: Option<RgbColor>,
}

impl Window {
    pub const fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            data: Vec::new(),
            transparent_color: None,
        }
    }

    pub fn init(&mut self, width: usize, height: usize, transparent_color: Option<RgbColor>) {
        let mut data = Vec::new();

        let mut data_for_each_y = Vec::new();
        data_for_each_y.resize(width, RgbColor::new());

        for _ in 0..height {
            data.resize(height, data_for_each_y.clone());
        }

        *self = Self {
            width,
            height,
            data,
            transparent_color,
        };
    }

    // pub fn draw_to<'a>(&self, writer: Locked<PixelWriterRef<'a>>, position: Vec2<usize>) {
    pub fn draw_to<'a>(&self, writer: PixelWriterRef<'a>, position: Vec2<usize>) {
        // let mut writer = writer.lock();
        if let Some(transparent_color) = self.transparent_color {
            for y in 0..self.height {
                for x in 0..self.width {
                    let c: RgbColor = self.data[y][x];
                    if c != transparent_color {
                        writer
                            .write_pixel(position.x + x, position.y + y, c)
                            .expect("Failed to write a pixel to the frame buffer.");
                    }
                }
            }
        } else {
            for y in 0..self.height {
                for x in 0..self.width {
                    writer
                        .write_pixel(position.x + x, position.y + y, self.data[y][x])
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
    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn write_pixel(&mut self, x: usize, y: usize, color: RgbColor) -> Result<()> {
        self.data[y][x] = color;
        Ok(())
    }
}
