use core::arch::naked_asm;

use crate::gdt::{KERNEL_CS, USER_CS, USER_SS};
use crate::serial_println;
use crate::x64::{IA32_EFER, IA32_FMASK, IA32_LSTAR, IA32_STAR, read_msr, write_msr};

type SyscallFunc = extern "C" fn(u64, u64, u64, u64, u64, u64) -> i64;

static mut SYSCALL_TABLE: [SyscallFunc; 1] = [serial_write];

pub fn init() {
    unsafe {
        // syscallを有効化
        write_msr(IA32_EFER, 0x0501);

        // syscallで呼び出される関数を登録
        let syscall_entry_addr = syscall_entry as *const extern "sysv64" fn() as u64;
        write_msr(IA32_LSTAR, syscall_entry_addr);

        // syscall/sysret時のCS/SSの値を登録
        let kernel_cs = KERNEL_CS.wait().0 as u64 & 0xfff8;
        let user_cs = USER_CS.wait().0 as u64 & 0xfff8;
        write_msr(IA32_STAR, (kernel_cs << 32) | ((user_cs - 16) << 48));
        // syscall時にはCSにIA32_STAR[32:32+16]、SSにIA32_STAR[32:32+16]+8が設定され、
        // sysret時にはCSにIA32_STAR[48:48+16]+16、SSにIA32_STAR[48+16]+8が設定される (添字はビット単位)
        // したがって、GDTへのCS/SSの登録順は以下のようになっている必要がある
        // ...
        // KERNEL_CS
        // KERNEL_SS
        // ...
        // USER_SS
        // USER_CS
        // ...

        write_msr(IA32_FMASK, 0);
    }
}

extern "C" fn serial_write(
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    arg5: u64,
    arg6: u64,
) -> i64 {
    // arg1: buffer
    // arg2: size

    let raw_buffer = arg1 as *const core::ascii::Char;
    let size = arg2 as usize;

    if size > 0x1000 {
        return -1;
    }

    let buffer = unsafe { core::slice::from_raw_parts(raw_buffer, size) };
    serial_println!("{}", buffer.as_str());
    0
}

#[naked]
extern "sysv64" fn syscall_entry() {
    unsafe {
        naked_asm!(
            "push rbp",
            "push rcx",     // original rip
            "push r11",     // original rflags
            "mov rcx, r10", // 4th arg
            "and eax, 0x7fffffff",
            "mov rbp, rsp",
            "and rsp, 0xfffffffffffffff0",
            "call [{syscall_table} + 8 * eax]",
            "mov rsp, rbp",
            "pop r11",
            "pop rcx",
            "pop rbp",
            "sysretq",
            syscall_table = sym SYSCALL_TABLE
        )
    }
}
