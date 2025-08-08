#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "sysv64" fn _start(args: &[&str]) -> Option<u64> {
    let mut stack = [0; 100];
    let mut stack_ptr = 0;

    fn pop(stack: &mut [u64], stack_ptr: &mut usize) -> u64 {
        let value = stack[*stack_ptr];
        *stack_ptr -= 1;
        value
    }

    fn push(stack: &mut [u64], stack_ptr: &mut usize, value: u64) {
        *stack_ptr += 1;
        stack[*stack_ptr] = value;
    }

    for arg in args {
        if **arg == *"+" {
            let b = pop(&mut stack, &mut stack_ptr);
            let a = pop(&mut stack, &mut stack_ptr);
            push(&mut stack, &mut stack_ptr, a + b);
        } else if **arg == *"-" {
            let b = pop(&mut stack, &mut stack_ptr);
            let a = pop(&mut stack, &mut stack_ptr);
            push(&mut stack, &mut stack_ptr, a - b);
        } else {
            let a = match (*arg).parse::<u64>() {
                Ok(i) => i,
                Err(_) => return None,
            };

            push(&mut stack, &mut stack_ptr, a);
        }
    }

    Some(pop(&mut stack, &mut stack_ptr))
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {
        unsafe { asm!("hlt") }
    }
}
