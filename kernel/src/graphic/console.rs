use core::{
    ascii,
    fmt::{self},
    mem::MaybeUninit,
    str,
};

use alloc::sync::Arc;
use common::graphic::RgbColor;
use glam::{U64Vec2, u64vec2};
use spin::{Mutex, Once};
use thiserror_no_std::Error;

use crate::{allocator::Locked, error::Result};

use super::{
    PixelWriter,
    font::{self, CHARACTER_HEIGHT, CHARACTER_WIDTH},
};

const ROWS: usize = 25;
const COLUMNS: usize = 150;
pub const WIDTH: usize = COLUMNS * CHARACTER_WIDTH;
pub const HEIGHT: usize = ROWS * CHARACTER_HEIGHT;

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

static CONSOLE: Locked<Console> = Locked::new(Console::new());
static IS_INITIALIZED: Once<()> = Once::new();

type ThreadSafeSharedPixelWriter = Arc<Mutex<(dyn PixelWriter + Send)>>;

pub struct Console {
    buffer: [Line<COLUMNS>; ROWS],
    bg_color: RgbColor,
    fg_color: RgbColor,
    cursor_row: u64,
    cursor_column: u64,
    writer: MaybeUninit<ThreadSafeSharedPixelWriter>,
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.print(s);
        Ok(())
    }
}

impl Console {
    const fn new() -> Self {
        Self {
            buffer: [Line::<COLUMNS>::null(); ROWS],
            bg_color: RgbColor::rgb(0x28, 0x28, 0x28, false),
            fg_color: RgbColor::rgb(0x28, 0x28, 0x28, false),
            cursor_row: 0,
            cursor_column: 0,
            writer: MaybeUninit::uninit(),
        }
    }

    fn init(
        &mut self,
        writer: ThreadSafeSharedPixelWriter,
        bg_color: RgbColor,
        fg_color: RgbColor,
    ) -> Result<()> {
        writer
            .lock()
            .fill_rect(u64vec2(0, 0), WIDTH as u64, HEIGHT as u64, bg_color)?;

        *self = Self {
            buffer: [Line::<COLUMNS>::null(); ROWS],
            bg_color,
            fg_color,
            cursor_row: 0,
            cursor_column: 0,
            writer: MaybeUninit::new(writer),
        };

        Ok(())
    }

    pub fn set_writer(&mut self, writer: ThreadSafeSharedPixelWriter) {
        self.writer = MaybeUninit::new(writer);
    }

    fn new_line(&mut self) {
        let writer = unsafe { &*self.writer.as_ptr() };

        self.cursor_column = 0;
        if self.cursor_row < ROWS as u64 - 1 {
            self.cursor_row += 1;
        } else {
            writer
                .lock()
                .fill_rect(
                    u64vec2(0, 0),
                    (COLUMNS * CHARACTER_WIDTH) as u64,
                    (ROWS * CHARACTER_HEIGHT) as u64,
                    self.bg_color.into(),
                )
                .expect("Failed to fill up the console.");

            for row in 0..ROWS - 1 {
                self.buffer[row] = self.buffer[row + 1];
                let mut writer_locked = writer.lock();

                let line = self.buffer[row];
                for (i, c) in line.chars[0..line.length].iter().enumerate() {
                    writer_locked
                        .write_char(
                            u64vec2(
                                (font::CHARACTER_WIDTH * i) as u64,
                                (font::CHARACTER_HEIGHT * row) as u64,
                            ),
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
            } else if self.cursor_column < COLUMNS as u64 - 1 {
                let writer = unsafe { &*self.writer.as_ptr() };

                writer
                    .lock()
                    .write_char(
                        u64vec2(
                            font::CHARACTER_WIDTH as u64 * self.cursor_column,
                            font::CHARACTER_HEIGHT as u64 * self.cursor_row,
                        ),
                        *c,
                        self.fg_color,
                    )
                    .unwrap();
                self.buffer[self.cursor_row as usize].push(*c).unwrap();
                self.cursor_column += 1;
            }
        }
    }
}

pub fn init(
    writer: ThreadSafeSharedPixelWriter,
    bg_color: RgbColor,
    fg_color: RgbColor,
) -> Result<()> {
    let mut console = CONSOLE.lock();
    console.init(writer, bg_color, fg_color)?;
    IS_INITIALIZED.call_once(|| ());
    Ok(())
}

pub fn is_initialized() -> bool {
    IS_INITIALIZED.is_completed()
}

pub fn get_console_reference() -> &'static Locked<Console> {
    &CONSOLE
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        crate::graphic::console::get_console_reference().lock().write_fmt(format_args!($($arg)*)).unwrap();
    }};
}

#[macro_export]
macro_rules! kprintln {
    () => (kprint!("\n"));
    ($($arg:tt)*) => ($crate::kprint!("{}\n", format_args!($($arg)*)));
}
