#![no_std]
#![no_main]

mod error;
mod graphic;

use core::arch::asm;
use core::panic::PanicInfo;

use graphic::framebuffer::FrameBufferWriter;
use graphic::{font, RgbColor};

static HELLO: &[u8] = b"Hello World!";

/// kernel entrypoint
pub fn kernel_main(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    // initialize framebuffer
    let mut framebuffer_from_bootloader = boot_info
        .framebuffer
        .take()
        .expect("failed to get framebuffer.");
    let mut framebuffer_writer =
        FrameBufferWriter::from_bootloader_api(&mut framebuffer_from_bootloader)
            .expect("Failed to create FrameBuffer from Framebuffer-in-bootloader_api");

    for x in 0..framebuffer_writer.width() {
        for y in 0..framebuffer_writer.height() {
            framebuffer_writer.pixel_write(x, y, &RgbColor::new(0xeb, 0xdb, 0xb2));
        }
    }

    font::write_ascii_string(
        &mut framebuffer_writer,
        100,
        100,
        b"Konosuba!",
        &RgbColor::new(0x28, 0x28, 0x28),
    );

    loop {
        unsafe { asm!("hlt") }
    }
}

bootloader_api::entry_point!(kernel_main);

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
