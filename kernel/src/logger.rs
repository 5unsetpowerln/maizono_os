use alloc::string::ToString;
use alloc::{format, string::String};
use log::LevelFilter;

use crate::{graphic::console, kprintln, serial_println};

pub struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        // metadata.level() >= log::Level::Debug
        true
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let level_msg = format!(
                "[{}{}]:  ",
                " ".repeat(5 - record.level().as_str().chars().count()),
                record.level()
            );

            let file_msg = if let Some(s) = record.file() {
                let l = record.line().unwrap();
                format!("{}@{}: ", s, l)
            } else {
                "???@???: ".to_string()
            };

            let content_msg = format!("{}", record.args());
            let content_msg = content_msg.replace(
                "\n",
                &format!(
                    "\n{}",
                    " ".repeat(level_msg.chars().count() + file_msg.chars().count())
                ),
            );
            let msg = format!("{}{}{}", level_msg, file_msg, content_msg);

            serial_println!("{}", msg);
            if console::is_initialized() {
                kprintln!("{}", msg);
            }
        }
    }

    fn flush(&self) {}
}

static LOGGER: Logger = Logger;

pub fn init() {
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(LevelFilter::Debug);
    }
}
