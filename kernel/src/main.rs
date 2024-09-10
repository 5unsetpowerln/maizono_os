#![no_std]
#![no_main]

mod error;
mod graphic;

use core::arch::asm;
use core::panic::PanicInfo;

use common::graphic::RgbColor;
use graphic::{
    console,
    framebuffer::{self},
    mouse,
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

    framebuffer::fill_rect(100, 100, 100, 100, RgbColor::from(0xcc241d00));
    printk!("favorite anime: {}", "konosuba!");

    mouse::draw_cursor();
    loop {
        unsafe { asm!("hlt") }
    }
}

bootloader_api::entry_point!(kernel_main);

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
