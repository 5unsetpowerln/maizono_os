use core::ops::Deref;

use uefi::mem::memory_map::MemoryMapOwned;

use crate::graphic::GraphicInfo;

pub struct BootInfo {
    pub graphic_info: GraphicInfo,
    pub memory_map: MemoryMapOwned,
    pub rsdp_addr: Option<u64>,
}

impl BootInfo {
    pub fn new(
        graphic_info: GraphicInfo,
        memory_map: MemoryMapOwned,
        rsdp_addr: Option<u64>,
    ) -> Self {
        Self {
            graphic_info,
            memory_map,
            rsdp_addr,
        }
    }
}

// pub struct MemoryMap(MemoryMapOwned);

// impl MemoryMap {
//     pub fn new(memmap: MemoryMapOwned) -> Self {
//         Self(memmap)
//     }
// }

// impl Deref for MemoryMap {
//     type Target = MemoryMapOwned;

//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl From<MemoryMapOwned> for MemoryMap {
//     fn from(value: MemoryMapOwned) -> Self {
//         Self(value)
//     }
// }

pub struct Kernel {
    base_addr: u64,
    entry_point_addr: u64,
    entry_point: extern "sysv64" fn(&BootInfo) -> !,
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
