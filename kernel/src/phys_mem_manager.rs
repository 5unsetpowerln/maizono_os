use core::ops::{Deref, DerefMut};

use spin::{mutex::MutexGuard, Mutex};
use thiserror_no_std::Error;
use uefi::mem::memory_map::{MemoryMap, MemoryMapOwned};

use crate::memory_map::{is_available, UEFI_PAGE_SIZE};

static MEMORY_MANAGER: Mutex<BitmapMemoryManager> = Mutex::new(BitmapMemoryManager::new());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MemoryManagerError {}

const BYTES_PER_FRAME: usize = 4 * 1024;

struct FrameID(usize);
impl FrameID {
    fn get(&self) -> usize {
        self.0
    }
}
impl Deref for FrameID {
    type Target = usize;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for FrameID {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

const FRAME_SIZE: usize = 4 * 1024;

const MAX_PHYSICAL_MEMORY_BYTES: usize = 128 * 1024 * 1024 * 1024; // 128GiB
const FRAME_COUNT: usize = MAX_PHYSICAL_MEMORY_BYTES / FRAME_SIZE;

type MapLine = usize;
const BITS_PER_MAP_LINE: usize = 8 * core::mem::size_of::<MapLine>();
const MAP_LINE_COUNT: usize = FRAME_COUNT / BITS_PER_MAP_LINE;

pub struct BitmapMemoryManager {
    alloc_map: [MapLine; MAP_LINE_COUNT],
    begin: FrameID,
    end: FrameID,
}

impl BitmapMemoryManager {
    const fn new() -> Self {
        Self {
            alloc_map: [0; MAP_LINE_COUNT],
            begin: FrameID(0),
            end: FrameID(FRAME_COUNT),
        }
    }

    pub fn init(&mut self, memory_map: &MemoryMapOwned) {
        let mut last_available_end = 0;
        for desc in memory_map.entries() {
            let phys_start = desc.phys_start as usize;
            let phys_end = phys_start + (desc.page_count as usize) * UEFI_PAGE_SIZE;

            // mark a missing area as an allocated area
            if last_available_end < phys_start as usize {
                let id = FrameID(last_available_end / BYTES_PER_FRAME);
                let count = (phys_start - last_available_end) / BYTES_PER_FRAME;
                self.mark_allocated(id, count);
            }

            // mark an used area as an allocated area
            if is_available(desc.ty) {
                last_available_end = phys_end;
            } else {
                let id = FrameID(phys_start / BYTES_PER_FRAME);
                let count = (desc.page_count as usize * UEFI_PAGE_SIZE) / BYTES_PER_FRAME;
                self.mark_allocated(id, count);
            }
        }

        self.set_memory_range(
            FrameID(1),
            FrameID(last_available_end as usize / BYTES_PER_FRAME),
        );
    }

    fn mark_allocated(&mut self, first_frame_id: FrameID, count: usize) {
        for i in 0..count {
            self.set_bit(FrameID(first_frame_id.get() + i), true);
        }
    }

    fn set_memory_range(&mut self, range_begin: FrameID, range_end: FrameID) {
        self.begin = range_begin;
        self.end = range_end;
    }

    fn set_bit(&mut self, frame_id: FrameID, allocated: bool) {
        let line_index = frame_id.get() / BITS_PER_MAP_LINE;
        let bit_index = frame_id.get() % BITS_PER_MAP_LINE;

        if allocated {
            self.alloc_map[line_index] |= 1 << bit_index;
        } else {
            self.alloc_map[line_index] &= !(1 << bit_index);
        }
    }

    fn get_bit(&self, frame_id: FrameID) -> bool {
        let line_index = frame_id.get() / BITS_PER_MAP_LINE;
        let bit_index = frame_id.get() % BITS_PER_MAP_LINE;

        (self.alloc_map[line_index] & (1 << bit_index)) != 0
    }

    fn alloc(&mut self, number_of_frame: usize) -> Option<FrameID> {
        let mut start_frame_id = self.begin.get();
        loop {
            let i = 0;
            for _ in i..number_of_frame {
                if start_frame_id + i >= self.end.get() {
                    return None;
                }
                if self.get_bit(FrameID(start_frame_id + i)) {
                    break;
                }
            }
            // If there are `number_of_frame`-consecutive free frames, following condition is true.
            if i == number_of_frame {
                self.mark_allocated(FrameID(start_frame_id), number_of_frame);
                return Some(FrameID(start_frame_id));
            }

            start_frame_id += i + 1;
        }
    }

    fn free(&mut self, first_frame_id: FrameID, number_of_frame: usize) {
        for i in 0..number_of_frame {
            self.set_bit(FrameID(first_frame_id.get() + i), false);
        }
    }
}

pub fn mem_manager() -> MutexGuard<'static, BitmapMemoryManager> {
    MEMORY_MANAGER.lock()
}
