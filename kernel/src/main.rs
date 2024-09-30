#![no_std]
#![no_main]

// extern crate alloc;

mod error;
mod graphic;
mod pci;

use core::arch::asm;
use core::panic::PanicInfo;

use common::{boot::BootInfo, graphic::RgbColor};
use graphic::{
    console,
    frame_buffer::{self},
};

/// kernel entrypoint
// pub extern "C" fn _start(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
#[no_mangle]
#[export_name = "_start"]
pub extern "sysv64" fn _start(boot_info: &BootInfo) -> ! {
    // init framebuffer module
    frame_buffer::init(&boot_info.graphic_info, RgbColor::from(0x28282800)).unwrap();

    // init console module
    console::init(RgbColor::from(0x3c383600), RgbColor::from(0xebdbb200)).unwrap();

    // init pci module
    pci::init().unwrap();

    printk!("framebuffer width: {}", frame_buffer::width().unwrap());
    printk!("framebuffer height: {}", frame_buffer::height().unwrap());

    // xhci
    pci::xhci().unwrap();

    loop {
        unsafe { asm!("hlt") }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
