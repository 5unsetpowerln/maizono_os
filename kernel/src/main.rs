#![no_std]
#![no_main]

mod error;
mod graphic;

use core::arch::asm;
use core::panic::PanicInfo;

use graphic::{
    console,
    framebuffer::{self, FrameBufferError},
    GraphicInfo, Pixel, RgbColor,
};

/// kernel entrypoint
pub fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    // generate maizono's informations from bootloader_api's informations.
    let graphic_info = GraphicInfo::from(
        boot_info
            .framebuffer
            .take()
            .expect("failed to get framebuffer."),
    );

    // init framebuffer
    framebuffer::init(&graphic_info, RgbColor::from(0x28282800));

    // init console
    console::init(RgbColor::from(0x28282800), RgbColor::from(0xebdbb200));

    printk!("favorite anime: {}", "konosuba!");

    loop {
        unsafe { asm!("hlt") }
    }
}

bootloader_api::entry_point!(kernel_main);

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
