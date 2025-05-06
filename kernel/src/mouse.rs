use alloc::sync::Arc;
use common::{graphic::RgbColor, matrix::Vec2};
use spin::{Lazy, Mutex};

use crate::{device::ps2, error::Error, graphic::PixelWriter, kprintln};

use super::frame_buffer;

pub const MOUSE_CURSOR_WIDTH: usize = 15;
pub const MOUSE_CURSOR_HEIGHT: usize = 24;

#[derive(Debug, Copy, Clone)]
enum MousePixel {
    Null,
    Border(RgbColor),
    Inner(RgbColor),
}

pub const MOUSE_TRANSPARENT_COLOR: RgbColor = RgbColor::rgb(0, 0, 1);

const MOUSE_CURSOR_DATA: [[MousePixel; MOUSE_CURSOR_WIDTH]; MOUSE_CURSOR_HEIGHT] = {
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

    let mut mouse_cursor = [[MousePixel::Null; MOUSE_CURSOR_WIDTH]; MOUSE_CURSOR_HEIGHT];

    let mut dy = 0;
    while dy < MOUSE_CURSOR_HEIGHT {
        let mut dx = 0;
        while dx < MOUSE_CURSOR_WIDTH {
            mouse_cursor[dy][dx] = match MOUSE_CURSOR_SHAPE[dy][dx] {
                0 => MousePixel::Null,
                1 => MousePixel::Border(RgbColor::from(0xffffff00)),
                2 => MousePixel::Inner(RgbColor::from(0x0)),
                _ => panic!("unexpected mouse pixel."),
            };
            dx += 1;
        }
        dy += 1;
    }

    mouse_cursor
};

pub enum MouseEvent {
    Move { displacement: Vec2<isize> },
    LeftClick,
    MiddleClick,
    RightClick,
}

type ThreadSafeSharedPixelWriter = Arc<Mutex<dyn PixelWriter>>;

pub fn draw_mouse_cursor<'a>(writer: ThreadSafeSharedPixelWriter, position: Vec2<isize>) {
    let mut writer = writer.lock();
    for dy in 0..MOUSE_CURSOR_HEIGHT {
        for dx in 0..MOUSE_CURSOR_WIDTH {
            match MOUSE_CURSOR_DATA[dy][dx] {
                MousePixel::Border(color) => writer
                    .write_pixel(position.x as usize + dx, position.y as usize + dy, color)
                    .expect("Failed to write a pixel to the writer."),
                MousePixel::Inner(color) => writer
                    .write_pixel(position.x as usize + dx, position.y as usize + dy, color)
                    .expect("Failed to write a pixel to the writer"),
                MousePixel::Null => {}
            }
        }
    }
}

pub fn mouse_observer(dx: isize, dy: isize) {}
