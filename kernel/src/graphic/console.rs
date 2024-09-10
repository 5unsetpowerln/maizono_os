use core::{
    fmt::{self},
    str,
};

use common::graphic::RgbColor;
use once_cell::sync::Lazy;

use crate::error::Result;

use super::{font::FONT, framebuffer};

const ROWS: usize = 25;
const COLUMNS: usize = 80;

pub static mut CONSOLE: Lazy<Option<Console>> = Lazy::new(|| None);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleError {
    UninitializedError,
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
    fn new(bg_color: RgbColor, fg_color: RgbColor) -> Self {
        Self {
            buffer: [['\x00'; COLUMNS]; ROWS],
            bg_color,
            fg_color,
            cursor_row: 0,
            cursor_column: 0,
        }
    }

    fn new_line(&mut self) {
        self.cursor_column = 0;
        if self.cursor_row < ROWS - 1 {
            self.cursor_row += 1;
            return;
        } else {
            for y in 0..ROWS * FONT.height {
                for x in 0..COLUMNS * FONT.width {
                    framebuffer::write_pixel(x, y, self.bg_color.into());
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
                framebuffer::write_string(0, row * FONT.height, s, self.fg_color);
            }

            *self.buffer.last_mut().expect("console buffer is empty") = ['\x00'; COLUMNS];
        }
    }

    fn print(&mut self, s: &str) {
        for c in s.chars() {
            if c == '\n' {
                self.new_line();
            } else if self.cursor_column < COLUMNS - 1 {
                framebuffer::write_char(
                    FONT.width * self.cursor_column,
                    FONT.height * self.cursor_row,
                    c,
                    self.fg_color,
                );
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

pub fn init(bg: RgbColor, fg: RgbColor) {
    unsafe {
        CONSOLE.replace(Console::new(bg, fg));
    }
}

macro_rules! call_console_method {
    ($method:ident, $arg:expr) => {
        unsafe {
            self::CONSOLE
                .as_mut()
                .ok_or(ConsoleError::UninitializedError)?
                .$method($arg);
        }
    };
}

pub fn print(s: &str) -> Result<()> {
    call_console_method!(print, s);
    Ok(())
}

pub fn println(s: &str) -> Result<()> {
    call_console_method!(println, s);
    Ok(())
}

#[macro_export]
macro_rules! printk {
    ($($arg:tt)*) => {{
        unsafe {
            use core::fmt::Write;
            crate::graphic::console::CONSOLE
                .as_mut()
                .ok_or(crate::graphic::console::ConsoleError::UninitializedError)
                .expect(
                    "\
                    Console wasn't initialized. \
                    As this error caused in printk! macro, \
                    it isn't impossible to return the error so, \
                    the kernel panicked.",
                ).write_fmt(core::format_args!($($arg)*));
            crate::graphic::console::println("");
        }
    }};
}
