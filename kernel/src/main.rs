#![no_std]
#![no_main]

// extern crate alloc;

mod error;
mod graphic;
mod pci;

use core::arch::asm;
use core::panic::PanicInfo;

use common::graphic::RgbColor;
use graphic::{
    console,
    framebuffer::{self},
};

/// kernel entrypoint
pub fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    // generate maizono's informations from bootloader_api's informations.
    let graphic_info = common::graphic::GraphicInfo::from(
        boot_info
            .framebuffer
            .take()
            .expect("failed to get framebuffer."),
    );

    // init framebuffer
    framebuffer::init(&graphic_info, RgbColor::from(0x28282800));

    // init console
    console::init(RgbColor::from(0x28282800), RgbColor::from(0xebdbb200));

    framebuffer::draw_rect(100, 100, 100, 100, RgbColor::from(0xcc241d00));

    match pci::scan_all_bus() {
        Ok(_) => {
            let devices = match pci::get_devices() {
                Ok(d) => d,
                Err(err) => {
                    printk!("Failed to get devices: {:?}", err);
                    panic!();
                }
            };

            for (i, device) in devices.enumerate() {
                printk!("device {}: {:?}", i, device);
            }
        }
        Err(err) => {
            printk!("failed to scann all the bus: {:?}", err)
        }
    }

    loop {
        unsafe { asm!("hlt") }
    }
}

bootloader_api::entry_point!(kernel_main);

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
