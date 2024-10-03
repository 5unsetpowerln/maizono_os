// use linked_list_allocator::LockedHeap;

use common::address::AlignedAddress64;
use spin::{Lazy, Mutex, MutexGuard};
use thiserror_no_std::Error;

use super::error::{UsbError, UsbResult};

#[derive(Debug, Clone, PartialEq, Error)]
pub enum MemoryError {
    #[error("No available memory space.")]
    FullMemoryError,
    #[error("Failed to lock the allocator.")]
    AllocatorLockError,
}

const MEMORY_POOL_SIZE: usize = 4096 * 32;
static mut MEMORY_POOL: MemoryPool = MemoryPool([0; MEMORY_POOL_SIZE]);
static mut ALLOCATOR: Lazy<Mutex<Allocator>> =
    Lazy::new(|| Mutex::new(unsafe { Allocator::new() }));

#[repr(align(64))]
#[derive(Debug)]
struct MemoryPool([u8; MEMORY_POOL_SIZE]);

#[repr(align(64))]
#[derive(Debug)]
struct Allocator {
    alloc_point_ptr: u64,
    end_ptr: u64,
}

impl Allocator {
    pub unsafe fn new() -> Self {
        let head_ptr = unsafe { MEMORY_POOL.0.as_ptr() as u64 };
        Self {
            alloc_point_ptr: head_ptr,
            end_ptr: head_ptr + MEMORY_POOL_SIZE as u64,
        }
    }

    // unsafe fn alloc_trb_ring(&mut self, ring_size: usize) -> UsbError<AlignedAddress64> {
    // unsafe {self.alloc_array(size_of::<u128>() *)}
    // }

    /// Allocates memory for array of T with zero-initialization.
    unsafe fn alloc_array<T>(
        &mut self,
        number_of_object: usize,
        alignment: usize,
        boudary: usize,
    ) -> UsbResult<AlignedAddress64> {
        unsafe { self.alloc_memory(size_of::<T>() * number_of_object, alignment, boudary) }
    }

    /// Allocates memory with zero-initialization.
    unsafe fn alloc_memory(
        &mut self,
        size: usize,
        alignment: usize,
        boundary: usize,
    ) -> UsbResult<AlignedAddress64> {
        if alignment > 0 {
            self.alloc_point_ptr = ceil(self.alloc_point_ptr, alignment as u64);
        }
        if boundary > 0 {
            let next_boudary = ceil(self.alloc_point_ptr, boundary as u64);
            if (next_boudary as usize) < self.alloc_point_ptr as usize + size {
                self.alloc_point_ptr = next_boudary;
            }
        }

        let memory_pool_ptr = unsafe { MEMORY_POOL.0.as_ptr() as usize };

        if memory_pool_ptr + MEMORY_POOL_SIZE < self.alloc_point_ptr as usize + size {
            return Err(MemoryError::FullMemoryError.into());
        }

        let allocated_addr = self.alloc_point_ptr;
        self.alloc_point_ptr += size as u64;

        for i in 0..size {
            let ptr = (allocated_addr as usize + i) as *mut u8;
            unsafe { *ptr = 0 };
        }

        Ok(AlignedAddress64::new(allocated_addr).unwrap())
    }

    fn free_memory(_ptr: u64) {}
}

fn ceil(ptr: u64, alignment: u64) -> u64 {
    (ptr + alignment - 1) & !(alignment - 1)
}

unsafe fn lock_allocator<'a>() -> UsbResult<MutexGuard<'a, Allocator>> {
    Ok(unsafe { ALLOCATOR.try_lock() }.ok_or(MemoryError::AllocatorLockError)?)
}

/// Allocates memory for array of T with zero-initialization.
pub fn alloc_array<T>(
    size: usize,
    alignment: usize,
    boudary: usize,
) -> UsbResult<AlignedAddress64> {
    unsafe {
        lock_allocator()
            .as_mut()
            .map_err(|err| err.clone())?
            .alloc_array::<T>(size, alignment, boudary)
    }
}
