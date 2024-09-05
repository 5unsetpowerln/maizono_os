fn main() {
    // read env variables that were set in build script
    let uefi_path = env!("UEFI_PATH");
    let bios_path = env!("BIOS_PATH");

    // choose whether to start the UEFI or BIOS image
    let uefi = true;

    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    if uefi {
        println!("uefi bootable disk image path: {}", uefi_path);
        cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());
        cmd.arg("-drive")
            .arg(format!("format=raw,file={}", uefi_path));
    } else {
        println!("bios bootable disk image path: {}", bios_path);
        cmd.arg("-drive")
            .arg(format!("format=raw,file={}", bios_path));
    }
    let mut child = cmd.spawn().unwrap();
    child.wait().unwrap();
}
