use core::arch::asm;
use core::ops::{Deref, DerefMut};

use spin::Mutex;

#[allow(dead_code)]
const PAGE_SIZE_4K: usize = 1024 * 4;
const PAGE_SIZE_2M: usize = 1024 * 1024 * 2;
const PAGE_SIZE_1G: usize = 1024 * 1024 * 1024 * 1;

const NUMBER_OF_PAGE_DIR: usize = 64;

#[repr(align(0x1000))] // PAGE_SIZE_4k
struct PageMapLevel4Table([u64; 512]);

impl PageMapLevel4Table {
    const fn new() -> Self {
        Self([0; 512])
    }
}

impl Deref for PageMapLevel4Table {
    type Target = [u64; 512];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PageMapLevel4Table {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[repr(align(0x1000))] // PAGE_SIZE_4k
struct PageDirectoryPointerTable([u64; 512]);

impl PageDirectoryPointerTable {
    const fn new() -> Self {
        Self([0; 512])
    }
}

impl Deref for PageDirectoryPointerTable {
    type Target = [u64; 512];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PageDirectoryPointerTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[repr(align(0x1000))] // PAGE_SIZE_4k
struct PageDirectory([[u64; 512]; NUMBER_OF_PAGE_DIR]);

impl PageDirectory {
    const fn new() -> Self {
        Self([[0; 512]; NUMBER_OF_PAGE_DIR])
    }

    fn len_inner(&self) -> usize {
        self.0[0].len()
    }
}

impl Deref for PageDirectory {
    type Target = [[u64; 512]; NUMBER_OF_PAGE_DIR];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PageDirectory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// static PAGE_MAP_LEVEL4_TABLE: Mutex<PageMapLevel4Table> = Mutex::new(PageMapLevel4Table::new());
// static PAGE_DIR_PTR_TABLE: Mutex<PageDirectoryPointerTable> =
//     Mutex::new(PageDirectoryPointerTable::new());
// static PAGE_DIR: Mutex<PageDirectory> = Mutex::new(PageDirectory::new());
static mut PAGE_MAP_LEVEL4_TABLE: PageMapLevel4Table = PageMapLevel4Table::new();
static mut PAGE_DIR_PTR_TABLE: PageDirectoryPointerTable = PageDirectoryPointerTable::new();
static mut PAGE_DIR: PageDirectory = PageDirectory::new();

pub fn init() {
    unsafe {
        PAGE_MAP_LEVEL4_TABLE[0] = PAGE_DIR_PTR_TABLE.as_ptr() as u64 | 0x003;
    }

    unsafe {
        for i in 0..PAGE_DIR.len() {
            PAGE_DIR_PTR_TABLE[i] = PAGE_DIR[i].as_ptr() as u64 | 0x003;
            for j in 0..PAGE_DIR.len_inner() {
                PAGE_DIR[i][j] = (i * PAGE_SIZE_1G + j * PAGE_SIZE_2M | 0x083) as u64
            }
        }
    }

    unsafe {
        asm!(
            "mov cr3, {}",
            in(reg) PAGE_MAP_LEVEL4_TABLE.as_ptr() as u64
        );
    }
}
