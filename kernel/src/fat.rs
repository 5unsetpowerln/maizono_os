use alloc::format;
use alloc::string::{String, ToString};
use core::ascii;
use spin::once::Once;

#[repr(C, packed)]
pub struct BPB {
    //   uint8_t jump_boot[3];
    pub jump_boot: [u8; 3],
    //   char oem_name[8];
    pub oem_name: [u8; 8],
    //   uint16_t bytes_per_sector;
    pub bytes_per_sector: u16,
    //   uint8_t sectors_per_cluster;
    pub sectors_per_cluster: u8,
    //   uint16_t reserved_sector_count;
    pub reserved_sector_count: u16,
    //   uint8_t num_fats;
    pub num_fats: u8,
    //   uint16_t root_entry_count;
    pub root_entry_count: u16,
    //   uint16_t total_sectors_16;
    pub total_sectors_16: u16,
    //   uint8_t media;
    pub media: u8,
    //   uint16_t fat_size_16;
    pub fat_size_16: u16,
    //   uint16_t sectors_per_track;
    pub sectors_per_track: u16,
    //   uint16_t num_heads;
    pub num_heads: u16,
    //   uint32_t hidden_sectors;
    pub hidden_sectors: u32,
    //   uint32_t total_sectors_32;
    pub total_sectors_32: u32,
    //   uint32_t fat_size_32;
    pub fat_size_32: u32,
    //   uint16_t ext_flags;
    pub ext_flags: u16,
    //   uint16_t fs_version;
    pub fs_version: u16,
    //   uint32_t root_cluster;
    pub root_cluster: u32,
    //   uint16_t fs_info;
    pub fs_info: u16,
    //   uint16_t backup_boot_sector;
    pub backup_boot_sector: u16,
    //   uint8_t reserved[12];
    pub reserved: [u8; 12],
    //   uint8_t drive_number;
    pub drive_number: u8,
    //   uint8_t reserved1;
    pub reserved1: u8,
    //   uint8_t boot_signature;
    pub boot_signature: u8,
    //   uint32_t volume_id;
    pub volume_id: u32,
    //   char volume_label[11];
    pub volume_lavel: [u8; 11],
    //   char fs_type[8];
    pub fs_type: [u8; 8],
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct DirectoryEntry {
    pub name: [ascii::Char; 11],
    pub attr: Attribute,
    pub ntres: u8,
    pub create_time_tenth: u8,
    pub create_time: u16,
    pub create_date: u16,
    pub last_access_date: u16,
    pub first_cluster_high: u16,
    pub write_time: u16,
    pub write_date: u16,
    pub first_cluster_low: u16,
    pub file_size: u32,
}

impl DirectoryEntry {
    pub fn get_name(&self) -> String {
        if self.name[8] != ascii::Char::Space {
            let name = self.name[0..8].as_str().trim();
            let ext = self.name[8..].as_str().trim();
            return format!("{name}.{ext}");
        }

        let name = self.name[0..8].as_str().trim();
        return name.to_string();
    }
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
pub enum Attribute {
    ReadOnly = 0x01,
    Hidden = 0x02,
    System = 0x04,
    VolumeID = 0x08,
    Directory = 0x10,
    Archive = 0x20,
    LongName = 0x0f,
}

impl DirectoryEntry {
    pub fn first_cluster(&self) -> u32 {
        self.first_cluster_low as u32 | ((self.first_cluster_high as u32) << 16)
    }
}

impl BPB {
    fn get_addr(&self) -> u64 {
        self as *const Self as u64
    }
}

pub static BOOT_VOLUME_IMAGE: Once<&'static BPB> = Once::new();

pub fn init(image_volume: &'static [u8]) {
    let bpb_size = size_of::<BPB>();

    assert!(image_volume.len() >= bpb_size);

    let bpb_ptr = image_volume.as_ptr() as *const BPB;

    let bpb_ref = unsafe { &*bpb_ptr };

    BOOT_VOLUME_IMAGE.call_once(|| bpb_ref);
}

// pub fn get_root_dir_entries() {
//     let boot_volume_image = get_boot_volume_image();

//     let ptr = get_cluster_addr(boot_volume_image.root_cluster as u64) as *const DirectoryEntry;

//     let entries_per_cluster = (boot_volume_image.bytes_per_sector as usize
//         / size_of::<DirectoryEntry>())
//         * boot_volume_image.sectors_per_cluster as usize;

//     for i in 0..entries_per_cluster {
//         let entry = unsafe { &*ptr.add(i) };
//     }
// }

pub fn get_sector_by_cluster<T>(cluster: u64) -> &'static T {
    let ptr = get_cluster_addr(cluster) as *const T;
    unsafe { &*ptr }
}

pub fn get_cluster_addr(cluster: u64) -> u64 {
    let boot_volume_image = get_boot_volume_image();

    let sector_num = boot_volume_image.reserved_sector_count as u64
        + boot_volume_image.num_fats as u64 * boot_volume_image.fat_size_32 as u64
        + (cluster - 2) * boot_volume_image.sectors_per_cluster as u64;

    let offset = sector_num * boot_volume_image.bytes_per_sector as u64;

    boot_volume_image.get_addr() + offset
}

pub fn get_boot_volume_image() -> &'static BPB {
    let r = *unsafe { BOOT_VOLUME_IMAGE.get_unchecked() };

    #[cfg(feature = "init_check")]
    let r = *BOOT_VOLUME_IMAGE.get().expect("uninitialized");

    r
}
