use alloc::collections::VecDeque;
use spin::Mutex;

use crate::{device::ps2::keyboard::KeyboardError, timer::Timer};

#[derive(Debug, Clone, Copy)]
pub enum Message {
    PS2MouseInterrupt,
    PS2KeyboardInterrupt(Result<u8, KeyboardError>),
    LocalAPICTimerInterrupt,
    TimerTimeout(Timer),
}
