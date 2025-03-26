pub mod bump_allocator;
pub mod linked_list_allocator;

use alloc::{boxed::Box, vec::Vec};
use core::{alloc::GlobalAlloc, ptr::null_mut};
use linked_list_allocator::LinkedListAllocator;

use bump_allocator::BumpAllocator;

use crate::{frame_manager, kprintln};

pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Self {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}

/// Align the given address `addr` upwards to alignment `align`.
///
/// Requires that `align` is power of two.
pub fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

const HEAP_FRAME_COUNT: usize = 64 * 512;

#[global_allocator]
static ALLOCATOR: Locked<LinkedListAllocator> = Locked::new(LinkedListAllocator::new());

pub fn init() {
    let heap_frame_head =
        frame_manager::alloc(HEAP_FRAME_COUNT).expect("failed to allocate frames for heap");
    let heap_start = heap_frame_head.to_bytes();
    let heap_size = HEAP_FRAME_COUNT * frame_manager::BYTES_PER_FRAME;

    unsafe { ALLOCATOR.lock().init(heap_start, heap_size) };
}

#[test_case]
fn large_vec() {
    let n = 1000;
    let mut vec = Vec::new();
    for i in 0..n {
        vec.push(i);
    }
    assert_eq!(vec.iter().sum::<u64>(), (n - 1) * n / 2);
}

#[test_case]
fn simple_allocation() {
    let heap_value_1 = Box::new(41);
    let heap_value_2 = Box::new(13);
    assert_eq!(*heap_value_1, 41);
    assert_eq!(*heap_value_2, 13);
}

#[test_case]
fn many_boxes_long_lived() {
    let long_lived = Box::new(1);
    for i in 0..HEAP_FRAME_COUNT * frame_manager::BYTES_PER_FRAME {
        let x = Box::new(i);
        assert_eq!(*x, i);
    }
    assert_eq!(*long_lived, 1);
}
