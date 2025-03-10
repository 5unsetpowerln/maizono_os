use common::arrayqueue::{ArrayQueue, LockLessArrayQueue};
use spin::Mutex;

use crate::{kprintln, ps2};

pub enum Message {
    PS2MouseInterrupt,
    PS2KeyboardInterrupt,
}

//pub static QUEUE: Mutex<ArrayQueue<Message, 128>> = Mutex::new(ArrayQueue::new());
pub static QUEUE: LockLessArrayQueue<Message, 128> = LockLessArrayQueue::new();

pub fn handle_message() {
    x86_64::instructions::interrupts::disable();

    if let Some(message) = unsafe { QUEUE.dequeue() } {
        match message {
            Message::PS2KeyboardInterrupt => {
                kprintln!("{:?}", unsafe { ps2::keyboard().lock().read_data() });
            }
            Message::PS2MouseInterrupt => {
                let a = unsafe { ps2::mouse().lock().receive_events(|data0, data1, data2| {}) };
            }
        }
    }

    x86_64::instructions::interrupts::enable();
}
