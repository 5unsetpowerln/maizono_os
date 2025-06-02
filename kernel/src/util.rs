pub fn u32_from_slice(slice: &[u8]) -> u32 {
    let mut bytes = [0; 4];
    bytes.copy_from_slice(slice);

    u32::from_le_bytes(bytes)
}
