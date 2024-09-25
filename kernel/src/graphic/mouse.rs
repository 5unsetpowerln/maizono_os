use common::graphic::RgbColor;

use crate::printk;

use super::frame_buffer;

pub const CURSOR_WIDTH: usize = 15;
pub const CURSOR_HEIGHT: usize = 15;

const CURSOR_SHAPE_STR: [&str; 15] = [
    "      @@@      ",
    "    @@...@@    ",
    "   @.......@   ",
    "  @...@@@...@  ",
    " @...@   @...@ ",
    " @..@     @..@ ",
    "@..@       @..@",
    "@..@       @..@",
    "@..@       @..@",
    " @..@     @..@ ",
    " @...@   @...@ ",
    "  @...@@@...@  ",
    "   @.......@   ",
    "    @@...@@    ",
    "      @@@      ",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseError {
    UninitializedError,
}

pub fn draw_cursor() {
    for (y, row) in CURSOR_SHAPE_STR.into_iter().enumerate() {
        if y >= CURSOR_HEIGHT {
            printk!(
                "CURSOR_HEIGHT was defined to be {} but, \
                        CURSOR_SHAPE_STR is {} in height.",
                CURSOR_HEIGHT,
                y + 1
            );
        }
        for (x, c) in row.chars().enumerate() {
            if x >= CURSOR_WIDTH {
                printk!(
                    "CURSOR_WIDTH was defined to be {} but, \
                                    CURSOR_SHAPE_STR is {} in width.",
                    CURSOR_WIDTH,
                    x + 1
                );
            }
            match c {
                ' ' => continue,
                '.' => {
                    frame_buffer::write_pixel(x, y, RgbColor::from(0x689d6a00).into());
                }
                '@' => {
                    frame_buffer::write_pixel(x, y, RgbColor::from(0xfbf1c700).into());
                }
                other => {
                    printk!(
                        "There is an unexpected character \"{}\" in CURSOR_SHAPE_STR",
                        other
                    );
                }
            };
        }
    }
}
