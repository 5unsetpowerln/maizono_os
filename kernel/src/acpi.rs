use uefi::mem::memory_map::{MemoryMap, MemoryMapOwned};

const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";
const MADT_SIGNATURE: &[u8; 4] = b"APIC";

// #[repr(C, packed)]
// struct RsdpDescriptor {
//     signature: [u8; 8],
//     checksum: u8,
//     oem_id: [u8; 6],
//     revision: u8,
//     rsdt_address: u32,
// }

// #[repr(C, packed)]
// struct Rsdt {
//     signature: [u8; 4],
//     length: u32,
// }

// pub unsafe fn find_rsdp(memmap: &MemoryMapOwned) {
//     for entry in memmap.entries() {
//         entry.phys_start
//     }
// }
