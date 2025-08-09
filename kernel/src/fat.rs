use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::{format, vec};
use bitflags::bitflags;
use core::ascii;
use spin::once::Once;

#[repr(C, packed)]
#[derive(Debug)]
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
        name.to_string()
    }

    pub fn is_file(&self) -> bool {
        self.attr.is_file()
    }

    pub fn read_file(&self, buf: &mut [u8]) -> usize {
        let bytes_per_cluster = get_bytes_per_cluster();

        let mut cluster = self.first_cluster();

        let mut written = 0;

        while cluster != 0 && cluster != END_OF_CLUSTER_CHAIN && cluster != BAD_CLUSTER {
            let src = get_sector_by_cluster::<u8>(cluster);

            if bytes_per_cluster > buf.len() - written {
                let src = unsafe { core::slice::from_raw_parts(src, buf.len() - written) };

                buf[written..].copy_from_slice(src);

                if buf.len() > self.file_size as usize {
                    return self.file_size as usize;
                }

                return buf.len();
            }

            let src = unsafe { core::slice::from_raw_parts(src, get_bytes_per_cluster()) };

            buf[written..].copy_from_slice(src);

            written += bytes_per_cluster;

            cluster = next_cluster(cluster);
        }

        written
    }

    pub fn read_file_to_vec(&self, buf: &mut Vec<u8>) {
        let bytes_per_cluster = get_bytes_per_cluster();

        let mut cluster = self.first_cluster();

        let mut written = 0;

        while cluster != 0 && cluster != END_OF_CLUSTER_CHAIN && cluster != BAD_CLUSTER {
            let src = get_sector_by_cluster::<u8>(cluster);

            if bytes_per_cluster > self.file_size as usize - written {
                let src =
                    unsafe { core::slice::from_raw_parts(src, self.file_size as usize - written) };
                buf.extend_from_slice(src);

                return;
            }

            let src = unsafe { core::slice::from_raw_parts(src, bytes_per_cluster) };

            buf.extend_from_slice(src);

            written += bytes_per_cluster;

            cluster = next_cluster(cluster);
        }
    }

    pub fn first_cluster(&self) -> u32 {
        self.first_cluster_low as u32 | ((self.first_cluster_high as u32) << 16)
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    pub struct Attribute: u8 {
        const READ_ONLY= 0x01;
        const HIDDEN= 0x02;
        const SYSTEM= 0x04;
        const VOLUME_ID= 0x08;
        const DIRECTORY= 0x10;
        const ARCHIVE= 0x20;
        const LONG_NAME= 0x0f;
    }
}

impl Attribute {
    pub fn is_directory(&self) -> bool {
        self.contains(Self::DIRECTORY)
    }

    pub fn is_file(&self) -> bool {
        !self.intersects(Self::DIRECTORY | Self::VOLUME_ID)
    }

    pub fn is_long_name(&self) -> bool {
        self.contains(Self::LONG_NAME)
    }
}

impl BPB {
    pub fn get_addr(&self) -> u64 {
        self as *const Self as u64
    }
}

pub static BOOT_VOLUME_IMAGE: Once<&'static BPB> = Once::new();
pub static ROOT_DIR_ENTRIES: Once<Vec<&'static DirectoryEntry>> = Once::new();
pub static BYTES_PER_CLUSTER: Once<usize> = Once::new();
pub const END_OF_CLUSTER_CHAIN: u32 = 0xfffffff;
pub const BAD_CLUSTER: u32 = 0xffffff7;

pub fn init(image_volume: &'static [u8]) {
    let bpb_size = size_of::<BPB>();

    assert!(image_volume.len() >= bpb_size);

    let boot_volume_image_ptr = image_volume.as_ptr() as *const BPB;

    let boot_volume_image = unsafe { &*boot_volume_image_ptr };

    BYTES_PER_CLUSTER.call_once(|| {
        boot_volume_image.bytes_per_sector as usize * boot_volume_image.sectors_per_cluster as usize
    });
    BOOT_VOLUME_IMAGE.call_once(|| boot_volume_image);
    ROOT_DIR_ENTRIES.call_once(get_root_dir_entries_internal);
}

pub fn get_root_dir_entries() -> &'static Vec<&'static DirectoryEntry> {
    let r = unsafe { ROOT_DIR_ENTRIES.get_unchecked() };

    #[cfg(feature = "init_check")]
    let r = ROOT_DIR_ENTRIES.get().expect("Uninitialized.");

    r
}

pub fn get_boot_volume_image() -> &'static BPB {
    let r = *unsafe { BOOT_VOLUME_IMAGE.get_unchecked() };

    #[cfg(feature = "init_check")]
    let r = *BOOT_VOLUME_IMAGE.get().expect("Uninitialized");

    r
}

pub fn get_bytes_per_cluster() -> usize {
    let r = *unsafe { BYTES_PER_CLUSTER.get_unchecked() };

    #[cfg(feature = "init_check")]
    let r = *BYTES_PER_CLUSTER.get().expect("Uninitialized");

    r
}

pub fn get_root_cluster() -> u32 {
    get_boot_volume_image().root_cluster
}

fn get_root_dir_entries_internal() -> Vec<&'static DirectoryEntry> {
    // エントリの配列が複数のクラスタにまたがっている場合に対応できていない。

    let boot_volume_image = get_boot_volume_image();

    let root_dir_entries =
        get_cluster_addr(boot_volume_image.root_cluster) as *const DirectoryEntry;

    let entries_per_sector =
        boot_volume_image.bytes_per_sector as usize / size_of::<DirectoryEntry>();
    let entries_per_cluster = entries_per_sector * boot_volume_image.sectors_per_cluster as usize;

    let mut entries = vec![];

    for i in 0..entries_per_cluster {
        let entry = unsafe { &*root_dir_entries.add(i) };
        entries.push(entry);
    }

    entries
}

pub fn find_file(name: &[ascii::Char], mut dir_cluster: u32) -> Option<&'static DirectoryEntry> {
    let boot_volume_image = get_boot_volume_image();

    if dir_cluster == 0 {
        dir_cluster = boot_volume_image.root_cluster;
    }

    while dir_cluster != END_OF_CLUSTER_CHAIN {
        let dir = get_sector_by_cluster::<DirectoryEntry>(dir_cluster);
        for i in 0..get_bytes_per_cluster() / size_of::<DirectoryEntry>() {
            if is_name_equal(unsafe { dir.add(i) }, name) {
                return Some(unsafe { &*dir.add(i) });
            }
        }

        dir_cluster = next_cluster(dir_cluster);
    }

    None
}

pub fn is_name_equal(entry: *const DirectoryEntry, name: &[ascii::Char]) -> bool {
    let entry = unsafe { &*entry };
    let mut name_8_3 = [ascii::Char::Space; 11];

    let mut i_8_3 = 0;

    for char in name {
        if i_8_3 >= name_8_3.len() {
            break;
        }

        if *char == ascii::Char::FullStop {
            i_8_3 = 8;
            continue;
        }

        name_8_3[i_8_3] = *char;
        i_8_3 += 1;
    }

    entry.name[..] == name_8_3[..]
}

pub fn next_cluster(cluster: u32) -> u32 {
    let boot_volume_image = get_boot_volume_image();

    let fat_offset = boot_volume_image.reserved_sector_count * boot_volume_image.bytes_per_sector;

    // fat: file allocation table
    let fat = (boot_volume_image.get_addr() + fat_offset as u64) as *const u32;

    let next = unsafe { *fat.add(cluster as usize) };

    if next >= 0xffffff8 {
        return END_OF_CLUSTER_CHAIN;
    } else if next == 0xffffff7 {
        return BAD_CLUSTER;
    }

    next
}

pub fn get_sector_by_cluster<T>(cluster: u32) -> *const T {
    get_cluster_addr(cluster) as *const T
}

pub fn get_bytes_by_cluster(cluster: u32) -> &'static [u8] {
    let ptr = get_cluster_addr(cluster) as *const u8;

    unsafe { core::slice::from_raw_parts(ptr, get_bytes_per_cluster()) }
}

pub fn get_cluster_addr(cluster: u32) -> u64 {
    let boot_volume_image = get_boot_volume_image();

    let sector_num = boot_volume_image.reserved_sector_count as u64
        + boot_volume_image.num_fats as u64 * boot_volume_image.fat_size_32 as u64
        + (cluster - 2) as u64 * boot_volume_image.sectors_per_cluster as u64;

    let offset = sector_num * boot_volume_image.bytes_per_sector as u64;

    boot_volume_image.get_addr() + offset
}
