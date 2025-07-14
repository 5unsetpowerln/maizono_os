use acpi::rsdp::Rsdp;
use uefi::mem::memory_map::MemoryMapOwned;

use crate::graphic::GraphicInfo;

pub type KernelEntryPoint = extern "sysv64" fn(&BootInfo) -> !;

pub struct BootInfo {
    pub graphic_info: GraphicInfo,
    pub memory_map: MemoryMapOwned,
    pub rsdp: &'static Rsdp,
}

impl BootInfo {
    pub fn new(graphic_info: GraphicInfo, memory_map: MemoryMapOwned, rsdp: &'static Rsdp) -> Self {
        Self {
            graphic_info,
            memory_map,
            rsdp,
        }
    }
}

pub struct Kernel {
    base_addr: u64,
    entry_point_addr: u64,
    entry_point: KernelEntryPoint,
}

impl Kernel {
    pub fn new(base_addr: u64, entry_point_addr: u64) -> Self {
        Self {
            base_addr,
            entry_point_addr,
            entry_point: unsafe { core::mem::transmute(entry_point_addr) },
        }
    }

    pub fn run(self, boot_info: &BootInfo) -> ! {
        (self.entry_point)(boot_info)
    }

    pub fn base_addr(&self) -> u64 {
        self.base_addr
    }

    pub fn entry_point_addr(&self) -> u64 {
        self.entry_point_addr
    }
}
