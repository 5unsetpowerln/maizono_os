use spin::once::Once;

#[repr(C, packed)]
pub struct BPB {
    //   uint8_t jump_boot[3];
    jump_boot: [u8; 3],
    //   char oem_name[8];
    oem_name: [u8; 8],
    //   uint16_t bytes_per_sector;
    bytes_per_sector: u16,
    //   uint8_t sectors_per_cluster;
    sectors_per_cluster: u8,
    //   uint16_t reserved_sector_count;
    reserved_sector_count: u16,
    //   uint8_t num_fats;
    num_fats: u8,
    //   uint16_t root_entry_count;
    root_entry_count: u16,
    //   uint16_t total_sectors_16;
    total_sectors_16: u16,
    //   uint8_t media;
    media: u8,
    //   uint16_t fat_size_16;
    fat_size_16: u16,
    //   uint16_t sectors_per_track;
    sectors_per_track: u16,
    //   uint16_t num_heads;
    num_heads: u16,
    //   uint32_t hidden_sectors;
    hidden_sectors: u32,
    //   uint32_t total_sectors_32;
    total_sectors_32: u32,
    //   uint32_t fat_size_32;
    fat_size_32: u32,
    //   uint16_t ext_flags;
    ext_flags: u16,
    //   uint16_t fs_version;
    fs_version: u16,
    //   uint32_t root_cluster;
    root_cluster: u32,
    //   uint16_t fs_info;
    fs_info: u16,
    //   uint16_t backup_boot_sector;
    backup_boot_sector: u16,
    //   uint8_t reserved[12];
    reserved: [u8; 12],
    //   uint8_t drive_number;
    drive_number: u8,
    //   uint8_t reserved1;
    reserved1: u8,
    //   uint8_t boot_signature;
    boot_signature: u8,
    //   uint32_t volume_id;
    volume_id: u32,
    //   char volume_label[11];
    volume_lavel: [u8; 11],
    //   char fs_type[8];
    fs_type: [u8; 8],
}

pub static BOOT_VOLUME_IMAGE: Once<&'static BPB> = Once::new();

pub fn init(image_volume: &'static [u8]) {
    let bpb_size = size_of::<BPB>();

    assert!(image_volume.len() >= bpb_size);

    let bpb_ptr = image_volume.as_ptr() as *const BPB;
    let bpb_ref = unsafe { &*bpb_ptr };

    BOOT_VOLUME_IMAGE.call_once(|| bpb_ref);
}
