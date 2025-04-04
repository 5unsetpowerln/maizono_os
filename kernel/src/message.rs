use common::arrayqueue::{ArrayQueue, LockLessArrayQueue};
use spin::Mutex;

use crate::device::ps2;
use crate::{kprintln, mouse};

pub enum Message {
    PS2MouseInterrupt,
    PS2KeyboardInterrupt,
}

static QUEUE: Mutex<ArrayQueue<Message, 128>> = Mutex::new(ArrayQueue::new());

pub fn handle_message() {
    x86_64::instructions::interrupts::disable();
    if let Some(message) = QUEUE.lock().dequeue() {
        match message {
            Message::PS2KeyboardInterrupt => {
                kprintln!("{:?}", unsafe { ps2::keyboard().lock().read_data() });
            }
            Message::PS2MouseInterrupt => {
                let _ = unsafe {
                    ps2::mouse().lock().receive_events(|event| match event {
                        mouse::MouseEvent::Move { displacement } => {
                            mouse::move_relative(displacement);
                        }
                        _ => {}
                    })
                };
            }
        }
    }
    x86_64::instructions::interrupts::enable();
}

pub fn enqueue(message: Message) {
    x86_64::instructions::interrupts::disable();
    let mut queue = QUEUE.lock();
    queue.enqueue(message);
    x86_64::instructions::interrupts::enable();
}

pub fn count() -> usize {
    x86_64::instructions::interrupts::disable();
    let queue = QUEUE.lock();
    let count = queue.count();
    x86_64::instructions::interrupts::enable();
    count
}
