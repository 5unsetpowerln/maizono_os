use core::fmt::Write;

use crate::mutex::Mutex;
use spin::Lazy;
use uart_16550::SerialPort;
use x86_64::instructions::port::Port;

pub static SERIAL1: Lazy<Mutex<SerialPort>> = Lazy::new(|| {
    let mut serial_port = unsafe { SerialPort::new(0x3f8) };
    serial_port.init();
    Mutex::new(serial_port)
});

pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL1
        .lock()
        .write_fmt(args)
        .expect("Printing to serial failed");
}

struct EmergencySerial {}

impl EmergencySerial {
    #[inline(always)]
    fn write_byte(&mut self, byte: u8) {
        const COM1: u16 = 0x3f8;
        const LSR: u16 = COM1 + 5;
        const THRE: u8 = 1 << 5;

        unsafe {
            let mut data = Port::<u8>::new(COM1);
            let mut lsr = Port::<u8>::new(LSR);

            while (lsr.read() & THRE) == 0 {
                core::hint::spin_loop();
            }

            data.write(byte);
        }
    }
}

impl core::fmt::Write for EmergencySerial {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &b in s.as_bytes() {
            if b == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(b);
        }
        Ok(())
    }
}

pub fn emergency_print(args: ::core::fmt::Arguments) {
    let mut s = EmergencySerial {};
    s.write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! serial_emergency_print {
    ($($arg:tt)*) => {
        $crate::serial::emergency_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! serial_emergency_println {
    () => ($crate::serial_emergency_print!("\n"));
    ($fmt:expr) => ($crate::serial_emergency_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_emergency_print!(concat!($fmt, "\n"), $($arg)*));
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(concat!($fmt, "\n"), $($arg)*));
}
