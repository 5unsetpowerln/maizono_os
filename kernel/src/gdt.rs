use core::arch::naked_asm;

use spin::Once;
use x86_64::{
    PrivilegeLevel::Ring0,
    registers::segmentation::{CS, DS, ES, FS, GS, SS, Segment},
    structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
};

use crate::mutex::Mutex;

static GDT: Mutex<GlobalDescriptorTable> = Mutex::new(GlobalDescriptorTable::new());
static KERNEL_CS: Once<SegmentSelector> = Once::new();
static KERNEL_SS: Once<SegmentSelector> = Once::new();
static USER_CS: Once<SegmentSelector> = Once::new();
static USER_SS: Once<SegmentSelector> = Once::new();

pub fn init() {
    let mut gdt = GDT.lock();

    let kernel_cs = gdt.append(Descriptor::kernel_code_segment());
    let kernel_ss = gdt.append(Descriptor::kernel_data_segment());
    let user_cs = gdt.append(Descriptor::user_code_segment());
    let user_ss = gdt.append(Descriptor::user_data_segment());

    unsafe {
        gdt.load_unsafe();

        DS::set_reg(SegmentSelector::new(0, Ring0));
        ES::set_reg(SegmentSelector::new(0, Ring0));
        FS::set_reg(SegmentSelector::new(0, Ring0));
        GS::set_reg(SegmentSelector::new(0, Ring0));

        CS::set_reg(kernel_cs);
        SS::set_reg(kernel_ss);
    }

    KERNEL_CS.call_once(|| kernel_cs);
    KERNEL_SS.call_once(|| kernel_ss);
    USER_CS.call_once(|| user_cs);
    USER_SS.call_once(|| user_ss);
}

pub fn get_kernel_cs() -> SegmentSelector {
    let cs = unsafe { KERNEL_CS.get_unchecked() };

    #[cfg(feature = "init_check")]
    let cs = KERNEL_CS.get().expect("Uninitialized");

    *cs
}

pub fn get_kernel_ss() -> SegmentSelector {
    let ss = unsafe { KERNEL_SS.get_unchecked() };

    #[cfg(feature = "init_check")]
    let ss = KERNEL_SS.get().expect("Uninitialized");

    *ss
}

pub fn get_user_cs() -> SegmentSelector {
    let cs = unsafe { USER_CS.get_unchecked() };

    #[cfg(feature = "init_check")]
    let cs = USER_CS.get().expect("Uninitialized");

    *cs
}

pub fn get_user_ss() -> SegmentSelector {
    let ss = unsafe { USER_SS.get_unchecked() };

    #[cfg(feature = "init_check")]
    let ss = USER_SS.get().expect("Uninitialized");

    *ss
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
            "push rcx", // ss
            "push r10", // rsp
            "push r8",  // rflags
            "push rdx", // cs
            "push r9",  // rip
            "iretq"
        )
    }
}
