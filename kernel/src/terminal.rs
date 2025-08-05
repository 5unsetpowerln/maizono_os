use core::ascii::{self, Char};

use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::arch::asm;
use pc_keyboard::{DecodedKey, KeyCode};
use spin::mutex::Mutex;
use spin::once::Once;
use x86_64::instructions::interrupts::without_interrupts;

use crate::TASK_IDS;
use crate::graphic::console;
use crate::kprint;
use crate::kprintln;
use crate::logger;
use crate::message;
use crate::task::TASK_MANAGER;
use crate::task::TaskManagerTrait;

const HISTORY_SIZE: usize = 8;
const PROMPT: &str = "$ ";

pub struct Terminal {
    line_buffer: String,
    cursor: usize,
    display_line_buffer: String,
    displayed_count: usize,
    history: VecDeque<String>,
    history_idx: isize,
}

impl Terminal {
    pub fn new() -> Self {
        let mut history = VecDeque::new();
        history.resize(HISTORY_SIZE, String::new());

        Self {
            line_buffer: String::new(),
            cursor: 0,
            display_line_buffer: "$ ".to_string(),
            displayed_count: 0,
            history,
            history_idx: -1,
        }
    }

    pub fn input_key(&mut self, key: DecodedKey) {
        match key {
            DecodedKey::Unicode(character) => match character.as_ascii().unwrap() {
                ascii::Char::Backspace => {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                        self.line_buffer.remove(self.cursor);
                        self.display_line_buffer.push(character);
                    }
                }
                ascii::Char::LineFeed => {
                    self.execute_line();

                    self.history.pop_back();
                    self.history.push_front(self.line_buffer.clone());
                    self.history_idx = -1;

                    self.reset_input();
                }
                _ => {
                    self.line_buffer.insert(self.cursor, character);
                    self.cursor += 1;
                    self.display_line_buffer.push(character);
                }
            },
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
                KeyCode::ArrowUp => {
                    self.history_up_or_down(1);
                }
                KeyCode::ArrowDown => {
                    self.history_up_or_down(-1);
                }
                _ => {}
            },
        }
    }

    pub fn history_up_or_down(&mut self, direction: i8) {
        let prev_history_idx = self.history_idx;

        if direction > 0
            && 0 <= self.history_idx + 1
            && self.history_idx + 1 < (HISTORY_SIZE as isize)
        {
            self.history_idx += 1;
        }

        if direction < 0 && 0 <= self.history_idx {
            self.history_idx -= 1;
        }

        if self.history_idx == -1 {
            console::clear_current_line();
            self.reset_input();
            return;
        }

        if let Some(history) = self.history.get(self.history_idx as usize) {
            let history = history.clone();

            if history.is_empty() {
                self.history_idx = prev_history_idx;
                return;
            }

            console::clear_current_line();
            self.line_buffer = history;
            self.display_line_buffer = format!("$ {}", self.line_buffer);
            self.cursor = self.line_buffer.chars().count();
            self.displayed_count = 0;
        }
    }

    pub fn display_on_console(&mut self) {
        if self.displayed_count < self.display_line_buffer.chars().count() {
            let diff = self.display_line_buffer[self.displayed_count..].to_string();
            kprint!("{}", diff);
            self.displayed_count += diff.chars().count();
        }
    }

    pub fn reset_input(&mut self) {
        self.cursor = 0;
        self.display_line_buffer = "$ ".to_string();
        self.line_buffer.clear();
        self.displayed_count = 0;
    }

    // コマンドが認識されたら true, コマンドが認識されなかったら (例えば空白だけ) false
    pub fn execute_line(&mut self) -> bool {
        let parts = self.line_buffer.as_str().split(' ').collect::<Vec<&str>>();

        let command = parts.iter().find(|&s| !s.is_empty());

        if command.is_none() {
            kprintln!("");
            kprintln!("");
            return false;
        }

        let command = *command.unwrap();

        match command {
            "echo" => {
                let args_index = self.line_buffer.as_str().find(command).unwrap();
                let args = self
                    .line_buffer
                    .as_str()
                    .get(args_index + command.len()..)
                    .unwrap_or("")
                    .trim();

                kprintln!("");
                kprintln!("{args}");
                kprintln!("");
            }
            "clear" => {
                console::clear();
            }
            _ => {
                kprintln!("");
                kprintln!("No such command: {command}");
                kprintln!("");
            }
        }

        true
    }
}

static TERMINAL: Once<Mutex<Terminal>> = Once::new();

pub fn init() {
    TERMINAL.call_once(|| Mutex::new(Terminal::new()));
}

pub fn terminal_task(task_id: u64, _data: u64) {
    without_interrupts(|| {
        *logger::CONSOLE_ENABLED.write() = false;
    });

    let draw_layer_task_id = TASK_IDS.wait().draw_layer_task_id;
    TERMINAL.wait().lock().display_on_console();

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
