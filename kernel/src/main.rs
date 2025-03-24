#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(ascii_char)]
#![feature(ascii_char_variants)]

extern crate alloc;

mod acpi;
mod allocator;
mod arch;
mod device;
mod error;
mod frame_manager;
mod gdt;
mod graphic;
mod interrupts;
mod memory_map;
mod message;
mod mouse;
mod paging;
mod pci;
mod qemu;
mod serial;

use core::arch::asm;
use core::panic::PanicInfo;

use common::{boot::BootInfo, graphic::RgbColor};
use device::ps2;
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
    frame_buffer::init(&boot_info.graphic_info, RgbColor::from(0x28282800)).unwrap();
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
    x86_64::instructions::interrupts::disable();
    interrupts::init();
    x86_64::instructions::interrupts::enable();

    //phys_mem_manager::mem_manager().init(&boot_info.memory_map);

    mouse::init(100, 100, RgbColor::from(0x28282800));

    #[cfg(test)]
    test_main();

    kprintln!("{} * {}", frame_buffer::height(), frame_buffer::width());
    kprintln!("It didn't crash.");
    loop {
        if message::count() > 0 {
            message::handle_message();
        } else {
            unsafe { asm!("hlt") }
        }
    }
}

trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) -> () {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

#[cfg(test)]
fn test_runner(tests: &[&dyn Testable]) {
    kprintln!("Running {} tests", tests.len());

    for test in tests {
        test.run();
    }

    qemu::exit_qemu(qemu::QemuExitCode::Success);
}

#[test_case]
fn tribial_assertion() {
    assert_eq!(1, 1);
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use qemu::exit_qemu;

    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(qemu::QemuExitCode::Failed);
    loop {
        unsafe { asm!("hlt") }
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kprintln!("[panic]");
    kprintln!("{}", info);
    loop {
        unsafe { asm!("hlt") }
    }
}
