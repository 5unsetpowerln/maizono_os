use core::ops::Deref;

use x86_64::structures::paging::{PageTableIndex, page_table::PageTableLevel};

#[derive(Clone, Copy)]
pub struct VirtAddr {
    inner: x86_64::VirtAddr,
}

impl VirtAddr {
    pub const fn new(value: u64) -> Self {
        Self {
            inner: x86_64::VirtAddr::new(value),
        }
    }

    pub fn set_page_table_index(&mut self, level: PageTableLevel, index: PageTableIndex) {
        let original_addr = self.inner.as_u64();
        let index = u64::from(index);
        let shift = 12 + (9 * (level as u8 - 1));
        let new_addr = (original_addr & !(0b111111111 << shift)) | (index << shift);

        self.inner = x86_64::VirtAddr::new(new_addr);
    }

    pub fn align_up_(self, align: u64) -> Self {
        Self {
            inner: self.align_up(align),
        }
    }

    pub fn align_down_(self, align: u64) -> Self {
        Self {
            inner: self.align_down(align),
        }
    }
}

impl Deref for VirtAddr {
    type Target = x86_64::VirtAddr;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
