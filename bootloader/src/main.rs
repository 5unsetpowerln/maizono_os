#![no_main]
#![no_std]

use core::fmt::Write;
use core::panic::PanicInfo;

// use log::info;
use uefi::{
    entry,
    table::{Boot, SystemTable},
    Handle, Status,
};

use core::arch::asm;
// use uefi::prelude::*;

#[entry]
// fn main() -> Status {
fn main(_image: Handle, mut system_table: SystemTable<Boot>) -> Status {
    // uefi::helpers::init().unwrap();
    // system_table.stdout().reset(false).unwrap();
    writeln!(system_table.stdout(), "Hello, World!").unwrap();
    // info!("Hello, World!");

    loop {
        unsafe { asm!("hlt") }
    }
    // Status::SUCCESS
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
