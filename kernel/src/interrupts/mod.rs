pub mod apic;

use crate::acpi::get_madt;
use crate::cpu::{self, get_local_apic_info_by_idx};
use crate::message::Message;
use crate::mutex::Mutex;
use crate::x64::{IA32_APIC_BASE_MSR, write_msr};
use crate::{acpi, device::ps2};
use ::acpi::madt::InterruptSourceOverrideEntry;
use apic::{IoApic, LocalApic};
use core::arch::{asm, naked_asm};
use log::{error, info};
use proc_macro_lib::align16_fn_for_interrupt;
use spin::Lazy;
use x86_64::VirtAddr;
use x86_64::instructions::port::Port;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use crate::{message, serial_emergency_println, timer};

pub static LAPIC: Mutex<LocalApic> = Mutex::new(LocalApic::new(0));

const EXTERNAL_IRQ_OFFSET: u8 = 32;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum IRQ {
    Timer = 0,
    Keyboard = 1, // PS/2 Keyboard
    Mouse = 12,
    Error = 19, // Cpu internal error (LVT Error)
    Spurious = 31,
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
    Spurious = EXTERNAL_IRQ_OFFSET + IRQ::Spurious.as_u8(),
    LocalAPICTimer = 0x41,
}

impl InterruptVector {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    idt.divide_error
        .set_handler_fn(divide_by_zero_exception_interrupt_handler);
    idt.debug.set_handler_fn(debug_exception_interrupt_handler);
    idt.non_maskable_interrupt
        .set_handler_fn(nmi_interrupt_handler);
    idt.breakpoint
        .set_handler_fn(breakpoint_exception_interrupt_handler);
    idt.overflow
        .set_handler_fn(overflow_exception_interrupt_handler);
    idt.bound_range_exceeded
        .set_handler_fn(bound_range_exceeded_exception_interrupt_handler);
    idt.invalid_opcode
        .set_handler_fn(invalid_opecode_exception_interrupt_handler);
    idt.device_not_available
        .set_handler_fn(device_not_available_exception_interrupt_handler);
    idt.double_fault
        .set_handler_fn(double_fault_exception_interrupt_handler);
    // coprocessor segment overrunはx86_64クレートがサポートしていない
    idt.invalid_tss
        .set_handler_fn(invalid_tss_exception_interrupt_handler);
    idt.segment_not_present
        .set_handler_fn(segment_not_present_exception_interrupt_handler);
    idt.stack_segment_fault
        .set_handler_fn(stack_fault_exception_interrupt_handler);
    idt.general_protection_fault
        .set_handler_fn(general_protection_exception_interrupt_handler);
    idt.page_fault
        .set_handler_fn(page_fault_exception_interrupt_handler);
    idt.x87_floating_point
        .set_handler_fn(fpu_floating_point_error_interrupt_handler);
    idt.alignment_check
        .set_handler_fn(alignment_check_exception_interrupt_handler);
    idt.machine_check
        .set_handler_fn(machine_check_exception_interrupt_handler);
    idt.simd_floating_point
        .set_handler_fn(simd_floating_point_exception_interrupt_handler);
    idt.virtualization
        .set_handler_fn(virtualization_exception_interrupt_handler);
    idt.cp_protection_exception
        .set_handler_fn(control_protection_exception_interrupt_handler);
    idt[InterruptVector::LocalAPICTimer as u8].set_handler_fn(timer::interrupt_handler);
    idt[InterruptVector::ExternalIrqTimer.as_u8()]
        .set_handler_fn(external_irq_timer_interrupt_handler);
    idt[InterruptVector::ExternalIrqKeyboard.as_u8()]
        .set_handler_fn(ps2::keyboard::interrupt_handler);
    idt[InterruptVector::ExternalIrqMouse.as_u8()].set_handler_fn(mouse_interrupt_handler);
    idt[InterruptVector::Spurious.as_u8()].set_handler_fn(spurious_interrupt_handler);

    idt
});

pub fn init() {
    init_idt();
    unsafe { disable_pic_8259() };
    init_first_local_apic();
}

fn init_idt() {
    IDT.load();
}

fn init_first_local_apic() {
    let local_apic_info = get_local_apic_info_by_idx(0);
    let local_apic_base = get_madt().local_apic_address;
    let io_apic_info = cpu::get_io_apic_info_by_idx(0);
    let io_apic_base = io_apic_info.io_apic_address;

    // Enabling APIC
    {
        // output a local apic base which is from MADT and once which is from IA32_APIC_BASE MSR
        // let local_apic_base = .local_apic_base();

        // update IA32_APIC_BASE MSR value to local apic base which is from MADT
        // and set bit 11 to enable apic (not local apic)
        unsafe { write_msr(IA32_APIC_BASE_MSR, (local_apic_base as u64) | (1 << 11)) };
    }

    // Initializing Local APIC
    {
        const MASKED: u32 = 1 << 16;
        let mut lapic = LocalApic::new(local_apic_base);

        // https://github.com/mit-pdos/xv6-public/blob/master/lapic.c
        // https://wiki.osdev.org/APIC

        // Enable local APIC by setting spurious interrupt vector register.
        lapic.write_spurious_interrupt_vector_register(
            0x100 | InterruptVector::Spurious.as_u8() as u32,
        );

        // the configuration of timer is set on the timer module.

        // Disable logical interrupt lines to prevent troubles caused by unexpected interrupts.
        lapic.write_lvt_lint0_register(MASKED);
        lapic.write_lvt_lint1_register(MASKED);

        // Disable performance counter orverflow interrupts.
        // performance monitoring counter provides functionality to count cpu internal events such as number of cycle, number of cache mistake, etc.
        lapic.write_lvt_performance_monitoring_counters_register(MASKED);

        // Map error interrupt to IRQ::ERROR.
        lapic.write_lvt_error_register((EXTERNAL_IRQ_OFFSET + IRQ::Error as u8) as u32);

        // Clear error status register (require back-to-back writes).
        lapic.write_error_status_register(0);
        lapic.write_error_status_register(0);

        // Ack any outstanding interrupts.
        lapic.write_end_of_interrupt_register(0);

        // Send an Init Level De-Assert to synchronise arbitration ID's.
        lapic.write_interrupt_command_register_high(0);
        const BCAST: u32 = 0x00080000;
        const LEVEL: u32 = 0x00008000;
        const INIT: u32 = 0x00000500;
        const DELIVERY_STATUS: u32 = 0x00001000;
        lapic.write_interrupt_command_register_low(BCAST | LEVEL | INIT);
        while (lapic.read_interrupt_command_register_low() & DELIVERY_STATUS) != 0 {}

        // Enable interrupts on the APIC (but at this stage, whether the processor accepts the interrupt is a separate issue).
        lapic.write_task_priority_register(0);

        *LAPIC.lock() = lapic;
    }

    // Initializing I/O APIC
    {
        info!("I/O APIC base: 0x{:X}", io_apic_base);
        let io_apic = IoApic::new(io_apic_base);

        // https://github.com/mit-pdos/xv6-public/blob/master/ioapic.c

        if io_apic.get_id() != io_apic_info.io_apic_id {
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
        let cpu0 = (local_apic_info.processor_id as u64) << (32 + 24);
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
    LAPIC.lock().write_end_of_interrupt_register(0);
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn external_irq_timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    error!("An external irq timer interrupt was happened.");
    notify_end_of_interrupt();
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    error!("A mouse interrupt was happned.");
    notify_end_of_interrupt();
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn external_keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    error!("A mouse interrupt was happned.");
    notify_end_of_interrupt();
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn spurious_interrupt_handler(_stack_frame: InterruptStackFrame) {
    info!("Spurious interrupt.");
}

fn dump_frame(frame: &InterruptStackFrame) {
    serial_emergency_println!("rip: 0x{:x}", frame.instruction_pointer.as_u64());
    serial_emergency_println!("cs: 0x{:x}", frame.code_segment.0);
    serial_emergency_println!("rflags: {:?}", frame.cpu_flags);
    serial_emergency_println!("rsp: 0x{:x}", frame.stack_pointer.as_u64());
    serial_emergency_println!("ss: 0x{:x}", frame.stack_segment.0);
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn divide_by_zero_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
) {
    serial_emergency_println!("interruption: divide by zero exception");
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn debug_exception_interrupt_handler(stack_frame: InterruptStackFrame) {
    serial_emergency_println!("interruption: debug exception");
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn nmi_interrupt_handler(stack_frame: InterruptStackFrame) {
    serial_emergency_println!("interruption: nmi interrupt");
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn breakpoint_exception_interrupt_handler(stack_frame: InterruptStackFrame) {
    serial_emergency_println!("interruption: breakpoint exception");
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn overflow_exception_interrupt_handler(stack_frame: InterruptStackFrame) {
    serial_emergency_println!("interruption: overflow exception");
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn bound_range_exceeded_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
) {
    serial_emergency_println!("interruption: boud range exceeded interrupt");
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn invalid_opecode_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
) {
    serial_emergency_println!("interruption: invalid opecode exception");
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn device_not_available_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
) {
    serial_emergency_println!("interruption: device not available exception");
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn double_fault_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    serial_emergency_println!(
        "interruption: double fault exception, error code: 0x{:x}",
        error_code
    );
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn coprocessor_segment_overrun_interrupt_handler(
    stack_frame: InterruptStackFrame,
) {
    serial_emergency_println!("interruption: coprocessor segment overrun");
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn invalid_tss_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    serial_emergency_println!(
        "interruption: invalid tss exception, error code: 0x{:x}",
        error_code
    );
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn segment_not_present_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    serial_emergency_println!(
        "interruption: segment not present exception, error code: 0x{:x}",
        error_code
    );
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn stack_fault_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    serial_emergency_println!(
        "interruption: stack fault exception, error code: 0x{:x}",
        error_code
    );
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn general_protection_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    serial_emergency_println!(
        "interruption: page fault exception, error code: {:?}",
        error_code
    );
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn page_fault_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    serial_emergency_println!(
        "interruption: page fault exception, error code: {:?}",
        error_code
    );
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn fpu_floating_point_error_interrupt_handler(
    stack_frame: InterruptStackFrame,
) {
    serial_emergency_println!("interruption: fpu floating point error");
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn alignment_check_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    serial_emergency_println!(
        "interruption: alignment check exception, error code: 0x{:x}",
        error_code
    );
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn machine_check_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
) -> ! {
    serial_emergency_println!("interruption: machine check exception");
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn simd_floating_point_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
) {
    serial_emergency_println!("interruption: simd floating point exception");
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn virtualization_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
) {
    serial_emergency_println!("interruption: virtualization exception");
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}

#[align16_fn_for_interrupt]
extern "x86-interrupt" fn control_protection_exception_interrupt_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    serial_emergency_println!(
        "interruption: control protection exception, error code: 0x{:x}",
        error_code
    );
    dump_frame(&stack_frame);
    loop {
        unsafe {
            asm!("hlt");
        }
    }
}
