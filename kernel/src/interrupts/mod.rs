pub mod apic;

use crate::message::Message;
use crate::{acpi, device::ps2};
use apic::{IoApic, LocalApic, write_msr};
use common::error;
use log::{error, info};
use spin::{Lazy, Once};
use x86_64::instructions::port::Port;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

use crate::message;

pub static LOCAL_APIC: Once<LocalApic> = Once::new();

const EXTERNAL_IRQ_OFFSET: u8 = 32;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum IRQ {
    Timer = 0,
    Keyboard = 1, // PS/2 Keyboard
    Mouse = 12,
    Error = 19, // Cpu internal error (LVT Error)
}

impl IRQ {
    const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[repr(u8)]
pub enum InterruptVector {
    ExternalIrqTimer = EXTERNAL_IRQ_OFFSET + IRQ::Timer.as_u8(),
    ExternalIrqKeyboard = EXTERNAL_IRQ_OFFSET + IRQ::Keyboard.as_u8(),
    ExternalIrqMouse = EXTERNAL_IRQ_OFFSET + IRQ::Mouse.as_u8(),
    ExternalIrqError = EXTERNAL_IRQ_OFFSET + IRQ::Error.as_u8(),
    LocalAPICTimer = 0x41,
}

impl InterruptVector {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.double_fault.set_handler_fn(double_fault_handler);
    idt[InterruptVector::LocalAPICTimer as u8].set_handler_fn(timer_interrupt_handler);
    idt[InterruptVector::ExternalIrqTimer.as_u8()].set_handler_fn(timer_interrupt_handler);
    idt[InterruptVector::ExternalIrqKeyboard.as_u8()]
        .set_handler_fn(ps2::keyboard::interrupt_handler);
    idt[InterruptVector::ExternalIrqMouse.as_u8()].set_handler_fn(mouse_interrupt_handler);

    idt
});

pub fn init() {
    init_idt();
    unsafe { disable_pic_8259() };
    init_apic();
}

fn init_idt() {
    IDT.load();
}

fn init_apic() {
    // Enabling APIC
    {
        const IA32_APIC_BASE_MSR: u32 = 0x1B;

        // output a local apic base which is from MADT and once which is from IA32_APIC_BASE MSR
        let local_apic_base = acpi::get_apic_info().local_apic_base();

        // update IA32_APIC_BASE MSR value to local apic base which is from MADT
        // and set bit 11 to enable apic (not local apic)
        unsafe { write_msr(IA32_APIC_BASE_MSR, (local_apic_base as u64) | (1 << 11)) };
    }

    // Initializing Local APIC
    {
        const MASKED: u32 = 1 << 16;
        let local_apic = LocalApic::new(acpi::get_apic_info().local_apic_base());
        LOCAL_APIC.call_once(|| local_apic);

        // https://github.com/mit-pdos/xv6-public/blob/master/lapic.c
        // https://wiki.osdev.org/APIC

        // Enable local APIC by setting spurious interrupt vector register.
        local_apic.write_spurious_interrupt_vector_register(0x100 | 0xff);

        // the configuration of timer is set on the timer module.

        // Disable logical interrupt lines to prevent troubles caused by unexpected interrupts.
        local_apic.write_lvt_lint0_register(MASKED);
        local_apic.write_lvt_lint1_register(MASKED);

        // Disable performance counter orverflow interrupts.
        // performance monitoring counter provides functionality to count cpu internal events such as number of cycle, number of cache mistake, etc.
        local_apic.write_lvt_performance_monitoring_counters_register(MASKED);

        // Map error interrupt to IRQ::ERROR.
        local_apic.write_lvt_error_register((EXTERNAL_IRQ_OFFSET + IRQ::Error as u8) as u32);

        // Clear error status register (require back-to-back writes).
        local_apic.write_error_status_register(0);
        local_apic.write_error_status_register(0);

        // Ack any outstanding interrupts.
        local_apic.write_end_of_interrupt_register(0);

        // Send an Init Level De-Assert to synchronise arbitration ID's.
        local_apic.write_interrupt_command_register_high(0);
        const BCAST: u32 = 0x00080000;
        const LEVEL: u32 = 0x00008000;
        const INIT: u32 = 0x00000500;
        const DELIVERY_STATUS: u32 = 0x00001000;
        local_apic.write_interrupt_command_register_low(BCAST | LEVEL | INIT);
        while (local_apic.read_interrupt_command_register_low() & DELIVERY_STATUS) != 0 {}

        // Enable interrupts on the APIC (but at this stage, whether the processor accepts the interrupt is a separate issue).
        local_apic.write_task_priority_register(0);
    }

    // Initializing I/O APIC
    {
        info!(
            "I/O APIC base: 0x{:X}",
            acpi::get_apic_info().io_apic_base()
        );
        let io_apic = IoApic::new(acpi::get_apic_info().io_apic_base());

        // https://github.com/mit-pdos/xv6-public/blob/master/ioapic.c

        if io_apic.get_id() != acpi::get_apic_info().io_apic_id() {
            panic!("id isn't equal to I/O Apic id; not a MP");
        }

        // Mark all interrupts edge-triggered, active high, disabled, and not routed to any CPUs.
        for i in 0..io_apic.get_max_amount_of_redirection_entries() {
            // Bit 16 Interrupt mask. Stops the interrupt from reaching the processor if set.
            let value = (EXTERNAL_IRQ_OFFSET as u64 + i as u64) | (1 << 16);
            unsafe {
                io_apic.set_redirection_entry_at(i as u32, value);
            }
        }

        // Redirect external interrupts to IDT via I/O Apic.
        let cpu0 = (acpi::get_apic_info().processor_id() as u64) << (32 + 24);
        unsafe {
            io_apic.set_redirection_entry_at(
                IRQ::Keyboard as u32,
                InterruptVector::ExternalIrqKeyboard as u64 | cpu0,
            );

            io_apic.set_redirection_entry_at(
                IRQ::Mouse as u32,
                InterruptVector::ExternalIrqMouse as u64 | cpu0,
            );
        }
    }
}

unsafe fn disable_pic_8259() {
    unsafe {
        Port::new(0xa1).write(0xffu8);
        Port::new(0x21).write(0xffu8);
    }
}

pub fn notify_end_of_interrupt() {
    LOCAL_APIC.wait().write_end_of_interrupt_register(0);
}

extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame) {
    info!("breakpoint exception occured.");
    notify_end_of_interrupt();
}

extern "x86-interrupt" fn double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("double fault occurred.");
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    message::enqueue(Message::LocalAPICTimerInterrupt);
    notify_end_of_interrupt();
}

extern "x86-interrupt" fn external_irq_timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    error!("An external irq timer interrupt was happened.");
    notify_end_of_interrupt();
}

extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    error!("A mouse interrupt was happned.");
    notify_end_of_interrupt();
}
