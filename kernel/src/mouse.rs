use alloc::sync::Arc;
use common::graphic::RgbColor;
use glam::{I64Vec2, U64Vec2, u64vec2};
use spin::{Lazy, Mutex};

use crate::graphic::PixelWriter;

pub const MOUSE_CURSOR_WIDTH: usize = 15;
pub const MOUSE_CURSOR_HEIGHT: usize = 24;

#[derive(Debug, Copy, Clone)]
enum MousePixel {
    Transparent(RgbColor),
    Border(RgbColor),
    Inner(RgbColor),
}

pub const MOUSE_TRANSPARENT_COLOR: RgbColor = RgbColor::from(0);

static MOUSE_CURSOR_DATA: Lazy<[[MousePixel; MOUSE_CURSOR_WIDTH]; MOUSE_CURSOR_HEIGHT]> =
    Lazy::new(|| {
        const MOUSE_CURSOR_SHAPE: [[u8; MOUSE_CURSOR_WIDTH]; MOUSE_CURSOR_HEIGHT] = [
            [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [1, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [1, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [1, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [1, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [1, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            [1, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0],
            [1, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0],
            [1, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0],
            [1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0],
            [1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0],
            [1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0, 0],
            [1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 0],
            [1, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1],
            [1, 2, 2, 2, 2, 2, 2, 1, 0, 0, 0, 0, 0, 0, 0],
            [1, 2, 2, 2, 2, 1, 1, 2, 1, 0, 0, 0, 0, 0, 0],
            [1, 2, 2, 2, 1, 0, 1, 2, 1, 0, 0, 0, 0, 0, 0],
            [1, 2, 2, 1, 0, 0, 0, 1, 2, 1, 0, 0, 0, 0, 0],
            [1, 2, 1, 0, 0, 0, 0, 1, 2, 1, 0, 0, 0, 0, 0],
            [1, 1, 0, 0, 0, 0, 0, 0, 1, 2, 1, 0, 0, 0, 0],
            [1, 0, 0, 0, 0, 0, 0, 0, 1, 2, 1, 0, 0, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 1, 0, 0, 0],
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0],
        ];

        let mut mouse_cursor = [[MousePixel::Transparent(RgbColor::transparent());
            MOUSE_CURSOR_WIDTH]; MOUSE_CURSOR_HEIGHT];
        for dy in 0..MOUSE_CURSOR_HEIGHT {
            for dx in 0..MOUSE_CURSOR_WIDTH {
                mouse_cursor[dy][dx] = match MOUSE_CURSOR_SHAPE[dy][dx] {
                    0 => continue,
                    1 => MousePixel::Border(RgbColor::from(0xffffff00)),
                    2 => MousePixel::Inner(RgbColor::from(0xcc241d00)),
                    _ => panic!("unexpected mouse pixel."),
                };
            }
        }

        mouse_cursor
    });

#[derive(Debug)]
pub enum MouseEvent {
    Move { displacement: I64Vec2 },
    LeftClick,
    MiddleClick,
    RightClick,
}

type ThreadSafeSharedPixelWriter = Arc<Mutex<dyn PixelWriter>>;

pub fn draw_mouse_cursor<'a>(writer: ThreadSafeSharedPixelWriter, position: U64Vec2) {
    let mut writer = writer.lock();
    for dy in 0..MOUSE_CURSOR_HEIGHT as u64 {
        for dx in 0..MOUSE_CURSOR_WIDTH as u64 {
            match MOUSE_CURSOR_DATA[dy as usize][dx as usize] {
                MousePixel::Border(color) => writer
                    .write_pixel(u64vec2(position.x + dx, position.y + dy), color)
                    .expect("Failed to write a pixel to the writer."),
                MousePixel::Inner(color) => writer
                    .write_pixel(u64vec2(position.x + dx, position.y + dy), color)
                    .expect("Failed to write a pixel to the writer"),
                MousePixel::Transparent(color) => writer
                    .write_pixel(u64vec2(position.x + dx, position.y + dy), color)
                    .expect("Failed to write a pixel to the writer"),
            }
        }
    }
}

pub fn mouse_observer(dx: isize, dy: isize) {}
