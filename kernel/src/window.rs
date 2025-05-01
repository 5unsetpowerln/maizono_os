use alloc::vec::Vec;
use common::{graphic::RgbColor, matrix::Vec2};

use crate::graphic::{PixelWriter, frame_buffer};

pub struct Window {
    width: usize,
    height: usize,
    data: Vec<Vec<RgbColor>>,
    transparent_color: Option<RgbColor>,
}

impl Window {
    pub fn new(width: usize, height: usize, transparent_color: Option<RgbColor>) -> Self {
        let mut data = Vec::new();

        let mut data_for_each_y = Vec::new();
        data_for_each_y.resize(width, RgbColor::new());

        for _ in 0..height {
            data.resize(height, data_for_each_y.clone());
        }

        Self {
            width,
            height,
            data,
            transparent_color,
        }
    }

    pub fn draw_to_frame_buffer(&self, position: Vec2<usize>) {
        if let Some(transparent_color) = self.transparent_color {
            for y in 0..self.height {
                for x in 0..self.width {
                    let c: RgbColor = self.data[y][x].into();
                    if c != transparent_color {
                        unsafe {
                            frame_buffer::get_frame_buffer_reference()
                                .lock()
                                .write_pixel(position.x + x, position.y + y, c.into())
                                .expect("Failed to write a pixel to the frame buffer.")
                        };
                    }
                }
            }
        } else {
            for y in 0..self.height {
                for x in 0..self.width {
                    unsafe {
                        frame_buffer::get_frame_buffer_reference()
                            .lock()
                            .write_pixel(position.x + x, position.y + y, self.data[y][x].into())
                            .expect("Failed to write a pixel to the frame buffer.")
                    };
                }
            }
        }
    }

    pub fn set_transparent_color(&mut self, color: RgbColor) {
        self.transparent_color = Some(color);
    }
}
