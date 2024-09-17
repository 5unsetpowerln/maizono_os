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
// #[no_mangle]
// pub extern "C" fn _start(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
pub extern "C" fn _start() -> ! {
    // // generate maizono's informations from bootloader_api's informations.
    // let graphic_info = common::graphic::GraphicInfo::from(
    //     boot_info
    //         .framebuffer
    //         .take()
    //         .expect("failed to get framebuffer."),
    // );

    // // init framebuffer module
    // framebuffer::init(&graphic_info, RgbColor::from(0x28282800));

    // // init console module
    // console::init(RgbColor::from(0x3c383600), RgbColor::from(0xebdbb200));

    // // init pci module
    // pci::init();

    // printk!("framebuffer width: {}", framebuffer::width().unwrap());
    // printk!("framebuffer height: {}", framebuffer::height().unwrap());

    // match pci::scan_all_bus() {
    //     Ok(_) => {
    //         let devices = match pci::get_devices() {
    //             Ok(d) => d,
    //             Err(err) => {
    //                 printk!("Failed to get devices: {:?}", err);
    //                 panic!();
    //             }
    //         };

    //         let mut xhc_device = None;
    //         for device in devices {
    //             if device.is_xhc() {
    //                 xhc_device.replace(device);

    //                 if device.is_intel() {
    //                     break;
    //                 }
    //             }
    //         }

    //         if let Some(found_xhc_device) = xhc_device {
    //             // printk!("found xhc device: {:?}", found_xhc_device);
    //             let xhc_base_addr = match found_xhc_device.read_base_addr(0) {
    //                 Ok(a) => a,
    //                 Err(err) => {
    //                     printk!("Failed to read base address from xhc device {:?}", err);
    //                     panic!()
    //                 }
    //             };

    //             // xhc_base_addr
    //             let xhc_mmio_base = xhc_base_addr & !(0xf as u64);
    //             printk!("xhc mmio base: {:016X}", xhc_mmio_base);
    //         }
    //     }
    //     Err(err) => {
    //         printk!("failed to scann all the bus: {:?}", err);
    //     }
    // };

    loop {
        unsafe { asm!("hlt") }
    }
}

// bootloader_api::entry_point!(kernel_main);

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
