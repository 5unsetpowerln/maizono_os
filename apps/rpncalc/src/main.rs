#![no_std]
#![no_main]

use core::arch::asm;
use core::panic::PanicInfo;

#[unsafe(no_mangle)]
pub extern "sysv64" fn _start(argc: usize, argv: *const *const u8) -> Option<i64> {
    let args = unsafe { core::slice::from_raw_parts(argv as *const &str, argc) };

    main(args)
}

fn main(args: &[&str]) -> Option<i64> {
    loop {}

    let mut stack = [0; 100];
    let mut stack_ptr = 0;

    fn pop(stack: &mut [i64], stack_ptr: &mut usize) -> i64 {
        let value = stack[*stack_ptr];
        *stack_ptr -= 1;
        value
    }

    fn push(stack: &mut [i64], stack_ptr: &mut usize, value: i64) {
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
            let a = match (*arg).parse::<i64>() {
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
