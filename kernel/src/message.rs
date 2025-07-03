use spin::Mutex;

use crate::{timer::Timer, types::Queue};

pub enum Message {
    PS2MouseInterrupt,
    PS2KeyboardInterrupt,
    LocalAPICTimerInterrupt,
    TimerTimeout(Timer),
}

pub static QUEUE: Mutex<Queue<Message>> = Mutex::new(Queue::new());

pub fn enqueue(message: Message) {
    x86_64::instructions::interrupts::disable();
    let mut queue = QUEUE.lock();
    queue.enqueue(message);
    x86_64::instructions::interrupts::enable();
}

pub fn count() -> usize {
    x86_64::instructions::interrupts::disable();
    let queue = QUEUE.lock();
    let count = queue.len();
    x86_64::instructions::interrupts::enable();
    count
}
