use core::cell::{Cell, UnsafeCell};
use core::hint::spin_loop;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicBool, Ordering};

use x86_64::instructions::interrupts;

use crate::cpu::{self, apic_id_to_idx, get_apic_count_max, get_local_apic_id};

struct PerApicLockCounter {
    array: [UnsafeCell<usize>; get_apic_count_max()],
}

unsafe impl Sync for PerApicLockCounter {}

impl PerApicLockCounter {
    unsafe fn increment(&self, apic_id: u8) {
        let idx = cpu::apic_id_to_idx(apic_id);

        debug_assert!(self.array.len() > idx);

        unsafe {
            let count_uc = self.array.get_unchecked(idx);
            *count_uc.get() += 1;
        }
    }

    /// per-cpuロックカウントを1減らす。
    /// すでに0の場合は何もしない。
    unsafe fn decrement(&self, apic_id: u8) {
        let idx = cpu::apic_id_to_idx(apic_id);
        debug_assert!(self.array.len() > idx);

        let prev = unsafe { self.get_by_idx(idx) };

        if prev > 0 {
            unsafe {
                let count_uc = self.array.get_unchecked(idx);
                *count_uc.get() -= 1;
            }
        }
    }

    unsafe fn get_by_idx(&self, idx: usize) -> usize {
        debug_assert!(self.array.len() > idx);

        unsafe {
            let count_uc = self.array.get_unchecked(idx);
            *count_uc.get()
        }
    }

    unsafe fn get(&self, apic_id: u8) -> usize {
        let idx = cpu::apic_id_to_idx(apic_id);

        debug_assert!(self.array.len() > idx);

        unsafe {
            let count_uc = self.array.get_unchecked(idx);
            *count_uc.get()
        }
    }
}

struct PerApicInitialInterruptState {
    array: [UnsafeCell<bool>; get_apic_count_max()],
}

unsafe impl Sync for PerApicInitialInterruptState {}

impl PerApicInitialInterruptState {
    unsafe fn set(&self, apic_id: u8, state: bool) {
        let idx = apic_id_to_idx(apic_id);

        debug_assert!(self.array.len() > idx);

        unsafe {
            let state_uc = self.array.get_unchecked(idx);
            *state_uc.get() = state;
        }
    }

    unsafe fn get(&self, apic_id: u8) -> bool {
        let idx = apic_id_to_idx(apic_id);

        debug_assert!(self.array.len() > idx);

        unsafe {
            let state_uc = self.array.get_unchecked(idx);
            *state_uc.get()
        }
    }
}

static mut PER_APIC_LOCK_COUNTER: MaybeUninit<PerApicLockCounter> = MaybeUninit::uninit();
static mut PER_APIC_INITIAL_INTERRUPT_STATE: MaybeUninit<PerApicInitialInterruptState> =
    MaybeUninit::uninit();

pub fn get_per_apic_lock_counter() -> &'static PerApicLockCounter {
    unsafe { PER_APIC_LOCK_COUNTER.assume_init_ref() }
}

pub fn get_per_apic_initial_interrupt_state() -> &'static PerApicInitialInterruptState {
    unsafe { PER_APIC_INITIAL_INTERRUPT_STATE.assume_init_ref() }
}

pub struct SpinMutex<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

pub struct SpinMutexGuard<'a, T> {
    _not_send: PhantomData<Cell<()>>,
    m: &'a SpinMutex<T>,
}

impl<T> SpinMutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> SpinMutexGuard<'_, T> {
        let interrupts_enabled = interrupts::are_enabled();

        if interrupts_enabled {
            interrupts::disable();
        }

        let per_apic_lock_counter = get_per_apic_lock_counter();
        let per_apic_initial_interrupt_state = get_per_apic_initial_interrupt_state();

        let current_lock_count = unsafe { per_apic_lock_counter.get(get_local_apic_id()) };

        if current_lock_count == 0 {
            unsafe {
                per_apic_initial_interrupt_state.set(get_local_apic_id(), interrupts_enabled);
            }
        }

        // per-cpuロックカウンタをインクリメントする
        unsafe {
            let per_apic_lock_counter = get_per_apic_lock_counter();
            per_apic_lock_counter.increment(get_local_apic_id());
        }

        while self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            spin_loop();
        }

        SpinMutexGuard {
            m: self,
            _not_send: PhantomData,
        }
    }
}

impl<T> Deref for SpinMutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.m.data.get() }
    }
}

impl<T> DerefMut for SpinMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.m.data.get() }
    }
}

impl<T> Drop for SpinMutexGuard<'_, T> {
    fn drop(&mut self) {
        // Local APICの割り込みを無効化する
        if interrupts::are_enabled() {
            interrupts::disable();
        }

        // per-cpuロックカウンタをデクリメントする
        let per_apic_lock_counter = get_per_apic_lock_counter();
        unsafe {
            per_apic_lock_counter.decrement(get_local_apic_id());
        }

        // ロックを解除する
        self.m.locked.store(false, Ordering::Release);

        // per-cpuロックカウンタが0になったら割り込み状態を初期状態に戻す
        let count = unsafe { per_apic_lock_counter.get(get_local_apic_id()) };
        if count == 0 {
            let per_apic_initial_interrupt_state = get_per_apic_initial_interrupt_state();
            if unsafe { per_apic_initial_interrupt_state.get(get_local_apic_id()) } {
                interrupts::enable();
            }
        }
    }
}

unsafe impl<T: Send> Sync for SpinMutex<T> {}
unsafe impl<T: Send> Send for SpinMutex<T> {}

pub fn init() {
    let per_apic_initial_interrupt_state = PerApicInitialInterruptState {
        array: core::array::from_fn(|_| UnsafeCell::new(false)),
    };
    let per_apic_lock_counter = PerApicLockCounter {
        array: core::array::from_fn(|_| UnsafeCell::new(0)),
    };

    unsafe {
        (*addr_of_mut!(PER_APIC_INITIAL_INTERRUPT_STATE)).write(per_apic_initial_interrupt_state);
        (*addr_of_mut!(PER_APIC_LOCK_COUNTER)).write(per_apic_lock_counter);
    }
}
