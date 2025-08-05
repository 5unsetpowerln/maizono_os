use core::{
    ascii,
    fmt::{self},
    mem::MaybeUninit,
    str,
};

use alloc::{sync::Arc, vec::Vec};
use ascii::Char;
use common::graphic::{RgbColor, rgb};
use glam::u64vec2;
use spin::{Mutex, Once};
use x86_64::instructions::interrupts::without_interrupts;

use crate::{allocator::Locked, error::Result, graphic::canvas::Canvas, serial_println};

use self::line::Line;

use super::{
    PixelWriter,
    font::{self, CHARACTER_HEIGHT, CHARACTER_WIDTH},
    rectangle,
};

const ROWS: usize = 25;
const COLUMNS: usize = 100;
pub const WIDTH: usize = COLUMNS * CHARACTER_WIDTH;
pub const HEIGHT: usize = ROWS * CHARACTER_HEIGHT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleError {
    UninitializedError,
    ConsoleLockError,
    LineNoCapacity,
}

mod line {
    use core::ascii;

    use arrayvec::ArrayVec;

    use crate::error::Result;
    use crate::graphic::console::ConsoleError;

    #[derive(Clone, Debug)]
    pub struct Line<const CAP: usize> {
        buffer: ArrayVec<ascii::Char, CAP>,
        cursor: usize,
    }

    impl<const CAP: usize> Line<CAP> {
        pub const fn new() -> Self {
            Self {
                buffer: ArrayVec::new_const(),
                cursor: 0,
            }
        }

        pub fn is_cursor_overlapping(&self) -> Option<ascii::Char> {
            if self.cursor < self.buffer.len() {
                Some(self.buffer[self.cursor])
            } else {
                None
            }
        }

        pub fn push(&mut self, char: ascii::Char) -> Result<()> {
            if self.cursor >= CAP {
                return Err(ConsoleError::LineNoCapacity.into());
            }

            assert!(self.cursor <= CAP);

            self.buffer.insert(self.cursor, char);

            self.cursor += 1;

            Ok(())
        }

        pub fn push_overflow(&mut self, char: ascii::Char) -> Option<ascii::Char> {
            assert!(self.cursor <= CAP);

            let mut r = None;

            if self.buffer.len() == CAP {
                r.replace(self.buffer.pop().unwrap());
            }

            self.buffer.insert(self.cursor, char);

            self.cursor += 1;

            r
        }

        pub fn move_cursor_left(&mut self) -> Result<()> {
            if self.cursor == 0 {
                return Err(ConsoleError::LineNoCapacity.into());
            }

            self.cursor -= 1;

            Ok(())
        }

        pub fn move_cursor_right(&mut self) -> Result<()> {
            if self.cursor == self.buffer.len() {
                return Err(ConsoleError::LineNoCapacity.into());
            }

            assert!(self.cursor < CAP);

            self.cursor += 1;

            Ok(())
        }

        pub fn shift_left(&mut self) -> Option<ascii::Char> {
            assert!(self.cursor == 0);

            if self.buffer.len() == 0 {
                return None;
            }

            Some(self.buffer.remove(0))
        }

        pub fn shift_right(&mut self, first_char: ascii::Char) -> Option<ascii::Char> {
            assert!(self.cursor == 0);

            let mut r = None;

            if self.buffer.len() == CAP {
                r.replace(self.buffer.pop().unwrap());
            }

            self.buffer.insert(0, first_char);

            r
        }

        pub fn remove(&mut self) -> Result<()> {
            if self.cursor == 0 {
                return Err(ConsoleError::LineNoCapacity.into());
            }

            self.cursor -= 1;

            self.buffer.remove(self.cursor);

            Ok(())
        }

        pub fn get_cursor(&self) -> Result<usize> {
            if self.cursor >= CAP {
                return Err(ConsoleError::LineNoCapacity.into());
            }
            Ok(self.cursor)
        }

        pub fn get_length(&self) -> usize {
            self.buffer.len()
        }

        pub fn get_chars(&self) -> &[ascii::Char] {
            &self.buffer
        }

        pub fn clear(&mut self) {
            self.buffer.clear();
            self.cursor = 0;
        }
    }
}

static CONSOLE: Once<Locked<Console>> = Once::new();

pub struct Console {
    buffer: [Line<COLUMNS>; ROWS],
    bg_color: RgbColor,
    fg_color: RgbColor,
    cursor_row: usize,
    canvas: MaybeUninit<Arc<Mutex<Canvas>>>,
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.print(s);
        Ok(())
    }
}

impl Console {
    fn init(canvas: Arc<Mutex<Canvas>>, bg_color: RgbColor, fg_color: RgbColor) -> Result<Self> {
        canvas
            .lock()
            .fill_rect(u64vec2(0, 0), WIDTH as u64, HEIGHT as u64, bg_color)?;

        Ok(Self {
            buffer: core::array::from_fn(|_| Line::new()),
            bg_color,
            fg_color,
            cursor_row: 0,
            canvas: MaybeUninit::new(canvas),
        })
    }

    fn new_line(&mut self) {
        if self.cursor_row < ROWS - 1 {
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

            let bg_color = self.bg_color;
            self.get_writer()
                .lock()
                .fill_rect(
                    u64vec2(0, (CHARACTER_HEIGHT * (ROWS - 1)) as u64),
                    WIDTH as u64,
                    CHARACTER_HEIGHT as u64,
                    bg_color,
                )
                .unwrap();

            self.buffer
                .last_mut()
                .expect("console buffer is empty")
                .clear();
        }
    }

    fn fill_row(&self, row: usize) {
        let bg_color = self.bg_color;
        self.get_writer()
            .lock()
            .fill_rect(
                u64vec2(0, (row * CHARACTER_HEIGHT) as u64),
                (CHARACTER_WIDTH * COLUMNS) as u64,
                CHARACTER_HEIGHT as u64,
                bg_color,
            )
            .unwrap();
    }

    fn render_row(&self, row: usize) {
        self.fill_row(row);

        let fg_color = self.fg_color;

        for (i, c) in self.buffer[row].get_chars().iter().enumerate() {
            self.get_writer()
                .lock()
                .write_char(
                    u64vec2(
                        (i * CHARACTER_WIDTH) as u64,
                        (row * CHARACTER_HEIGHT) as u64,
                    ),
                    *c,
                    fg_color,
                )
                .unwrap();
        }
    }

    fn backspace(&mut self) {
        if self.buffer[self.cursor_row].remove().is_err() && self.cursor_row != 0 {
            self.cursor_row -= 1;
            self.buffer[self.cursor_row].remove().unwrap();
        }

        let mut shift_row = self.cursor_row + 1;

        if shift_row < ROWS {
            while let Some(c) = self.buffer[shift_row].shift_left() {
                self.buffer[shift_row - 1].push(c).unwrap();

                if self.buffer[shift_row].get_length() < COLUMNS {
                    break;
                }

                shift_row += 1;
            }
        } else {
            shift_row = self.cursor_row;
        }

        if let Ok(cc) = self.buffer[self.cursor_row].get_cursor() {
            cc
        } else {
            self.new_line();
            0
        };

        for row in self.cursor_row..=shift_row {
            self.render_row(row);
        }
    }

    fn get_writer(&self) -> &Arc<Mutex<Canvas>> {
        (unsafe { &*self.canvas.as_ptr() }) as _
    }

    fn print_char(&mut self, c: Char) {
        let cursor_column = if let Ok(cc) = self.buffer[self.cursor_row].get_cursor() {
            cc
        } else {
            self.new_line();
            0
        };

        let mut shift_row = self.cursor_row + 1;
        let mut reached = false;
        let mut popped: ascii::Char;
        if let Some(popped_) = self.buffer[self.cursor_row].push_overflow(c) {
            popped = popped_;

            loop {
                if shift_row >= ROWS {
                    reached = true;
                    break;
                }

                if let Some(p) = self.buffer[shift_row].shift_right(popped) {
                    popped = p;
                    shift_row += 1;
                } else {
                    break;
                }
            }
        } else {
            shift_row -= 1;
            // これ以降poppedが使われるのはreachedが立っているときだけなので適当な値で初期化して良い
            popped = ascii::Char::Null;
        }

        for row in self.cursor_row..=shift_row.min(ROWS) {
            self.render_row(row);
        }

        if reached {
            self.new_line();
            self.buffer.last_mut().unwrap().push(popped).unwrap();
        }
    }

    fn print_cursor(&mut self) {
        let cursor_column = if let Ok(cc) = self.buffer[self.cursor_row].get_cursor() {
            cc
        } else {
            self.new_line();
            0
        };
        let cursor_row = self.cursor_row;
        let fg_color = self.fg_color;
        let bg_color = self.bg_color;

        self.get_writer()
            .lock()
            .fill_rect(
                u64vec2(
                    (cursor_column * CHARACTER_WIDTH) as u64,
                    (cursor_row * CHARACTER_HEIGHT) as u64,
                ),
                CHARACTER_WIDTH as u64,
                CHARACTER_HEIGHT as u64,
                fg_color,
            )
            .unwrap();
        if let Some(c) = self.buffer[self.cursor_row].is_cursor_overlapping() {
            self.get_writer()
                .lock()
                .write_char(
                    u64vec2(
                        (cursor_column * CHARACTER_WIDTH) as u64,
                        (cursor_row * CHARACTER_HEIGHT) as u64,
                    ),
                    c,
                    bg_color,
                )
                .unwrap();
        }
    }

    fn erase_cursor(&mut self) {
        let cursor_column = if let Ok(cc) = self.buffer[self.cursor_row].get_cursor() {
            cc
        } else {
            self.new_line();
            0
        };
        let cursor_row = self.cursor_row;
        let bg_color = self.bg_color;
        let fg_color = self.fg_color;

        self.get_writer()
            .lock()
            .fill_rect(
                u64vec2(
                    (cursor_column * CHARACTER_WIDTH) as u64,
                    (cursor_row * CHARACTER_HEIGHT) as u64,
                ),
                CHARACTER_WIDTH as u64,
                CHARACTER_HEIGHT as u64,
                bg_color,
            )
            .unwrap();

        if let Some(c) = self.buffer[self.cursor_row].is_cursor_overlapping() {
            self.get_writer()
                .lock()
                .write_char(
                    u64vec2(
                        (cursor_column * CHARACTER_WIDTH) as u64,
                        (cursor_row * CHARACTER_HEIGHT) as u64,
                    ),
                    c,
                    fg_color,
                )
                .unwrap();
        }
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

    fn move_cursor_left(&mut self) {
        self.erase_cursor();

        if self.buffer[self.cursor_row].move_cursor_left().is_err() && self.cursor_row != 0 {
            self.cursor_row -= 1;
            self.buffer[self.cursor_row].move_cursor_left().unwrap();
        }

        self.print_cursor();
    }

    fn move_cursor_right(&mut self) {
        self.erase_cursor();

        if self.buffer[self.cursor_row].move_cursor_right().is_err() && self.cursor_row != ROWS - 1
        {
            self.cursor_row += 1;
        }

        self.print_cursor();
    }

    fn clear(&mut self) {
        for line in self.buffer.as_mut() {
            line.clear();
        }

        self.cursor_row = 0;

        let mut writer = self.get_writer().lock();

        writer
            .fill_rect(
                u64vec2(0, 0),
                (CHARACTER_WIDTH * COLUMNS) as u64,
                (CHARACTER_HEIGHT * ROWS) as u64,
                self.bg_color,
            )
            .unwrap();
    }

    fn clear_current_line(&mut self) {
        self.buffer[self.cursor_row].clear();

        let mut writer = self.get_writer().lock();

        let cursor_row = self.cursor_row;

        writer
            .fill_rect(
                u64vec2(0, (CHARACTER_HEIGHT * cursor_row) as u64),
                (CHARACTER_WIDTH * COLUMNS) as u64,
                CHARACTER_HEIGHT as u64,
                self.bg_color,
            )
            .unwrap();
    }
}

pub fn init(canvas: Arc<Mutex<Canvas>>, bg_color: RgbColor, fg_color: RgbColor) -> Result<()> {
    let console = Console::init(canvas, bg_color, fg_color)?;
    CONSOLE.call_once(|| Locked::new(console));
    Ok(())
}

pub fn is_initialized() -> bool {
    CONSOLE.is_completed()
}

fn get_locked_console<'a>() -> &'a Locked<Console> {
    let console = unsafe { CONSOLE.get_unchecked() };

    #[cfg(feature = "init_check")]
    let console = CONSOLE.get().expect("Console is not initialized.");

    console
}

pub fn move_cursor_left() {
    without_interrupts(|| {
        get_locked_console().lock().move_cursor_left();
    });
}

pub fn move_cursor_right() {
    without_interrupts(|| {
        get_locked_console().lock().move_cursor_right();
    })
}

pub fn clear() {
    without_interrupts(|| {
        get_locked_console().lock().clear();
    })
}

pub fn clear_current_line() {
    without_interrupts(|| {
        get_locked_console().lock().clear_current_line();
    });
}

pub fn _print(args: ::core::fmt::Arguments) {
    without_interrupts(|| {
        use core::fmt::Write;

        get_locked_console()
            .lock()
            .write_fmt(args)
            .expect("Failed to print string to console.");
    });
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
