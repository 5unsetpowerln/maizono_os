use core::cell::{Cell, UnsafeCell};
use core::hint::spin_loop;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

use log::debug;
use thiserror_no_std::Error;
use x86_64::instructions::interrupts;

use crate::cpu::{self, apic_id_to_idx, get_apic_count_max, get_local_apic_id};
use crate::error::Result;
use crate::serial_emergency_println;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MutexError {
    #[error("Unsupported pixel format.")]
    LockError,
}

struct PerApicLockCounter {
    array: UnsafeCell<[usize; get_apic_count_max()]>,
}

unsafe impl Sync for PerApicLockCounter {}

impl PerApicLockCounter {
    const fn new() -> Self {
        Self {
            array: UnsafeCell::new([0; get_apic_count_max()]),
        }
    }

    unsafe fn increment(&self, apic_id: u8) {
        let array = unsafe { &mut *self.array.get() };
        let idx = cpu::apic_id_to_idx(apic_id);

        debug_assert!(array.len() > idx);

        array[idx] += 1;
    }

    /// per-cpuロックカウントを1減らす。
    /// すでに0の場合は何もしない。
    unsafe fn decrement(&self, apic_id: u8) {
        let array = unsafe { &mut *self.array.get() };
        let idx = cpu::apic_id_to_idx(apic_id);

        debug_assert!(array.len() > idx);

        array[idx] -= 1;
    }

    unsafe fn get_by_idx(&self, idx: usize) -> usize {
        let array = unsafe { &mut *self.array.get() };

        debug_assert!(array.len() > idx);

        array[idx]
    }

    unsafe fn get(&self, apic_id: u8) -> usize {
        let array = unsafe { &mut *self.array.get() };
        let idx = cpu::apic_id_to_idx(apic_id);

        debug_assert!(array.len() > idx);

        array[idx]
    }
}

struct PerApicInitialInterruptState {
    array: UnsafeCell<[bool; get_apic_count_max()]>,
}

unsafe impl Sync for PerApicInitialInterruptState {}

impl PerApicInitialInterruptState {
    const fn new() -> Self {
        Self {
            array: UnsafeCell::new([true; get_apic_count_max()]),
        }
    }

    unsafe fn set(&self, apic_id: u8, state: bool) {
        let idx = apic_id_to_idx(apic_id);
        let array = unsafe { &mut *self.array.get() };

        debug_assert!(array.len() > idx);

        array[idx] = state;
    }

    unsafe fn get(&self, apic_id: u8) -> bool {
        let idx = apic_id_to_idx(apic_id);
        let array = unsafe { &mut *self.array.get() };

        debug_assert!(array.len() > idx);

        array[idx]
    }
}

static PER_APIC_LOCK_COUNTER: PerApicLockCounter = PerApicLockCounter::new();
static PER_LAPIC_INIT_INTERRUPT_STATE: PerApicInitialInterruptState =
    PerApicInitialInterruptState::new();

pub struct Mutex<T: ?Sized> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

pub struct MutexGuard<'a, T> {
    _not_send: PhantomData<Cell<()>>,
    m: &'a Mutex<T>,
}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        let interrupts_enabled = interrupts::are_enabled();

        if interrupts_enabled {
            interrupts::disable();
        }

        let current_local_apic_id = get_local_apic_id();

        let current_lock_count = unsafe { PER_APIC_LOCK_COUNTER.get(current_local_apic_id) };

        if current_lock_count == 0 {
            unsafe {
                PER_LAPIC_INIT_INTERRUPT_STATE.set(current_local_apic_id, interrupts_enabled);
            }
        }

        // per-cpuロックカウンタをインクリメントする
        unsafe {
            PER_APIC_LOCK_COUNTER.increment(current_local_apic_id);
        }

        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            spin_loop();
        }

        MutexGuard {
            m: self,
            _not_send: PhantomData,
        }
    }

    pub fn try_lock(&self) -> Result<MutexGuard<'_, T>> {
        todo!()
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.m.data.get() }
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.m.data.get() }
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        // Local APICの割り込みを無効化する
        if interrupts::are_enabled() {
            interrupts::disable();
        }

        let current_local_apic_id = get_local_apic_id();

        // per-cpuロックカウンタをデクリメントする
        unsafe {
            PER_APIC_LOCK_COUNTER.decrement(current_local_apic_id);
        }

        // ロックを解除する
        self.m.locked.store(false, Ordering::Release);

        // per-cpuロックカウンタが0になったら割り込み状態を初期状態に戻す
        let count = unsafe { PER_APIC_LOCK_COUNTER.get(current_local_apic_id) };
        if count == 0 && unsafe { PER_LAPIC_INIT_INTERRUPT_STATE.get(current_local_apic_id) } {
            interrupts::enable();
        }
    }
}

unsafe impl<T: Send> Sync for Mutex<T> {}
unsafe impl<T: Send> Send for Mutex<T> {}
