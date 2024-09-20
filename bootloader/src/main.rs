#![no_main]
#![no_std]

extern crate alloc;

use core::arch::asm;
use core::panic::PanicInfo;

use alloc::format;
use alloc::string::ToString;
use alloc::vec;
use anyhow::Error;
use anyhow::Result;
use boot::{AllocateType, MemoryType};
use runtime::{ResetType, Time};
use uefi::{
    mem::memory_map::MemoryMap,
    prelude::*,
    println,
    proto::media::file::{Directory, File, FileAttribute, FileInfo, FileMode, FileType},
    CStr16,
};

const MEMMAP_DUMP_NAME: &CStr16 = cstr16!("memmap_dump");
const KERNEL_FILE_NAME: &CStr16 = cstr16!("kernel.elf");
const KERNEL_BASE_ADDR: u64 = 0x100000;
const EFI_PAGE_SIZE: u64 = 0x1000;

#[entry]
fn efi_main(image: Handle, system_table: SystemTable<Boot>) -> Status {
    main_inner(image, system_table)
}

fn open_root_dir(image: Handle) -> Directory {
    let mut simple_file_system = uefi::boot::get_image_file_system(image).unwrap();
    simple_file_system.open_volume().unwrap()
}

fn file_info_size(file_name: &CStr16) -> usize {
    // uefi::proto::media::file::FileInfo (version: 0.32.0) has following fields:
    // pub struct FileInfo {
    //     size: u64,
    //     file_size: u64,
    //     physical_size: u64,
    //     create_time: Time,
    //     last_access_time: Time,
    //     modification_time: Time,
    //     attribute: FileAttribute,
    //     file_name: [Char16],
    // }
    // only file_name field has dynamic length so, caller have to give file_name.

    let u64_size = size_of::<u64>();
    let time_size = size_of::<Time>();

    let size_size = u64_size;
    let file_size_size = u64_size;
    let physical_size_size = u64_size;
    let create_time_size_size = time_size;
    let last_access_time_size = time_size;
    let modification_time_size = time_size;
    let attribute_size = size_of::<FileAttribute>();
    let file_name_size = file_name.num_bytes();

    size_size
        + file_size_size
        + physical_size_size
        + create_time_size_size
        + last_access_time_size
        + modification_time_size
        + attribute_size
        + file_name_size
}

fn save_memmap_dump(image: Handle, st: &mut SystemTable<Boot>) {
    let memmap = st
        .boot_services()
        .memory_map(MemoryType::LOADER_DATA)
        .expect("Failed to get memory map");

    let mut root_dir = open_root_dir(image);
    let mut memmap_file = match root_dir
        .open(
            MEMMAP_DUMP_NAME,
            FileMode::CreateReadWrite,
            FileAttribute::empty(),
        )
        .unwrap()
        .into_type()
        .unwrap()
    {
        FileType::Regular(f) => f,
        FileType::Dir(_d) => {
            panic!("memmap is directory.")
        }
    };

    memmap_file
        .write(b"index, type, type(name), physical_start, number of pages, attribute\n")
        .unwrap();
    for (i, d) in memmap.entries().enumerate() {
        let line = format!(
            "{}, 0x{:X}, {:?}, 0x{:08X}, 0x{:X}, 0x{:X}\n",
            i,
            d.ty.0,
            d.ty,
            d.phys_start,
            d.page_count,
            d.att.bits() & 0xfffff
        );
        memmap_file.write(line.as_bytes()).unwrap();
    }
    memmap_file.close();
}

fn load_kernel(image: Handle, st: &mut SystemTable<Boot>) -> Result<()> {
    let mut root_dir = open_root_dir(image);
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
            println!(
                "{} was a directory. It must be regular file.",
                KERNEL_FILE_NAME
            );
            panic!("");
        }
    };

    let mut kernel_file_info_vec = vec![0; file_info_size(KERNEL_FILE_NAME)];
    let kernel_file_info = kernel_file
        .get_info::<FileInfo>(&mut kernel_file_info_vec)
        .map_err(|e| Error::msg(e).context("Failed to get information of kernel file."))?;
    let kernel_file_size = kernel_file_info.file_size() as usize;

    st.boot_services()
        .allocate_pages(
            AllocateType::Address(KERNEL_BASE_ADDR as u64),
            MemoryType::LOADER_DATA,
            (kernel_file_size + EFI_PAGE_SIZE as usize - 1) / EFI_PAGE_SIZE as usize,
        )
        .map_err(|e| Error::msg(e).context("Failed to allocate pages for the kernel."))?;
    let kernel_data_buf =
        unsafe { core::slice::from_raw_parts_mut(KERNEL_BASE_ADDR as *mut u8, kernel_file_size) };
    kernel_file.read(kernel_data_buf).unwrap();
    Ok(())
}

fn main_inner(image: Handle, mut st: SystemTable<Boot>) -> Status {
    st.stdout().reset(false).unwrap();
    uefi::helpers::init().unwrap();

    // create memmap dump
    println!("creating memmap dump...");
    save_memmap_dump(image, &mut st);
    println!("done.");

    // load kernel
    println!("loading kernel...");
    if let Err(e) = load_kernel(image, &mut st) {
        print_error(e);
        panic!();
    }
    println!("done.");

    // println!("exiting boot services.");
    // // let memmap = unsafe {
    // let (runtime_services, _) = unsafe {
    //     st.exit_boot_services(MemoryType::LOADER_DATA)
    //     // uefi::boot::exit_boot_services(MemoryType::BOOT_SERVICES_DATA)
    // };
    // println!("done.");

    println!("calculating address of entry point of the kernel.");
    let entry_point_addr = u64::from_le_bytes({
        let mut s = [0u8; 8];
        s.copy_from_slice(unsafe {
            core::slice::from_raw_parts((KERNEL_BASE_ADDR + 24) as *const u8, 8)
        });
        s
    });
    let entry_point: extern "sysv64" fn() -> ! = unsafe { core::mem::transmute(entry_point_addr) };
    println!("done.");

    println!("calling kernel.");
    entry_point();
    // println!("kernel is finished.");

    // unsafe {
    //     runtime_services
    //         .runtime_services()
    //         .reset(ResetType::SHUTDOWN, Status::SUCCESS, None);
    // }
}

fn print_error(err: Error) {
    for (i, e) in err.chain().enumerate() {
        if i == 0 {
            println!("{}", e.to_string());
        } else {
            println!("    {}", e.to_string());
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    println!("Bootloader will be panicked.");
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
