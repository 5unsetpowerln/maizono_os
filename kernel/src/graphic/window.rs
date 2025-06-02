use alloc::sync::Arc;
use common::graphic::{RgbColor, rgb};
use glam::{U64Vec2, u64vec2};
use spin::Mutex;

use crate::graphic::PixelWriter;
use crate::graphic::canvas::Canvas;

pub fn draw_window(canvas: Arc<Mutex<Canvas>>) {
    let fill_rect = |pos: U64Vec2, width: u64, height: u64, c: RgbColor| {
        canvas.lock().fill_rect(pos, width, height, c).unwrap();
    };

    let canvas_width = canvas.lock().width();
    let canvas_height = canvas.lock().height();

    fill_rect(u64vec2(0, 0), canvas_width, 1, rgb(0xc6c6c6));
    fill_rect(u64vec2(1, 1), canvas_width - 2, 1, rgb(0xffffff));
    fill_rect(u64vec2(0, 0), 1, canvas_height, rgb(0xc6c6c6));
    fill_rect(u64vec2(1, 1), 1, canvas_height - 2, rgb(0xffffff));
    fill_rect(
        u64vec2(canvas_width - 2, 1),
        1,
        canvas_height - 2,
        rgb(0x848484),
    );
    fill_rect(
        u64vec2(canvas_width - 1, 0),
        1,
        canvas_height,
        rgb(0x000000),
    );
    fill_rect(
        u64vec2(2, 2),
        canvas_width - 4,
        canvas_height - 4,
        rgb(0xc6c6c6),
    );
    fill_rect(u64vec2(3, 3), canvas_width - 6, 18, rgb(0x000084));
    fill_rect(
        u64vec2(1, canvas_height - 2),
        canvas_width - 2,
        1,
        rgb(0x848484),
    );
    fill_rect(
        u64vec2(0, canvas_height - 1),
        canvas_width,
        1,
        rgb(0x000000),
    );
}
