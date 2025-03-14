use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicUsize, Ordering},
};

pub struct ArrayQueue<T, const CAP: usize> {
    array: [Option<T>; CAP],
    write_pos: usize,
    read_pos: usize,
}

impl<T, const CAP: usize> ArrayQueue<T, CAP> {
    pub const fn new() -> Self {
        Self {
            array: [const { None }; CAP],
            write_pos: 0,
            read_pos: 0,
        }
    }

    pub fn enqueue(&mut self, element: T) {
        let next_write_pos = (self.write_pos + 1) % CAP;

        if next_write_pos != self.read_pos {
            self.array[self.write_pos] = Some(element);
            self.write_pos = next_write_pos;
        }
    }

    pub fn dequeue(&mut self) -> Option<T> {
        if self.read_pos == self.write_pos {
            return None;
        }

        let element = self.array[self.read_pos].take();
        self.read_pos = (self.read_pos + 1) % CAP;
        element
    }

    pub fn count(&self) -> usize {
        self.write_pos - self.read_pos
    }
}

impl<T, const CAP: usize> Default for ArrayQueue<T, CAP> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LockLessArrayQueue<T, const CAP: usize> {
    array: UnsafeCell<[Option<T>; CAP]>,
    write_pos: AtomicUsize,
    read_pos: AtomicUsize,
}

unsafe impl<T, const CAP: usize> Sync for LockLessArrayQueue<T, CAP> {}

impl<T, const CAP: usize> LockLessArrayQueue<T, CAP> {
    pub const fn new() -> Self {
        Self {
            array: UnsafeCell::new([const { None }; CAP]),
            write_pos: AtomicUsize::new(0),
            read_pos: AtomicUsize::new(0),
        }
    }

    pub unsafe fn enqueue(&self, element: T) {
        let write_pos = self.write_pos.load(Ordering::Relaxed);
        let next_write_pos = (write_pos + 1) % CAP;

        if next_write_pos != self.read_pos.load(Ordering::Acquire) {
            unsafe {
                (*self.array.get())[write_pos] = Some(element);
            }
            self.write_pos.store(next_write_pos, Ordering::Release);
        }
    }

    pub unsafe fn dequeue(&self) -> Option<T> {
        let read_pos = self.read_pos.load(Ordering::Relaxed);
        if read_pos == self.write_pos.load(Ordering::Acquire) {
            return None;
        }

        let element = unsafe { (*self.array.get())[read_pos].take() };
        self.read_pos.store((read_pos + 1) % CAP, Ordering::Release);
        element
    }
}

impl<T, const CAP: usize> Default for LockLessArrayQueue<T, CAP> {
    fn default() -> Self {
        Self::new()
    }
}

//pub trait ConstDefault {
//const fn const_default();
//}
