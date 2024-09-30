use core::{
    fmt::{self},
    str,
};

use common::graphic::RgbColor;
use spin::{Mutex, MutexGuard};

use crate::error::Result;

use super::{
    font::{self, CHARACTER_HEIGHT, CHARACTER_WIDTH},
    frame_buffer,
};

const ROWS: usize = 25;
const COLUMNS: usize = 150;

pub static mut CONSOLE: Mutex<Option<Console>> = Mutex::new(None);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleError {
    UninitializedError,
    ConsoleLockError,
}

pub struct Console {
    buffer: [[char; COLUMNS]; ROWS],
    bg_color: RgbColor,
    fg_color: RgbColor,
    cursor_row: usize,
    cursor_column: usize,
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.print(s);
        Ok(())
    }
}

impl Console {
    fn new(bg_color: RgbColor, fg_color: RgbColor) -> Result<Self> {
        frame_buffer::fill_rect(
            0,
            0,
            COLUMNS * CHARACTER_WIDTH,
            ROWS * CHARACTER_HEIGHT,
            bg_color,
        )?;
        Ok(Self {
            buffer: [['\x00'; COLUMNS]; ROWS],
            bg_color,
            fg_color,
            cursor_row: 0,
            cursor_column: 0,
        })
    }

    fn new_line(&mut self) {
        self.cursor_column = 0;
        if self.cursor_row < ROWS - 1 {
            self.cursor_row += 1;
        } else {
            for y in 0..ROWS * font::CHARACTER_HEIGHT {
                for x in 0..COLUMNS * font::CHARACTER_WIDTH {
                    frame_buffer::write_pixel(x, y, self.bg_color.into())
                        .expect("Failed to write a pixel.");
                }
            }

            for row in 0..ROWS - 1 {
                self.buffer[row] = self.buffer[row + 1];

                let mut s_buf = [0; COLUMNS * 4];
                let mut pos = 0;

                for c in self.buffer[row] {
                    if c == '\x00' {
                        break;
                    }
                    let c_bytes = c.encode_utf8(&mut s_buf[pos..]);
                    pos += c_bytes.len();
                }

                let s = str::from_utf8(&s_buf[..pos]).expect("utf-8 decode error");
                frame_buffer::write_string(0, row * font::CHARACTER_HEIGHT, s, self.fg_color)
                    .expect("Failed to write string.");
            }

            *self.buffer.last_mut().expect("console buffer is empty") = ['\x00'; COLUMNS];
        }
    }

    fn print(&mut self, s: &str) {
        for c in s.chars() {
            if c == '\n' {
                self.new_line();
            } else if self.cursor_column < COLUMNS - 1 {
                frame_buffer::write_char(
                    font::CHARACTER_WIDTH * self.cursor_column,
                    font::CHARACTER_HEIGHT * self.cursor_row,
                    c,
                    self.fg_color,
                )
                .expect("Failed to write a character.");
                self.buffer[self.cursor_row][self.cursor_column] = c;
                self.cursor_column += 1;
            }
        }
    }

    fn println(&mut self, s: &str) {
        self.print(s);
        self.new_line();
    }
}

fn lock_console<'a>() -> Result<MutexGuard<'a, Option<Console>>> {
    match unsafe { CONSOLE.try_lock() } {
        Some(lock) => Ok(lock),
        None => Err(ConsoleError::ConsoleLockError.into()),
    }
}

#[macro_export]
macro_rules! printk {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        crate::graphic::console::println("").unwrap();
        match unsafe { crate::graphic::console::CONSOLE.try_lock() }.as_mut() {
            Some(lock) => match lock.as_mut() {
                Some(console) => {
                    console.write_fmt(core::format_args!($($arg)*)).unwrap();
                }
                None => {
                    panic!();
                }
            },
            None => panic!(),
        };
    }};
}

pub fn init(bg: RgbColor, fg: RgbColor) -> Result<()> {
    lock_console()?.replace(Console::new(bg, fg)?);
    Ok(())
}

pub fn print(s: &str) -> Result<()> {
    lock_console()?
        .as_mut()
        .ok_or(ConsoleError::ConsoleLockError)?
        .print(s);
    Ok(())
}

pub fn println(s: &str) -> Result<()> {
    lock_console()?
        .as_mut()
        .ok_or(ConsoleError::ConsoleLockError)?
        .println(s);
    Ok(())
}
