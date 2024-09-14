fn main() {
    // read env variables that were set in build script
    let uefi_path = env!("UEFI_PATH");
    let bios_path = env!("BIOS_PATH");

    // choose whether to start the UEFI or BIOS image
    let uefi = true;

    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    if uefi {
        println!("uefi bootable disk image path: {}", uefi_path);
        // println!("{:?}", ovmf_prebuilt::ovmf_pure_efi());
        cmd.arg("-enable-kvm");

        cmd.arg("-m").arg("1G");

        cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());

        // cmd.arg("-drive")
        //     .arg("if=pflash,format=raw,readonly,file=./OVMF_CODE.fd");
        // cmd.arg("-drive")
        //     .arg("if=pflash,format=raw,file=./OVMF_VARS.fd");
        cmd.arg("-drive").arg(format!(
            "if=ide,index=0,media=disk,format=raw,file={}",
            uefi_path
        ));

        // cmd.arg("-device").arg("nec-usb-xhci,id=xhci");
        cmd.arg("-device").arg("qemu-xhci,id=xhci");
        cmd.arg("-device").arg("usb-mouse");
        cmd.arg("-device").arg("usb-kbd");
        cmd.arg("-device").arg("usb-tablet");

        cmd.arg("-monitor").arg("stdio");
    } else {
        println!("bios bootable disk image path: {}", bios_path);
        cmd.arg("-drive")
            .arg(format!("format=raw,file={}", bios_path));
    }
    let mut child = cmd.spawn().unwrap();
    child.wait().unwrap();
}
