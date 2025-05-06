use core::{ascii, ops::Deref};

use common::graphic::RgbColor;
use font::{GARBLED_FONT, U8_FONT};

use crate::error::Result;

pub mod char;
pub mod console;
pub mod font;
pub mod frame_buffer;

pub trait PixelWriter {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn write_pixel(&mut self, x: usize, y: usize, color: RgbColor) -> Result<()>;

    fn write_char(&mut self, x: usize, y: usize, ascii: ascii::Char, fg: RgbColor) -> Result<()> {
        let glyph_index = ascii as usize;
        let glyph = {
            if glyph_index >= U8_FONT.len() {
                GARBLED_FONT
            } else {
                U8_FONT[glyph_index]
            }
        };

        for (dy, row) in glyph.iter().enumerate() {
            for dx in 0..font::CHARACTER_WIDTH {
                if (row >> 7 - dx) & 1 == 1 {
                    self.write_pixel(x + dx, y + dy, fg)?;
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
                self.write_pixel(x_inner, y_inner, color)?;
            }
        }
        Ok(())
    }

    fn fill(&mut self, color: RgbColor) -> Result<()> {
        for x in 0..self.width() {
            for y in 0..self.height() {
                match self.write_pixel(x, y, color) {
                    Ok(_) => continue,
                    Err(err) => return Err(err),
                }
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
            self.write_pixel(x_inner, y, color)?;
            self.write_pixel(x_inner, y + height - 1, color)?;
        }
        for y_inner in y..y + height {
            self.write_pixel(x, y_inner, color)?;
            self.write_pixel(x + width - 1, y_inner, color)?;
        }
        Ok(())
    }

    fn write_string(&mut self, x: usize, y: usize, data: &str, fg: RgbColor) -> Result<()> {
        for (i, c) in data
            .as_ascii()
            .expect("non ascii string is given.")
            .iter()
            .enumerate()
        {
            self.write_char(x + i * font::CHARACTER_WIDTH * 2, y, c.clone(), fg)?;
        }
        Ok(())
    }
}
