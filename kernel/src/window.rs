use alloc::{format, vec::Vec};
use anyhow::Context;
use common::graphic::RgbColor;
use glam::U64Vec2;

use crate::{error::Result, graphic::PixelWriter, serial_println};

type PixelWriterRef<'a> = &'a mut dyn PixelWriter;

#[derive(Debug)]
pub struct Window {
    width: u64,
    height: u64,
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

    pub fn init(&mut self, width: u64, height: u64, transparent_color: Option<RgbColor>) {
        let mut data = Vec::new();

        let mut data_for_each_y = Vec::new();
        data_for_each_y.resize(width as usize, RgbColor::new());

        for _ in 0..height {
            data.resize(height as usize, data_for_each_y.clone());
        }

        *self = Self {
            width,
            height,
            data,
            transparent_color,
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
        // self.data[position.y as usize][position.x as usize] = color;
        let ptr = self
            .data
            .get_mut(position.y as usize)
            .with_context(|| {
                format!(
                    "Failed to get a mutable reference of data[{}] / data[{}][{}]",
                    position.y, self.height, self.width
                )
            })
            .unwrap()
            .get_mut(position.x as usize)
            .with_context(|| {
                format!(
                    "Failed to get a mutable reference of data[{}][{}] / data[{}][{}].",
                    position.y, position.x, self.height, self.width
                )
            })
            .unwrap();
        *ptr = color;
        Ok(())
    }
}
