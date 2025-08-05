use alloc::collections::VecDeque;
use glam::{I64Vec2, U64Vec2};
use pc_keyboard::DecodedKey;
use spin::Mutex;

use crate::{device::ps2::keyboard::KeyboardError, timer::Timer};

#[derive(Debug, Clone, Copy)]
pub enum Message {
    PS2MouseInterrupt,
    PS2KeyboardInterrupt(Result<u8, KeyboardError>),
    LocalAPICTimerInterrupt,
    TimerTimeout(Timer),
    DrawLayer,
    // Layer(LayerOperation),
    // LayerFinish,
    KeyInput(DecodedKey),
}

// #[derive(Debug, Clone, Copy)]
// pub struct KeyInput {
//     decoded_key: DecodedKey,
//     src_task_id: u64,
// }

// impl KeyInput {
//     pub fn new(decoded_key: DecodedKey, src_task_id: u64) -> Self {
//         Self {
//             decoded_key,
//             src_task_id,
//         }
//     }
// }

#[derive(Debug, Clone, Copy)]
pub struct LayerOperation {
    pub kind: LayerOperationKind,
    pub layer_id: usize,
    pub src_task_id: u64,
}

impl LayerOperation {
    pub fn new(kind: LayerOperationKind, layer_id: usize, src_task_id: u64) -> Self {
        Self {
            kind,
            layer_id,
            src_task_id,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LayerOperationKind {
    MoveAbsolute(U64Vec2),
    MoveRelative(I64Vec2),
    Draw,
}
