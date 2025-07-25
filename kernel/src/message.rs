use spin::Mutex;

use crate::{device::ps2::keyboard::KeyboardError, timer::Timer, types::Queue};

#[derive(Debug)]
pub enum Message {
    PS2MouseInterrupt,
    PS2KeyboardInterrupt(Result<u8, KeyboardError>),
    LocalAPICTimerInterrupt,
    TimerTimeout(Timer),
}

pub static QUEUE: Mutex<Queue<Message>> = Mutex::new(Queue::new());

pub fn enqueue(message: Message) {
    let mut queue = QUEUE.lock();
    queue.enqueue(message);
}

pub fn count() -> usize {
    x86_64::instructions::interrupts::disable();
    let queue = QUEUE.lock();
    let count = queue.len();
    x86_64::instructions::interrupts::enable();
    count
}
