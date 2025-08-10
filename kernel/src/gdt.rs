use spin::{Mutex, MutexGuard, Once};
use x86_64::{
    PrivilegeLevel::Ring0,
    registers::segmentation::{CS, DS, ES, FS, GS, SS, Segment},
    structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
};

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

    return *cs;
}

pub fn get_kernel_ss() -> SegmentSelector {
    let ss = unsafe { KERNEL_SS.get_unchecked() };

    #[cfg(feature = "init_check")]
    let ss = KERNEL_SS.get().expect("Uninitialized");

    return *ss;
}

pub fn get_user_cs() -> SegmentSelector {
    let cs = unsafe { USER_CS.get_unchecked() };

    #[cfg(feature = "init_check")]
    let cs = USER_CS.get().expect("Uninitialized");

    return *cs;
}

pub fn get_user_ss() -> SegmentSelector {
    let ss = unsafe { USER_SS.get_unchecked() };

    #[cfg(feature = "init_check")]
    let ss = USER_SS.get().expect("Uninitialized");

    return *ss;
}
