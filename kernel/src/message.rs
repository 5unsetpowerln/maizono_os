use alloc::collections::VecDeque;
use spin::Mutex;

use crate::{device::ps2::keyboard::KeyboardError, timer::Timer};

#[derive(Debug)]
pub enum Message {
    PS2MouseInterrupt,
    PS2KeyboardInterrupt(Result<u8, KeyboardError>),
    LocalAPICTimerInterrupt,
    TimerTimeout(Timer),
}

pub static QUEUE: Mutex<VecDeque<Message>> = Mutex::new(VecDeque::new());

pub fn count() -> usize {
    x86_64::instructions::interrupts::disable();
    let queue = QUEUE.lock();
    let count = queue.len();
    x86_64::instructions::interrupts::enable();
    count
}
