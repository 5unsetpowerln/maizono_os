#![no_std]
#![no_main]
#![feature(inherent_associated_types)]

// extern crate alloc;

mod bitmap_mem_manager;
mod error;
mod graphic;
mod memory_map;
mod paging;
mod pci;
mod segmentation;

use core::arch::asm;
use core::panic::PanicInfo;

use common::{boot::BootInfo, graphic::RgbColor};
use graphic::{
    console,
    frame_buffer::{self},
};

const KERNEL_STACK_SIZE: usize = 1024 * 1024;
static KERNEL_STACK: KernelStack = KernelStack::new();
#[repr(align(16))]
struct KernelStack([u8; KERNEL_STACK_SIZE]);
impl KernelStack {
    const fn new() -> Self {
        Self([0; KERNEL_STACK_SIZE])
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }
}

fn switch_to_kernel_stack(
    new_entry: extern "sysv64" fn(&BootInfo) -> !,
    boot_info: &BootInfo,
) -> ! {
    unsafe {
        asm!(
            "mov rdi, {}",
            "mov rsp, {}",
            "call {}",
            in(reg) boot_info, in(reg) KERNEL_STACK.as_ptr() as u64 + KERNEL_STACK.len() as u64,
            in(reg) new_entry
        );
    }
    loop {
        unsafe { asm!("hlt") }
    }
}

/// kernel entrypoint
#[no_mangle]
#[export_name = "_start"]
pub extern "sysv64" fn _start(boot_info: &BootInfo) -> ! {
    switch_to_kernel_stack(main, boot_info);
}

extern "sysv64" fn main(boot_info: &BootInfo) -> ! {
    frame_buffer::frame_buf()
        .unwrap()
        .init(&boot_info.graphic_info, RgbColor::from(0x28282800))
        .unwrap();
    console::console()
        .unwrap()
        .init(RgbColor::from(0x3c383600), RgbColor::from(0xebdbb200))
        .unwrap();

    segmentation::init();
    paging::init();

    if let Err(err) = pci::init() {
        printk!("{:#?}", err);
    }

    printk!("kernel_main: {}", main as *mut fn() as u64);
    printk!("framebuffer width: {}", frame_buffer::width().unwrap());
    printk!("framebuffer height: {}", frame_buffer::height().unwrap());

    bitmap_mem_manager::bitmap_mem_manager().init(&boot_info.memory_map);

    printk!("hello");

    loop {
        unsafe { asm!("hlt") }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
