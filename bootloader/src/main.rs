#![no_main]
#![no_std]

mod kernel;

use core::arch::asm;

extern crate alloc;
use acpi::rsdp::Rsdp;
use alloc::vec::Vec;
use anyhow::Error;
use anyhow::Result;
use anyhow::anyhow;
use common::boot::BootInfo;
use common::graphic::GraphicInfo;
use kernel::load_kernel;
use log::debug;
use log::error;
use log::info;
use uefi::CStr16;
use uefi::boot::ScopedProtocol;
use uefi::helpers;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::proto::media::file::File;
use uefi::proto::media::file::FileAttribute;
use uefi::proto::media::file::FileInfo;
use uefi::proto::media::file::FileMode;
use uefi::proto::media::file::FileType;
use uefi::proto::media::file::RegularFile;
use uefi::table::cfg::ACPI2_GUID;
use uefi::{prelude::*, proto::media::file::Directory};

#[entry]
fn efi_main() -> Status {
    main_inner()
}

fn main_inner() -> Status {
    helpers::init().unwrap();

    // init graphic
    info!("Opening gop.");
    let graphic_info = init_graphic();

    // load kernel
    info!("Loading kernel.");
    let kernel = load_kernel();
    debug!("entrypoint: 0x{:x}", kernel.entry_point_addr());

    // init acpi
    info!("Getting RSDP.");
    let rsdp = init_acpi();

    // exit boot services
    info!("exiting boot services.");
    info!("#############################");
    info!("### KERNEL WILL BE CALLED ###");
    info!("#############################");

    let memory_map = unsafe { boot::exit_boot_services(boot::MemoryType::RUNTIME_SERVICES_DATA) };

    let boot_info = BootInfo::new(graphic_info, memory_map, rsdp);
    kernel.run(&boot_info);

    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

fn init_graphic() -> GraphicInfo {
    let gop_handle = boot::get_handle_for_protocol::<GraphicsOutput>()
        .expect("Failed to get handle for protocol of GraphicsOutput.");
    let mut gop = boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle)
        .expect("Failed to open protocol of GraphicsOutput exclusively.");
    GraphicInfo::from_gop(&mut gop).expect("Failed to get graphic info from gop.")
}

fn init_acpi() -> &'static Rsdp {
    if let Some(rsdp_addr) = system::with_config_table(|table| {
        let acpi_entry = table.iter().find(|e| e.guid == ACPI2_GUID);
        acpi_entry.map(|e| e.address as u64)
    }) {
        let rsdp = unsafe { &*(rsdp_addr as *const Rsdp) };
        if let Err(e) = rsdp.validate() {
            error!("Failed to validate rsdp: {:?}", e);
        }

        return rsdp;
    } else {
        error!("rsdp was not found");
        panic!("")
    }
}

fn open_root_dir(image: Handle) -> Directory {
    let mut simple_file_system = uefi::boot::get_image_file_system(image).unwrap();
    simple_file_system.open_volume().unwrap()
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

fn get_file_size(file: &mut RegularFile) -> Result<u64> {
    let mut prev_size = 0;
    let mut count = 0;

    loop {
        if count > 0x100 {
            return Err(anyhow!("Stopped to attempt allocate file info."));
        }

        let mut buffer = Vec::new();
        let size = prev_size + 0x20;
        buffer.resize(size, 0);

        match file.get_info::<FileInfo>(&mut buffer) {
            Ok(c) => return Ok(c.file_size()),
            Err(e) => {
                if e.status() == Status::BUFFER_TOO_SMALL {
                    prev_size = buffer.len();
                    count += 1;
                    continue;
                } else {
                    return Err(anyhow!(Error::msg(e)));
                }
            }
        }
    }
}

fn open_file(file_name: &CStr16) -> RegularFile {
    let mut simple_file_system = uefi::boot::get_image_file_system(boot::image_handle()).unwrap();
    let mut root_dir = simple_file_system.open_volume().unwrap();

    let file = match root_dir
        .open(file_name, FileMode::Read, FileAttribute::empty())
        .expect("Failed to open kernel file.")
        .into_type()
        .expect(
            "Failed to make the kernel file handler into file type (regular file or directory).",
        ) {
        FileType::Regular(f) => f,
        FileType::Dir(_) => {
            panic!("{} was a directory. It must be a regular file.", file_name)
        }
    };

    file
}
