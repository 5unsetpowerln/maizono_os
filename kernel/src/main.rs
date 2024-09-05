#![no_std]
#![no_main]

mod error;
mod framebuffer;

use core::arch::asm;
use core::panic::PanicInfo;

use framebuffer::{FrameBuffer, RgbColor};

static HELLO: &[u8] = b"Hello World!";

/// kernel entrypoint
pub fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    // initialize framebuffer
    let mut framebuffer_from_bootloader = boot_info
        .framebuffer
        .take()
        .expect("failed to get framebuffer.");
    let mut framebuffer = FrameBuffer::from_bootloader_api(&mut framebuffer_from_bootloader)
        .expect("Failed to create FrameBuffer from Framebuffer-in-bootloader_api");

    for x in 0..framebuffer.width() {
        for y in 0..framebuffer.height() {
            framebuffer.pixel_write(x, y, RgbColor::new(0xff, 0xff, 0xff));
        }
    }
    for x in 0..=200 {
        for y in 0..=200 {
            framebuffer.pixel_write(x, y, RgbColor::new(0x00, 0xff, 0x00))
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
