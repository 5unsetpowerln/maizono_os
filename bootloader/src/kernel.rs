use core::slice::from_raw_parts_mut;

use alloc::vec;
use common::boot::Kernel;
use goblin::elf;
use uefi::{
    CStr16,
    boot::{self, AllocateType, MemoryType},
    cstr16,
    proto::media::file::{File, FileInfo},
};

use crate::open_file;

const KERNEL_FILE_NAME: &CStr16 = cstr16!("kernel.elf");
const UEFI_PAGE_SIZE: usize = 0x1000;

pub fn load_kernel() -> Kernel {
    let mut file = open_file(KERNEL_FILE_NAME);

    let file_info = file
        .get_boxed_info::<FileInfo>()
        .expect("Failed to get kernel file info.");
    let file_size = file_info.file_size() as usize;
    let mut kernel_file_vec = vec![0; file_size];

    file.read(&mut kernel_file_vec)
        .expect("Failed to read kernel to the buffer.");

    load_elf(&kernel_file_vec)
}

fn load_elf(src: &[u8]) -> Kernel {
    let elf = elf::Elf::parse(src).expect("Failed to parse the elf.");

    let mut dest_start = u64::MAX;
    let mut dest_end = 0;
    for ph in elf.program_headers.iter() {
        if ph.p_type == elf::program_header::PT_LOAD {
            dest_start = dest_start.min(ph.p_vaddr);
            dest_end = dest_end.max(ph.p_vaddr + ph.p_memsz);
        }
    }
    let dest_range = dest_end - dest_start;

    let page_count = dest_range.div_ceil(UEFI_PAGE_SIZE as u64) as usize;
    let _ = boot::allocate_pages(
        AllocateType::Address(dest_start),
        MemoryType::LOADER_DATA,
        page_count,
    )
    .expect("Failed to allocate pages for the kernel.");

    let copy_size_sum = copy_load_segment(src, &elf);
    assert!(copy_size_sum < page_count * UEFI_PAGE_SIZE);

    Kernel::new(dest_start, elf.entry)
}

fn copy_load_segment(src: &[u8], elf: &elf::Elf) -> usize {
    let mut copy_size_sum = 0;

    for ph in elf.program_headers.iter() {
        if ph.p_type == elf::program_header::PT_LOAD {
            let offset = ph.p_offset as usize;
            let file_size = ph.p_filesz as usize;
            let mem_size = ph.p_memsz as usize;

            let dest = unsafe { from_raw_parts_mut(ph.p_vaddr as *mut u8, mem_size) };
            dest[..file_size].copy_from_slice(&src[offset..offset + file_size]);
            dest[file_size..].fill(0);
            copy_size_sum += mem_size;
        }
    }

    copy_size_sum
}
