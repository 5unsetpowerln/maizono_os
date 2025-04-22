use spin::Mutex;

use crate::device::ps2;
use crate::types::Queue;
use crate::{kprintln, mouse};

pub enum Message {
    PS2MouseInterrupt,
    PS2KeyboardInterrupt,
    LocalAPICTimerInterrupt,
}

// static QUEUE: Mutex<ArrayQueue<Message, 128>> = Mutex::new(ArrayQueue::new());
static QUEUE: Mutex<Queue<Message>> = Mutex::new(Queue::new());

pub fn handle_message() {
    x86_64::instructions::interrupts::disable();
    if let Some(message) = QUEUE.lock().dequeue() {
        match message {
            Message::PS2KeyboardInterrupt => {
                // must receive data to prevent the block
                let data = unsafe { ps2::keyboard().lock().read_data() };
                kprintln!("{:?}", data);
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
            Message::LocalAPICTimerInterrupt => {
                kprintln!("local apic timer interrupt occured!");
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
    let count = queue.len();
    x86_64::instructions::interrupts::enable();
    count
}
