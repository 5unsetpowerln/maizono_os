use core::arch::asm;

use x86_64::{
    registers::segmentation::{Segment, CS, DS, ES, FS, GS, SS},
    structures::gdt::SegmentSelector,
    PrivilegeLevel::Ring0,
};

type Gdt = [SegmentDescriptor; 3];
static mut GDT: Gdt = [SegmentDescriptor::new(); 3];

pub fn init() {
    unsafe {
        GDT[0].0 = 0;
        GDT[1].set_code_segment(DescriptorType::ExecuteRead, 0, 0, 0xfffff);
        GDT[2].set_data_segment(DescriptorType::ReadWrite, 0, 0, 0xfffff);

        load_gdt(size_of::<Gdt>() as u16 - 1, GDT.as_ptr() as u64);

        DS::set_reg(SegmentSelector::new(0, Ring0));
        ES::set_reg(SegmentSelector::new(0, Ring0));
        FS::set_reg(SegmentSelector::new(0, Ring0));
        GS::set_reg(SegmentSelector::new(0, Ring0));

        CS::set_reg(SegmentSelector::new(1, Ring0));
        SS::set_reg(SegmentSelector::new(2, Ring0));
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct SegmentDescriptor(u64);

impl SegmentDescriptor {
    #[inline]
    pub const fn new() -> Self {
        Self(0)
    }

    #[inline]
    fn set_data_segment(
        &mut self,
        descriptor_type: DescriptorType,
        descriptor_privilege_level: u8,
        base: u32,
        limit: u32,
    ) {
        self.set_code_segment(descriptor_type, descriptor_privilege_level, base, limit);
        self.set_long_mode(true);
        self.set_default_operation_size(true);
    }

    #[inline]
    fn set_code_segment(
        &mut self,
        descriptor_type: DescriptorType,
        descriptor_privilege_level: u8,
        base: u32,
        limit: u32,
    ) {
        self.0 = 0;

        self.set_base_low((base & 0xffff) as u16);
        self.set_base_middle(((base >> 16) & 0xff) as u8);
        self.set_base_high(((base >> 24) & 0xff) as u8);

        self.set_limit_low((limit & 0xffff) as u16);
        self.set_limit_high(((limit >> 16) & 0xff) as u8);

        self.set_descriptor_type(descriptor_type);
        self.set_system_segment(true);
        self.set_descriptor_privilege_level(descriptor_privilege_level);
        self.set_present(true);
        self.set_available(false);
        self.set_long_mode(true);
        self.set_default_operation_size(false);
        self.granularity(true);
    }

    #[allow(dead_code)]
    #[inline]
    fn set_limit_low(&mut self, value: u16) {
        self.0 = (self.0 & !0xffff) | (value as u64);
    }

    #[allow(dead_code)]
    #[inline]
    fn set_base_low(&mut self, value: u16) {
        self.0 = (self.0 & !(0xffff << 16)) | ((value as u64) << 16)
    }

    #[allow(dead_code)]
    #[inline]
    fn set_base_middle(&mut self, value: u8) {
        self.0 = (self.0 & !(0xff << 32)) | ((value as u64) << 32)
    }

    #[allow(dead_code)]
    #[inline]
    fn set_descriptor_type(&mut self, value: DescriptorType) {
        self.0 = (self.0 & !(0xf << 40)) | ((value.get() as u64) << 40)
    }

    #[allow(dead_code)]
    #[inline]
    fn set_system_segment(&mut self, value: bool) {
        self.0 = (self.0 & !(0b1 << 44))
            | if value {
                (0b1 as u64) << 44
            } else {
                (0b0 as u64) << 44
            }
    }

    #[allow(dead_code)]
    #[inline]
    fn set_descriptor_privilege_level(&mut self, value: u8) {
        self.0 = (self.0 & !(0b11 << 45)) | ((value as u64) << 45)
    }

    #[allow(dead_code)]
    #[inline]
    fn set_present(&mut self, value: bool) {
        self.0 = (self.0 & !(0b1 << 47))
            | if value {
                (0b1 as u64) << 47
            } else {
                (0b0 as u64) << 47
            }
    }

    #[allow(dead_code)]
    #[inline]
    fn set_limit_high(&mut self, value: u8) {
        self.0 = (self.0 & !(0xf << 48)) | ((value as u64) << 48)
    }

    #[allow(dead_code)]
    #[inline]
    fn set_available(&mut self, value: bool) {
        self.0 = (self.0 & !(0b1 << 52))
            | if value {
                (0b1 as u64) << 52
            } else {
                (0b0 as u64) << 52
            }
    }

    #[allow(dead_code)]
    #[inline]
    fn set_long_mode(&mut self, value: bool) {
        self.0 = (self.0 & !(0b1 << 53))
            | if value {
                (0b1 as u64) << 53
            } else {
                (0b0 as u64) << 53
            }
    }

    #[allow(dead_code)]
    #[inline]
    pub fn set_default_operation_size(&mut self, value: bool) {
        self.0 = (self.0 & !(0b1 << 54))
            | if value {
                (0b1 as u64) << 54
            } else {
                (0b0 as u64) << 54
            }
    }

    #[allow(dead_code)]
    #[inline]
    fn granularity(&mut self, value: bool) {
        self.0 = (self.0 & !(0b1 << 55))
            | if value {
                (0b1 as u64) << 55
            } else {
                (0b0 as u64) << 55
            }
    }

    #[allow(dead_code)]
    #[inline]
    fn set_base_high(&mut self, value: u8) {
        self.0 = (self.0 & !(0xff << 56)) | (value as u64) << 56
    }
}

#[allow(dead_code)]
pub enum DescriptorType {
    // system segment & gate descriptor types
    Upper8Bytes,   // 0
    LDT,           // 2
    TSSAvailable,  // 9
    TSSBusy,       // 11
    CallGate,      // 12
    InterruptGate, // 14
    TrapGate,      // 15

    // code & data segment types
    ReadWrite,   // 2
    ExecuteRead, // 10
}

impl DescriptorType {
    pub fn get(&self) -> u8 {
        match self {
            Self::Upper8Bytes => 0,
            Self::LDT => 2,
            Self::TSSAvailable => 9,
            Self::TSSBusy => 11,
            Self::CallGate => 12,
            Self::InterruptGate => 14,
            Self::TrapGate => 15,
            Self::ReadWrite => 2,
            Self::ExecuteRead => 10,
        }
    }
}

#[allow(dead_code)]
#[repr(C, packed)]
struct GlobalDescriptorTableArgs {
    limit: u16,
    offset: u64,
}

impl GlobalDescriptorTableArgs {
    fn as_ptr(&self) -> *const Self {
        self as *const Self
    }
}

#[allow(dead_code)]
unsafe fn load_gdt(limit: u16, offset: u64) {
    let lgdt_arg = GlobalDescriptorTableArgs { limit, offset };
    unsafe {
        asm!(
            "lgdt [{}]",
            in(reg) lgdt_arg.as_ptr() as u64
        );
    }
}
