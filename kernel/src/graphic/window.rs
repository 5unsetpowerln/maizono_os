use alloc::sync::Arc;
use common::graphic::{RgbColor, rgb};
use glam::{U64Vec2, u64vec2};
use spin::{Mutex, MutexGuard};

use crate::graphic::PixelWriter;
use crate::graphic::canvas::Canvas;

const CLOSE_BUTTON_HEIGHT: usize = 14;
const CLOSE_BUTTON_WIDTH: usize = 16;

const CLOSE_BUTTON: [[u8; CLOSE_BUTTON_WIDTH]; CLOSE_BUTTON_HEIGHT] = [
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3], // "...............@"
    [0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 3], // ".:::::::::::::$@"
    [0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 3], // ".:::::::::::::$@"
    [0, 1, 1, 1, 3, 3, 1, 1, 1, 1, 3, 3, 1, 1, 2, 3], // ".:::@@::::@@::$@"
    [0, 1, 1, 1, 1, 3, 3, 1, 1, 3, 3, 1, 1, 1, 2, 3], // ".::::@@::@@:::$@"
    [0, 1, 1, 1, 1, 1, 3, 3, 3, 3, 1, 1, 1, 1, 2, 3], // ".:::::@@@@::::$@"
    [0, 1, 1, 1, 1, 1, 1, 3, 3, 1, 1, 1, 1, 1, 2, 3], // ".::::::@@:::::$@"
    [0, 1, 1, 1, 1, 1, 3, 3, 3, 3, 1, 1, 1, 1, 2, 3], // ".:::::@@@@::::$@"
    [0, 1, 1, 1, 1, 3, 3, 1, 1, 3, 3, 1, 1, 1, 2, 3], // ".::::@@::@@:::$@"
    [0, 1, 1, 1, 3, 3, 1, 1, 1, 1, 3, 3, 1, 1, 2, 3], // ".:::@@::::@@::$@"
    [0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 3], // ".:::::::::::::$@"
    [0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 3], // ".:::::::::::::$@"
    [0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3], // ".$$$$$$$$$$$$$$@"
    [3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3], // "@@@@@@@@@@@@@@@@"
];

pub fn draw_window(canvas: Arc<Mutex<Canvas>>, title: &str) {
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

    let mut locked_canvas: MutexGuard<'_, Canvas> = canvas.lock();

    locked_canvas
        .write_string(u64vec2(24, 4), title, rgb(0xffffff))
        .unwrap();

    for (y, row) in CLOSE_BUTTON.iter().enumerate() {
        for (x, p) in row.iter().enumerate() {
            let color = match p {
                1 => rgb(0xc6c6c6),
                2 => rgb(0x848484),
                3 => rgb(0x000000),
                _ => rgb(0xffffff),
            };
            locked_canvas
                .write_pixel(
                    u64vec2(
                        canvas_width - 5 - CLOSE_BUTTON_WIDTH as u64 + x as u64,
                        5 + y as u64,
                    ),
                    color,
                )
                .unwrap();
        }
    }
}
