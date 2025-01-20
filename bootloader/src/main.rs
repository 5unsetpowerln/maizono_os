#![no_main]
#![no_std]

mod kernel;

use core::arch::asm;

extern crate alloc;
use alloc::format;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::Error;
use anyhow::Result;
use boot::MemoryType;
use common::address::PhysPtr;
use common::boot::BootInfo;
use common::graphic::GraphicInfo;
use kernel::load_kernel;
use log::debug;
use log::error;
use log::info;
use runtime::Time;
use uefi::boot::ScopedProtocol;
use uefi::helpers;
use uefi::mem::memory_map::MemoryMap;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::table::cfg::ACPI2_GUID;
use uefi::{
    prelude::*,
    proto::media::file::{Directory, File, FileAttribute, FileMode, FileType},
    CStr16,
};

const MEMMAP_DUMP_NAME: &CStr16 = cstr16!("memmap_dump");

#[entry]
fn efi_main() -> Status {
    main_inner()
}

fn main_inner() -> Status {
    helpers::init().unwrap();

    // create memmap dump
    info!("creating memmap dump");
    if let Err(e) = save_memmap_dump() {
        print_error(&e);
        panic!();
    }

    info!("opening gop");
    let mut gop = match open_gop() {
        Ok(g) => g,
        Err(e) => {
            info!("hello");
            print_error(&Error::msg(e).context("Failed to open graphics-output-protocol."));
            panic!("panicked.");
        }
    };
    let graphic_info =
        match GraphicInfo::from_gop(&mut gop) {
            Ok(info) => info,
            Err(err) => {
                print_error(&Error::msg(err.msg()).context(
                    "Failed to create common::GraphicInfo to give to the kernel from gop.",
                ));
                panic!("panicked");
            }
        };
    info!("frame_buffer_addr: 0x{:X}", graphic_info.frame_buffer_addr);

    info!("loading kernel");
    let kernel = match load_kernel() {
        Ok(addr) => addr,
        Err(err) => {
            print_error(&err.context("Failed to load the kernel"));
            panic!("panicked");
        }
    };
    info!("kernel_entry_point: 0x{:X}", kernel.entry_point_addr());
    info!("kernel_base_addr: 0x{:X}", kernel.base_addr());
    debug!("main_inner: 0x{:X}", main_inner as *const fn() as u64);
    // debug!(
    //     "0x{:X} == 0x{:X} = {}",
    //     kernel.entry_point as *const fn() as u64,
    //     kernel.entry_point_addr,
    //     kernel.entry_point as *const fn() as u64 == kernel.entry_point_addr
    // );

    info!("finding rsdp addr.");
    let rsdp_addr = find_rsdp();
    info!("rsdp_addr: {:?}", rsdp_addr);

    info!("exiting boot services.");
    let memory_map = unsafe { boot::exit_boot_services(boot::MemoryType::BOOT_SERVICES_DATA) };

    let boot_info = BootInfo::new(graphic_info, memory_map, rsdp_addr);
    kernel.run(&boot_info);

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

fn find_rsdp() -> Option<PhysPtr> {
    system::with_config_table(|table| {
        let acpi_entry = table.iter().find(|e| e.guid == ACPI2_GUID);
        acpi_entry.map(|e| PhysPtr::from_ptr(e.address))
    })
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

fn save_memmap_dump() -> Result<()> {
    let memmap = uefi::boot::memory_map(MemoryType::LOADER_DATA)
        .map_err(|e| Error::msg(e).context("Failed to get memory map"))?;

    let mut root_dir = open_root_dir(boot::image_handle());
    let mut memmap_file = match root_dir
        .open(
            MEMMAP_DUMP_NAME,
            FileMode::CreateReadWrite,
            FileAttribute::empty(),
        )
        .map_err(|e| Error::msg(e).context("Failed to open memmap file"))?
        .into_type()
        .map_err(|e| {
            Error::msg(e).context(
                "Failed to make memmap file handler into file type (regular file or directory).",
            )
        })? {
        FileType::Regular(f) => f,
        FileType::Dir(_d) => {
            bail!(anyhow!(
                "{} was directory. It must be a regular file.",
                MEMMAP_DUMP_NAME
            ))
        }
    };

    memmap_file
        .write(b"index, type, type(name), physical_start, number of pages, attribute\n")
        .map_err(|e| Error::msg(e).context("Failed to write data to memmap dump file"))?;
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
        memmap_file
            .write(line.as_bytes())
            .map_err(|e| Error::msg(e).context("Failed to write data to memmap dump file"))?;
    }
    memmap_file.close();

    Ok(())
}

fn open_gop() -> Result<ScopedProtocol<GraphicsOutput>> {
    let gop_handle = boot::get_handle_for_protocol::<GraphicsOutput>().map_err(|e| {
        Error::msg(e).context("Failed to get handle for protocol of GraphicsOutput.")
    })?;
    let gop = boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle).map_err(|e| {
        Error::msg(e).context("Failed to open protocol of GraphicsOutput exclusively.")
    })?;
    Ok(gop)
}

fn print_error(err: &Error) {
    error!("{:#?}", err);
}
