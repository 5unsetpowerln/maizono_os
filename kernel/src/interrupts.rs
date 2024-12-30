use core::ptr::write_volatile;
use spin::Lazy;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::printk;

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    idt[InterruptVector::LocalAPICTimer as u8].set_handler_fn(timer_interrupt_handler);
    idt
});

pub fn notify_end_of_interrupt() {
    unsafe { write_volatile(0xfee000b0 as *mut u32, 0) }
}

pub fn init_idt() {
    IDT.load();
}

// extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
//     // printk!("{:#?}", stack_frame);
//     printk!("breakpoint exception occured.");
//     // notify_end_of_interrupt();
//     // unsafe { PICS.lock().notify_end_of_interrupt(); }
// }

// extern "x86-interrupt" fn double_fault_handler(
//     stack_frame: InterruptStackFrame,
//     _error_code: u64,
// ) -> ! {
//     printk!("double fault occurred.");
//     panic!();
// }

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    printk!("A");
    notify_end_of_interrupt();
}

#[repr(u8)]
pub enum InterruptVector {
    LocalAPICTimer = 0x41,
}
