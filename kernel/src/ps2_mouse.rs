// #![feature(abi_x86_interrupt)]

use core::arch::asm;

use ps2_mouse::{Mouse, MouseState};
use spin::{Lazy, Mutex};
use x86_64::{
    instructions::port::{Port, PortReadOnly, PortWriteOnly},
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame},
};

use crate::{interrupts, printk};

// pub static MOUSE: Lazy<Mutex<Mouse>> = Lazy::new(|| Mutex::new(Mouse::new()));
static MOUSE: Mutex<Mouse> = Mutex::new(Mouse::new());

pub fn check_ps2_mouse() -> bool {
    let mut data_port: Port<u8> = Port::new(0x60);
    let mut cmd_port: PortWriteOnly<u8> = PortWriteOnly::new(0x64);

    unsafe {
        cmd_port.write(0xA8);
    }

    unsafe { data_port.write(0xf4) }

    let response: u8 = unsafe { data_port.read() };
    response == 0xfa
}

fn init_mouse() {
    MOUSE.lock().init().unwrap();
    MOUSE.lock().set_on_complete(on_complete);
}

fn on_complete(mouse_state: MouseState) {
    printk!("{:?}", mouse_state);
}

pub extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: &mut InterruptStackFrame) {
    // let mut port = PortReadOnly::new(0x60);
    // let packet: u8 = unsafe { port.read() };
    // MOUSE.lock().process_packet(packet);
    printk!("mouse moved!");

    // interrupt::notify_end_of_interrupt();
}
