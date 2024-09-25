use core::slice;

use alloc::vec;
use anyhow::{anyhow, bail, Error, Result};
use common::boot::Kernel;
use goblin::elf;
use log::debug;
use uefi::{
    boot::{self, AllocateType, MemoryType},
    cstr16,
    proto::media::file::{File, FileAttribute, FileInfo, FileMode, FileType},
    runtime::set_virtual_address_map,
    CStr16,
};

use crate::{file_info_size, open_root_dir};

const KERNEL_FILE_NAME: &CStr16 = cstr16!("kernel.elf");
const UEFI_PAGE_SIZE: usize = 0x1000;

pub fn load_kernel() -> Result<Kernel> {
    let mut root_dir = open_root_dir(boot::image_handle());

    let mut kernel_file = match root_dir
        .open(KERNEL_FILE_NAME, FileMode::Read, FileAttribute::empty())
        .map_err(|e| Error::msg(e).context("Failed to open kernel file."))?
        .into_type()
        .map_err(|e| {
            Error::msg(e).context(
                "Failed to make the kernel file handler into file type (regular file or directory).",
            )
        })? {
        FileType::Regular(f) => f,
        FileType::Dir(_) => {
            bail!(anyhow!(
                "{} was a directory. It must be a regular file.",
                KERNEL_FILE_NAME
            ));
        }
    };

    let mut kernel_file_info_vec = vec![0; file_info_size(KERNEL_FILE_NAME)];
    let kernel_file_info = kernel_file
        .get_info::<FileInfo>(&mut kernel_file_info_vec)
        .map_err(|e| Error::msg(e).context("Failed to get information of kernel file."))?;

    let kernel_file_size = kernel_file_info.file_size() as usize;

    let mut kernel_file_vec = vec![0; kernel_file_size];
    kernel_file
        .read(&mut kernel_file_vec)
        .map_err(|e| Error::msg(e).context("Failed to read data from the kernel file"))?;

    Ok(load_elf(&kernel_file_vec)
        .map_err(|e| Error::msg(e).context("Failed to load the elf file"))?)
}

fn load_elf(src: &[u8]) -> Result<Kernel> {
    let elf =
        elf::Elf::parse(src).map_err(|e| Error::msg(e).context("Failed to parse the elf."))?;

    let (dest_range, base_addr) = {
        let mut dest_start = 0;
        let mut dest_end = 0;
        for program_header in elf.program_headers.iter() {
            if program_header.p_type != elf::program_header::PT_LOAD {
                continue;
            }
            dest_start = dest_start.min(program_header.p_paddr);
            dest_end = dest_end.max(program_header.p_vaddr + program_header.p_memsz);
        }
        ((dest_end - dest_start) as usize, dest_start)
    };

    let page_size = (dest_range + UEFI_PAGE_SIZE - 1) / UEFI_PAGE_SIZE;

    boot::allocate_pages(
        AllocateType::AnyPages,
        // AllocateType::Address(base_addr),
        MemoryType::LOADER_DATA,
        page_size,
    )
    .map_err(|e| Error::msg(e).context("Failed to allocate pages for the kernel."))?
    .as_ptr() as u64;

    copy_load_segment(src, &elf);
    debug!("base addr: 0x{:X}", base_addr);
    // copy_dynamic_segment(src, base_addr, &elf);

    let entry_point_addr = elf.entry;

    Ok(Kernel::new(base_addr, entry_point_addr))
}

fn copy_load_segment(src: &[u8], elf: &elf::Elf) -> Result<()> {
    for program_header in elf.program_headers.iter() {
        if program_header.p_type != elf::program_header::PT_LOAD {
            continue;
        }

        copy_segment(src, program_header)
            .map_err(|e| Error::msg(e).context("Failed to copy the segment."))?;
    }
    Ok(())
}

fn copy_dynamic_segment(src: &[u8], elf: &elf::Elf) -> Result<()> {
    for program_header in elf.program_headers.iter() {
        if program_header.p_type != elf::program_header::PT_DYNAMIC {
            debug!("type: {}", program_header.p_type);
            debug!(
                "range: 0x{:X} ~ 0x{:X}",
                program_header.p_paddr,
                program_header.p_paddr + program_header.p_memsz
            );
            debug!("");
            continue;
        }

        copy_segment(src, program_header)
            .map_err(|e| Error::msg(e).context("Failed to copy the segment."))?;
    }

    Ok(())
}

fn copy_segment(src: &[u8], program_header: &elf::ProgramHeader) -> Result<()> {
    let offset = program_header.p_offset as usize;
    let file_size = program_header.p_filesz as usize;
    let mem_size = program_header.p_memsz as usize;
    let virtual_addr = program_header.p_vaddr;
    // let physical_addr = program_header.p_paddr;

    debug!(
        "copying to 0x{:X} ~ 0x{:X}",
        virtual_addr,
        (virtual_addr + mem_size as u64)
    );

    let dest = unsafe { slice::from_raw_parts_mut(virtual_addr as *mut u8, mem_size) };

    dest[..file_size].copy_from_slice(&src[offset..offset + file_size]);
    dest[file_size..].fill(0);

    Ok(())
}
