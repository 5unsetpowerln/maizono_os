use core::{ascii, fmt::Debug, ops::Deref};

use alloc::sync::Arc;
use common::graphic::RgbColor;
use font::{GARBLED_FONT, U8_FONT};
use glam::{U64Vec2, u64vec2};
use spin::Mutex;

use crate::{
    error::Result,
    graphic::{canvas::Canvas, layer::Layer},
};

pub trait PixelWriter: Debug {
    fn width(&self) -> u64;
    fn height(&self) -> u64;
    fn write_pixel(&mut self, position: U64Vec2, color: RgbColor) -> Result<()>;

    fn write_char(&mut self, position: U64Vec2, ascii: ascii::Char, fg: RgbColor) -> Result<()> {
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
                    self.write_pixel(
                        U64Vec2::from((position.x + dx as u64, position.y + dy as u64)),
                        fg,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn fill_rect(
        &mut self,
        position: U64Vec2,
        width: u64,
        height: u64,
        color: RgbColor,
    ) -> Result<()> {
        for x in position.x..position.x + width {
            for y in position.y..position.y + height {
                let pos = u64vec2(x, y);
                self.write_pixel(pos, color)?;
            }
        }
        Ok(())
    }

    fn fill(&mut self, color: RgbColor) -> Result<()> {
        for x in 0..self.width() {
            for y in 0..self.height() {
                match self.write_pixel(U64Vec2 { x, y }, color) {
                    Ok(_) => continue,
                    Err(err) => return Err(err),
                }
            }
        }
        Ok(())
    }

    fn draw_rect(
        &mut self,
        position: U64Vec2,
        width: u64,
        height: u64,
        color: RgbColor,
    ) -> Result<()> {
        for x in position.x..position.x + width {
            self.write_pixel(U64Vec2 { x, y: position.y }, color)?;
            self.write_pixel(
                U64Vec2 {
                    x,
                    y: position.y + height - 1,
                },
                color,
            )?;
        }
        for y in position.y..position.y + height {
            self.write_pixel(U64Vec2 { x: position.x, y }, color)?;
            self.write_pixel(
                U64Vec2 {
                    x: position.x + width - 1,
                    y,
                },
                color,
            )?;
        }
        Ok(())
    }

    fn write_string(&mut self, pos: U64Vec2, s: &str, fg: RgbColor) -> Result<()> {
        for (i, c) in s
            .as_ascii()
            .expect("Non ascii character is given.")
            .iter()
            .enumerate()
        {
            self.write_char(
                pos + u64vec2(i as u64 * font::CHARACTER_WIDTH as u64, 0),
                *c,
                fg,
            )?;
        }

        Ok(())
    }
}

pub trait PixelWriterCopyable: PixelWriter {
    fn copy_internal_buffer(&mut self, position: U64Vec2, src: &[u32]);
}

pub struct Rectangle {
    pub pos: U64Vec2,
    pub width: u64,
    pub height: u64,
}

pub fn rectangle(pos: U64Vec2, width: u64, height: u64) -> Rectangle {
    Rectangle { pos, width, height }
}

pub fn create_canvas_and_layer(
    width: u64,
    height: u64,
    consider_transparent: bool,
) -> (Arc<Mutex<Canvas>>, Layer) {
    let canvas = create_arc_mutex_canvas(width, height, consider_transparent);
    let layer = Layer::new(canvas.clone());
    (canvas, layer)
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

pub mod canvas;
pub mod char;
pub mod console;
pub mod font;
pub mod frame_buffer;
pub mod layer;
// pub mod window;
