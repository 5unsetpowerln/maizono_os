use core::arch::asm;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut, Index, IndexMut};

use crate::mutex::Mutex;

#[allow(dead_code)]
const PAGE_SIZE_4K: usize = 1024 * 4;
const PAGE_SIZE_2M: usize = 1024 * 1024 * 2;
const PAGE_SIZE_1G: usize = 1024 * 1024 * 1024 * 1;

const NUMBER_OF_PAGE_DIR: usize = 64;

#[repr(align(0x1000))] // PAGE_SIZE_4k
struct PageMapLevel4Table(UnsafeCell<[u64; 512]>);
unsafe impl Sync for PageMapLevel4Table {}

impl PageMapLevel4Table {
    const fn new() -> Self {
        Self(UnsafeCell::new([0; 512]))
    }

    fn addr(&self) -> u64 {
        let ptr = self as *const Self;
        return ptr as u64;
    }

    fn get(&self, index: usize) -> &u64 {
        debug_assert!(index < NUMBER_OF_PAGE_DIR);
        unsafe { &(*self.0.get())[index] }
    }

    #[allow(clippy::mut_from_ref)]
    fn get_mut(&self, index: usize) -> &mut u64 {
        debug_assert!(index < NUMBER_OF_PAGE_DIR);
        unsafe { &mut (*self.0.get())[index] }
    }
}

#[repr(align(0x1000))] // PAGE_SIZE_4k
struct PageDirectoryPointerTable(UnsafeCell<[u64; 512]>);
unsafe impl Sync for PageDirectoryPointerTable {}

impl PageDirectoryPointerTable {
    const fn new() -> Self {
        Self(UnsafeCell::new([0; 512]))
    }

    fn addr(&self) -> u64 {
        let ptr = self as *const Self;
        return ptr as u64;
    }

    fn get(&self, index: usize) -> &u64 {
        debug_assert!(index < NUMBER_OF_PAGE_DIR);
        unsafe { &(*self.0.get())[index] }
    }

    #[allow(clippy::mut_from_ref)]
    fn get_mut(&self, index: usize) -> &mut u64 {
        debug_assert!(index < NUMBER_OF_PAGE_DIR);
        unsafe { &mut (*self.0.get())[index] }
    }
}

#[repr(align(0x1000))] // PAGE_SIZE_4k
struct PageDirectory(UnsafeCell<[[u64; 512]; NUMBER_OF_PAGE_DIR]>);

unsafe impl Sync for PageDirectory {}

impl PageDirectory {
    const fn new() -> Self {
        Self(UnsafeCell::new([[0; 512]; NUMBER_OF_PAGE_DIR]))
    }

    fn len(&self) -> usize {
        unsafe { (*self.0.get()).len() }
    }

    fn len_inner(&self) -> usize {
        let inner = &unsafe { *self.0.get() };
        inner[0].len()
    }

    fn addr(&self) -> u64 {
        let ptr = self as *const Self;
        ptr as u64
    }

    fn get(&self, index: usize) -> &[u64; 512] {
        debug_assert!(index < NUMBER_OF_PAGE_DIR);
        unsafe { &(*self.0.get())[index] }
    }

    #[allow(clippy::mut_from_ref)]
    fn get_mut(&self, index: usize) -> &mut [u64; 512] {
        debug_assert!(index < NUMBER_OF_PAGE_DIR);
        unsafe { &mut (*self.0.get())[index] }
    }
}

static PAGE_MAP_LEVEL4_TABLE: PageMapLevel4Table = PageMapLevel4Table::new();
static PAGE_DIR_PTR_TABLE: PageDirectoryPointerTable = PageDirectoryPointerTable::new();
static PAGE_DIR: PageDirectory = PageDirectory::new();

pub fn init() {
    *PAGE_MAP_LEVEL4_TABLE.get_mut(0) = PAGE_DIR_PTR_TABLE.addr() | 0x003;

    for i in 0..PAGE_DIR.len() {
        *PAGE_DIR_PTR_TABLE.get_mut(i) = PAGE_DIR.get(i).as_ptr() as u64 | 0x003;

        for j in 0..PAGE_DIR.len_inner() {
            PAGE_DIR.get_mut(i)[j] = ((i * PAGE_SIZE_1G + j * PAGE_SIZE_2M) | 0x083) as u64;
        }
    }

    unsafe {
        asm!(
            "mov cr3, {}",
            in(reg) &PAGE_MAP_LEVEL4_TABLE
        );
    }
}
