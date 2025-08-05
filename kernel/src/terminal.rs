use core::ascii::{self, Char};

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use core::arch::asm;
use log::debug;
use log::info;
use pc_keyboard::{DecodedKey, KeyCode};
use spin::mutex::Mutex;
use spin::once::Once;
use x86_64::instructions::interrupts::without_interrupts;

use crate::LAYER_IDS;
use crate::TASK_IDS;
use crate::graphic::console;
use crate::kprint;
use crate::message;
use crate::serial_println;
use crate::task::TASK_MANAGER;
use crate::task::TaskManagerTrait;

pub struct Terminal {
    line_buffer: String,
    cursor: usize,
    display_line_buffer: String,
    displayed_count: usize,
}

impl Terminal {
    pub fn new() -> Self {
        Self {
            line_buffer: String::new(),
            cursor: 0,
            display_line_buffer: String::new(),
            displayed_count: 0,
        }
    }

    pub fn input_key(&mut self, key: DecodedKey) {
        match key {
            DecodedKey::Unicode(character) => {
                // 表示用バッファには文字を区別せずにそのまま入力していく。Consoleがバックスペースなどを処理してくれるから
                // self.display_line_buffer.push(character);

                match character.as_ascii().unwrap() {
                    ascii::Char::Backspace => {
                        if self.cursor > 0 {
                            self.cursor -= 1;
                            self.line_buffer.remove(self.cursor);
                            self.display_line_buffer.push(character);
                        }
                    }
                    ascii::Char::LineFeed => {}
                    _ => {
                        self.line_buffer.insert(self.cursor, character);
                        self.cursor += 1;
                        self.display_line_buffer.push(character);
                    }
                }
            }
            DecodedKey::RawKey(key) => match key {
                KeyCode::ArrowLeft => {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                        console::move_cursor_left();
                    }
                }
                KeyCode::ArrowRight => {
                    if self.cursor < self.line_buffer.chars().count() {
                        self.cursor += 1;
                        console::move_cursor_right();
                    }
                }
                _ => {}
            },
        }
    }

    pub fn display_on_console(&mut self) {
        if self.displayed_count < self.display_line_buffer.chars().count() {
            let diff = self.display_line_buffer[self.displayed_count..].to_string();
            kprint!("{}", diff);
            serial_println!("{:?}", diff);
            serial_println!("{}", self.displayed_count);
            self.displayed_count += diff.chars().count();
        }
    }
}

static TERMINAL: Once<Mutex<Terminal>> = Once::new();

pub fn init() {
    TERMINAL.call_once(|| Mutex::new(Terminal::new()));
}

pub fn terminal_task(task_id: u64, _data: u64) {
    let draw_layer_task_id = TASK_IDS.wait().draw_layer_task_id;

    loop {
        if let Some(message::Message::KeyInput(decoded_key)) = without_interrupts(|| {
            TASK_MANAGER
                .wait()
                .lock()
                .receive_message_from_task(task_id)
                .unwrap()
        }) {
            let mut terminal = TERMINAL.wait().lock();
            terminal.input_key(decoded_key);
            serial_println!("line_buffer   : {:?}", terminal.line_buffer);
            terminal.display_on_console();
        } else {
            without_interrupts(|| {
                TASK_MANAGER
                    .wait()
                    .lock()
                    .send_message_to_task(draw_layer_task_id, &message::Message::DrawLayer)
                    .unwrap();
            });

            TASK_MANAGER.wait().sleep(task_id).unwrap();
            continue;
        }

        unsafe { asm!("hlt") }
    }
}
