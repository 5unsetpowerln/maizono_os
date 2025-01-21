#![no_std]
#![no_main]
#![feature(inherent_associated_types)]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![feature(const_mut_refs)]
#![feature(ascii_char)]
#![feature(ascii_char_variants)]

// extern crate alloc;

mod acpi;
mod arch;
mod error;
mod gdt;
mod graphic;
mod interrupts;
mod memory_map;
mod paging;
mod pci;
mod phys_mem_manager;
mod ps2;

use core::panic::PanicInfo;
use core::{arch::asm, ptr::read_unaligned};

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
    // new_entry: extern "sysv64" fn(&BootInfo) -> !,
    new_entry: fn(&BootInfo) -> !,
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
#[unsafe(no_mangle)]
pub extern "sysv64" fn _start(boot_info: &BootInfo) -> ! {
    switch_to_kernel_stack(main, boot_info);
}

fn main(boot_info: &BootInfo) -> ! {
    frame_buffer::frame_buf()
        .unwrap()
        .init(&boot_info.graphic_info, RgbColor::from(0x28282800))
        .unwrap();
    console::console()
        .unwrap()
        .init(RgbColor::from(0x3c383600), RgbColor::from(0xebdbb200))
        .unwrap();
    gdt::init();
    paging::init();
    pci::devices()
        .unwrap()
        .init()
        .unwrap_or_else(|err| kprintln!("{:#?}", err));

    let rsdp_addr = boot_info.rsdp_addr.unwrap_or_else(|| {
        kprintln!("RSDP adderss wan't found. The kernel will panic.");
        panic!();
    });
    kprintln!("rsdp_addr: 0x{:X}", rsdp_addr.get());

    unsafe { acpi::init(rsdp_addr) };
    // timer::init_local_apic_timer();
    // timer::start_local_apic_timer();

    ps2::init();
    interrupts::init();
    x86_64::instructions::interrupts::enable();

    phys_mem_manager::mem_manager().init(&boot_info.memory_map);

    kprintln!("It didn't crash.");
    loop {
        unsafe { asm!("hlt") }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kprintln!("[panic]");
    kprintln!("{}", info);
    loop {
        unsafe { asm!("hlt") }
    }
}
