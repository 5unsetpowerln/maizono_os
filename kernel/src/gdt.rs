use core::{
    arch::asm,
    arch::naked_asm,
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    ptr::{addr_of, addr_of_mut},
};

use spin::Once;
use x86_64::{
    PrivilegeLevel::Ring0,
    VirtAddr,
    registers::segmentation::{CS, DS, ES, FS, GS, SS, Segment},
    structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
};

use crate::{frame_manager, serial_emergency_println};

static mut GDT: GlobalDescTable = GlobalDescTable::new();
static mut TSS: TaskStateSegment = TaskStateSegment::new();
static KERNEL_CS: Once<SegmentSelector> = Once::new();
static KERNEL_SS: Once<SegmentSelector> = Once::new();
static USER_CS: Once<SegmentSelector> = Once::new();
static USER_SS: Once<SegmentSelector> = Once::new();

// Global Descriptor Table
struct GlobalDescTable {
    table: UnsafeCell<x86_64::structures::gdt::GlobalDescriptorTable>,
}

impl GlobalDescTable {
    pub const fn new() -> Self {
        Self {
            table: UnsafeCell::new(GlobalDescriptorTable::new()),
        }
    }
}

impl Deref for GlobalDescTable {
    type Target = GlobalDescriptorTable;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.table.get() }
    }
}
impl DerefMut for GlobalDescTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *self.table.get_mut()
    }
}

unsafe impl Sync for GlobalDescTable {}

// TSS
struct TaskStateSegment {
    tss: UnsafeCell<x86_64::structures::tss::TaskStateSegment>,
}

unsafe impl Sync for TaskStateSegment {}

impl TaskStateSegment {
    const fn new() -> Self {
        Self {
            tss: UnsafeCell::new(x86_64::structures::tss::TaskStateSegment::new()),
        }
    }

    fn get_ptr(&self) -> *const x86_64::structures::tss::TaskStateSegment {
        self.tss.get()
    }
}

impl Deref for TaskStateSegment {
    type Target = x86_64::structures::tss::TaskStateSegment;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.tss.get() }
    }
}

impl DerefMut for TaskStateSegment {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.tss.get_mut()
    }
}

pub fn init() {
    let gdt = unsafe { &mut *addr_of_mut!(GDT) };

    let kernel_cs = gdt.append(Descriptor::kernel_code_segment());
    let kernel_ss = gdt.append(Descriptor::kernel_data_segment());
    let user_cs = gdt.append(Descriptor::user_code_segment());
    let user_ss = gdt.append(Descriptor::user_data_segment());

    // init TSS
    let tss_tr = unsafe {
        let tss = &mut *addr_of_mut!(TSS);
        let stack = frame_manager::alloc(8)
            .expect("Failed to allocate stack for TSS")
            .to_addr();
        // 割り込みハンドラでスタックのアラインメントを0x10にするために-8しておく
        let rsp = stack.as_u64() + 0x1000 * 8 - 8;
        serial_emergency_println!("TSS.RSP0: 0x{:x}", rsp);
        tss.privilege_stack_table[0] = VirtAddr::new(rsp);
        gdt.append(Descriptor::tss_segment_unchecked(tss.get_ptr()))
    };

    unsafe {
        gdt.load_unsafe();

        DS::set_reg(SegmentSelector::new(0, Ring0));
        ES::set_reg(SegmentSelector::new(0, Ring0));
        FS::set_reg(SegmentSelector::new(0, Ring0));
        GS::set_reg(SegmentSelector::new(0, Ring0));

        CS::set_reg(kernel_cs);
        SS::set_reg(kernel_ss);

        load_tr(tss_tr);
    }

    KERNEL_CS.call_once(|| kernel_cs);
    KERNEL_SS.call_once(|| kernel_ss);
    USER_CS.call_once(|| user_cs);
    USER_SS.call_once(|| user_ss);
}

unsafe fn load_tr(tr: SegmentSelector) {
    unsafe {
        asm!("ltr di", in("di") tr.0);
    }
}

pub fn get_kernel_cs() -> SegmentSelector {
    debug_assert!(KERNEL_CS.is_completed());
    unsafe { *KERNEL_CS.get_unchecked() }
}

pub fn get_kernel_ss() -> SegmentSelector {
    debug_assert!(KERNEL_SS.is_completed());
    unsafe { *KERNEL_SS.get_unchecked() }
}

pub fn get_user_cs() -> SegmentSelector {
    debug_assert!(USER_CS.is_completed());
    unsafe { *USER_CS.get_unchecked() }
}

pub fn get_user_ss() -> SegmentSelector {
    debug_assert!(USER_SS.is_completed());
    unsafe { *USER_SS.get_unchecked() }
}

pub unsafe fn call_app(argc: usize, argv: *const *const u8, rip: u64, rsp: u64) {
    let cs = get_user_cs().0 as u64;
    let ss = get_user_ss().0 as u64;
    let rflags = x86_64::registers::rflags::read_raw();

    unsafe {
        call_app_inner(argc, argv, cs, ss, rflags, rip, rsp);
    }
}

#[naked]
unsafe extern "C" fn call_app_inner(
    argc: usize,
    argv: *const *const u8,
    cs: u64,
    ss: u64,
    rflags: u64,
    rip: u64,
    rsp: u64,
) {
    unsafe {
        naked_asm!(
            "push rbp",
            "mov rbp, rsp",
            "mov r10, [rbp + 16]",
            "push rcx", // ss
            "push r10", // rsp
            "push r8",  // rflags
            "push rdx", // cs
            "push r9",  // rip
            "iretq"
        )
    }
}
