use core::{
    ascii,
    fmt::{self},
    mem::MaybeUninit,
    str,
};

use alloc::sync::Arc;
use ascii::Char;
use common::graphic::{RgbColor, rgb};
use glam::u64vec2;
use spin::{Mutex, Once};
use thiserror_no_std::Error;
use x86_64::instructions::interrupts::without_interrupts;

use crate::{allocator::Locked, error::Result, graphic::canvas::Canvas, serial_println};

use super::{
    PixelWriter,
    font::{self, CHARACTER_HEIGHT, CHARACTER_WIDTH},
    rectangle,
};

const ROWS: usize = 25;
const COLUMNS: usize = 100;
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
    cursor: usize,
}

impl<const CAP: usize> Line<CAP> {
    // pub fn new(chars: [ascii::Char; CAP], length: usize) -> Result<Self> {
    //     if length > CAP {
    //         return Err(ConsoleError::LineLengthOverflow.into());
    //     }

    //     return Ok(Self { chars, length });
    // }

    pub const fn null() -> Self {
        Self {
            chars: [ascii::Char::from_u8(0).unwrap(); CAP],
            cursor: 0,
        }
    }

    pub fn push(&mut self, char: ascii::Char) -> Result<()> {
        if self.cursor >= CAP {
            return Err(ConsoleError::LineLengthOverflow.into());
        }

        self.chars[self.cursor] = char;
        self.cursor += 1;

        Ok(())
    }
}

static CONSOLE: Locked<Console> = Locked::new(Console::new());
static IS_INITIALIZED: Once<()> = Once::new();

pub struct Console {
    buffer: [Line<COLUMNS>; ROWS],
    bg_color: RgbColor,
    fg_color: RgbColor,
    cursor_row: u64,
    cursor_column: u64,
    canvas: MaybeUninit<Arc<Mutex<Canvas>>>,
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
            bg_color: rgb(0x282828),
            fg_color: rgb(0x282828),
            cursor_row: 0,
            cursor_column: 0,
            canvas: MaybeUninit::uninit(),
        }
    }

    fn init(
        &mut self,
        canvas: Arc<Mutex<Canvas>>,
        bg_color: RgbColor,
        fg_color: RgbColor,
    ) -> Result<()> {
        canvas
            .lock()
            .fill_rect(u64vec2(0, 0), WIDTH as u64, HEIGHT as u64, bg_color)?;

        *self = Self {
            buffer: [Line::<COLUMNS>::null(); ROWS],
            bg_color,
            fg_color,
            cursor_row: 0,
            cursor_column: 0,
            canvas: MaybeUninit::new(canvas),
        };

        Ok(())
    }

    fn new_line(&mut self) {
        self.cursor_column = 0;
        if self.cursor_row < ROWS as u64 - 1 {
            self.cursor_row += 1;
        } else {
            let move_src_rect = rectangle(
                u64vec2(0, CHARACTER_HEIGHT as u64),
                WIDTH as u64,
                (HEIGHT - CHARACTER_HEIGHT) as u64,
            );

            self.get_writer()
                .lock()
                .move_rect(u64vec2(0, 0), move_src_rect);
            self.get_writer()
                .lock()
                .fill_rect(
                    u64vec2(0, (CHARACTER_HEIGHT * (ROWS - 1)) as u64),
                    WIDTH as u64,
                    CHARACTER_HEIGHT as u64,
                    self.bg_color,
                )
                .unwrap();
            *self.buffer.last_mut().expect("console buffer is empty") = Line::<COLUMNS>::null();
        }
    }

    fn backspace(&mut self) {
        if self.cursor_column == 0 {
            if self.cursor_row != 0 {
                self.cursor_row -= 1;
                self.buffer[self.cursor_row as usize].cursor -= 1;
                self.cursor_column = self.buffer[self.cursor_row as usize].cursor as u64;
            }
        } else {
            self.cursor_column -= 1;
            serial_println!("{}", self.buffer[self.cursor_row as usize].cursor);
            self.buffer[self.cursor_row as usize].cursor -= 1;
        }

        self.get_writer()
            .lock()
            .fill_rect(
                u64vec2(
                    font::CHARACTER_WIDTH as u64 * self.cursor_column,
                    font::CHARACTER_HEIGHT as u64 * self.cursor_row,
                ),
                font::CHARACTER_WIDTH as u64,
                font::CHARACTER_HEIGHT as u64,
                self.bg_color,
            )
            .unwrap();
    }

    fn get_writer<'a>(&mut self) -> &'a Arc<Mutex<Canvas>> {
        (unsafe { &*self.canvas.as_ptr() }) as _
    }

    fn print_char(&mut self, c: Char) {
        self.get_writer()
            .lock()
            .write_char(
                u64vec2(
                    font::CHARACTER_WIDTH as u64 * self.cursor_column,
                    font::CHARACTER_HEIGHT as u64 * self.cursor_row,
                ),
                c,
                self.fg_color,
            )
            .unwrap();

        self.buffer[self.cursor_row as usize].push(c).unwrap();

        if self.cursor_column == COLUMNS as u64 - 1 {
            self.new_line()
        } else {
            self.cursor_column += 1;
        }
    }

    fn print_cursor(&mut self) {
        self.get_writer()
            .lock()
            .fill_rect(
                u64vec2(
                    self.cursor_column * CHARACTER_WIDTH as u64,
                    self.cursor_row * CHARACTER_HEIGHT as u64,
                ),
                CHARACTER_WIDTH as u64 / 4,
                CHARACTER_HEIGHT as u64,
                self.fg_color,
            )
            .unwrap();
    }

    fn erase_cursor(&mut self) {
        self.get_writer()
            .lock()
            .fill_rect(
                u64vec2(
                    self.cursor_column * CHARACTER_WIDTH as u64,
                    self.cursor_row * CHARACTER_HEIGHT as u64,
                ),
                CHARACTER_WIDTH as u64 / 4,
                CHARACTER_HEIGHT as u64,
                self.bg_color,
            )
            .unwrap();
    }

    fn print(&mut self, s: &str) {
        self.erase_cursor();
        for c in s.as_ascii().expect("Non ascii character is given.") {
            match *c {
                Char::LineFeed => self.new_line(),
                Char::Backspace => self.backspace(),
                _ => {
                    self.print_char(*c);
                }
            }
        }
        self.print_cursor();
    }
}

pub fn init(canvas: Arc<Mutex<Canvas>>, bg_color: RgbColor, fg_color: RgbColor) -> Result<()> {
    let mut console = CONSOLE.lock();
    console.init(canvas, bg_color, fg_color)?;
    IS_INITIALIZED.call_once(|| ());
    Ok(())
}

pub fn is_initialized() -> bool {
    IS_INITIALIZED.is_completed()
}

pub fn get_console_reference() -> &'static Locked<Console> {
    &CONSOLE
}

pub fn _print(args: ::core::fmt::Arguments) {
    // #[cfg(feature = "logging_in_interrupt_handler")]
    // x86_64::instructions::interrupts::disable();

    without_interrupts(|| {
        use core::fmt::Write;
        get_console_reference()
            .lock()
            .write_fmt(args)
            .expect("Failed to print string to console.");
    });

    // #[cfg(feature = "logging_in_interrupt_handler")]
    // x86_64::instructions::interrupts::enable();
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => {{
        crate::graphic::console::_print(format_args!($($arg)*));
    }};
}

#[macro_export]
macro_rules! kprintln {
    () => (kprint!("\n"));
    ($($arg:tt)*) => ($crate::kprint!("{}\n", format_args!($($arg)*)));
}
