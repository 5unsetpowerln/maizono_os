// use linked_list_allocator::LockedHeap;

use common::address::AlignedAddress;

const MEMORY_POOL_SIZE: usize = 4096 * 32;
static MEMORY_POOL: MemoryPool = MemoryPool([0; MEMORY_POOL_SIZE]);

#[repr(align(64))]
#[derive(Debug)]
pub struct MemoryPool([u8; MEMORY_POOL_SIZE]);

#[repr(align(64))]
#[derive(Debug)]
pub struct Allocator {
    alloc_point_ptr: u64,
    end_ptr: u64,
}

impl Allocator {
    pub fn new() -> Self {
        let head_ptr = MEMORY_POOL.0.as_ptr() as u64;
        Self {
            alloc_point_ptr: head_ptr,
            end_ptr: head_ptr + MEMORY_POOL_SIZE as u64,
        }
    }

    fn allocate_memory(
        &mut self,
        size: usize,
        alignment: usize,
        boundary: usize,
    ) -> Option<AlignedAddress<64>> {
        if alignment > 0 {
            self.alloc_point_ptr = ceil(self.alloc_point_ptr, alignment as u64);
        }
        if boundary > 0 {
            let next_boudary = ceil(self.alloc_point_ptr, boundary as u64);
            if (next_boudary as usize) < self.alloc_point_ptr as usize + size {
                self.alloc_point_ptr = next_boudary;
            }
        }

        let memory_pool_ptr = MEMORY_POOL.0.as_ptr() as usize;

        if memory_pool_ptr + MEMORY_POOL_SIZE < self.alloc_point_ptr as usize + size {
            return None;
        }

        let allocated_addr = self.alloc_point_ptr;
        self.alloc_point_ptr += size as u64;
        Some(AlignedAddress::<64>::new(allocated_addr).unwrap())
    }
}

fn ceil(ptr: u64, alignment: u64) -> u64 {
    (ptr + alignment - 1) & !(alignment - 1)
}

// static mut ALLOCATOR: LockedHeap = LockedHeap::empty();

// pub fn init_alloc() {
//     unsafe {
//         ALLOCATOR
//             .lock()
//             .init(MEMORY_POOL.as_mut_ptr(), MEMORY_POOL.len());
//     }
// }
