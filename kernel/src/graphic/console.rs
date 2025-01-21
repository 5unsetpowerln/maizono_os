use core::{
    ascii,
    fmt::{self},
    str,
};

use common::graphic::RgbColor;
use spin::{Mutex, MutexGuard};
use thiserror_no_std::Error;

use crate::error::Result;

use super::{
    font::{self, CHARACTER_HEIGHT, CHARACTER_WIDTH},
    frame_buffer,
};

const ROWS: usize = 25;
const COLUMNS: usize = 150;

static CONSOLE: Mutex<Console> = Mutex::new(Console::new_empty());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ConsoleError {
    #[error("The console is not initialized yet.")]
    UninitializedError,
    #[error("Failed to lock the console.")]
    ConsoleLockError,
    #[error("The number of characters in the line overflowed the capacity.")]
    LineLengthOverflow,
}

#[derive(Debug, Clone, Copy)]
pub struct Line<const CAP: usize> {
    chars: [ascii::Char; CAP],
    length: usize,
}

impl<const CAP: usize> Line<CAP> {
    pub fn new(chars: [ascii::Char; CAP], length: usize) -> Result<Self> {
        if length > CAP {
            return Err(ConsoleError::LineLengthOverflow.into());
        }

        return Ok(Self { chars, length });
    }

    pub const fn null() -> Self {
        Self {
            chars: [ascii::Char::from_u8(0).unwrap(); CAP],
            length: 0,
        }
    }

    pub fn push(&mut self, char: ascii::Char) -> Result<()> {
        if self.length == CAP {
            return Err(ConsoleError::LineLengthOverflow.into());
        }

        self.chars[self.length] = char;
        self.length += 1;

        Ok(())
    }
}

pub struct Console {
    buffer: [Line<COLUMNS>; ROWS],
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
    const fn new_empty() -> Self {
        Self {
            buffer: [Line::<COLUMNS>::null(); ROWS],
            bg_color: RgbColor::rgb(0x28, 0x28, 0x28),
            fg_color: RgbColor::rgb(0x28, 0x28, 0x28),
            cursor_row: 0,
            cursor_column: 0,
        }
    }

    pub fn init(&mut self, bg_color: RgbColor, fg_color: RgbColor) -> Result<()> {
        frame_buffer::fill_rect(
            0,
            0,
            COLUMNS * CHARACTER_WIDTH,
            ROWS * CHARACTER_HEIGHT,
            bg_color,
        )?;
        *self = Self {
            buffer: [Line::<COLUMNS>::null(); ROWS],
            bg_color,
            fg_color,
            cursor_row: 0,
            cursor_column: 0,
        };
        Ok(())
    }

    fn new_line(&mut self) {
        self.cursor_column = 0;
        if self.cursor_row < ROWS - 1 {
            self.cursor_row += 1;
        } else {
            frame_buffer::fill_rect(
                0,
                0,
                COLUMNS * CHARACTER_WIDTH,
                ROWS * CHARACTER_HEIGHT,
                self.bg_color.into(),
            )
            .expect("Failed to fill up the console.");

            for row in 0..ROWS - 1 {
                self.buffer[row] = self.buffer[row + 1];

                let line = self.buffer[row];
                for (i, c) in line.chars[0..line.length].iter().enumerate() {
                    frame_buffer::write_char(
                        font::CHARACTER_WIDTH * i,
                        font::CHARACTER_HEIGHT * row,
                        *c,
                        self.fg_color,
                    )
                    .unwrap();
                }
            }

            *self.buffer.last_mut().expect("console buffer is empty") = Line::<COLUMNS>::null();
        }
    }

    fn print(&mut self, s: &str) {
        for c in s.as_ascii().expect("Non ascii character is given.") {
            if *c == ascii::Char::LineFeed {
                self.new_line()
            } else if self.cursor_column < COLUMNS - 1 {
                frame_buffer::write_char(
                    font::CHARACTER_WIDTH * self.cursor_column,
                    font::CHARACTER_HEIGHT * self.cursor_row,
                    *c,
                    self.fg_color,
                )
                .unwrap();
                self.buffer[self.cursor_row].push(*c).unwrap();
                self.cursor_column += 1;
            }
        }
    }

    fn println(&mut self, s: &str) {
        self.print(s);
        self.new_line();
    }
}

pub fn console() -> Result<MutexGuard<'static, Console>> {
    match { CONSOLE.try_lock() } {
        Some(lock) => Ok(lock),
        None => Err(ConsoleError::ConsoleLockError.into()),
    }
}

#[macro_export]
macro_rules! kprintln {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        crate::graphic::console::console().unwrap().write_fmt(core::format_args!($($arg)*)).unwrap();
        crate::graphic::console::println("").unwrap();
    }};
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        crate::graphic::console::console().unwrap().write_fmt(core::format_args!($($arg)*)).unwrap();
    }};
}

pub fn print(s: &str) -> Result<()> {
    console()?.print(s);
    Ok(())
}

pub fn println(s: &str) -> Result<()> {
    console()?.println(s);
    Ok(())
}
