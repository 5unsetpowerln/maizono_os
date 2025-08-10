use core::ascii::{self, Char};
use core::ptr::copy_nonoverlapping;

use alloc::collections::VecDeque;
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use core::arch::asm;
use goblin::elf;
use goblin::elf::program_header::PT_LOAD;
use pc_keyboard::{DecodedKey, KeyCode};
use spin::mutex::Mutex;
use spin::once::Once;
use x86_64::instructions::interrupts::without_interrupts;
use x86_64::structures::paging::page_table::{FrameError, PageTableEntry, PageTableLevel};
use x86_64::structures::paging::{PageTable, PageTableFlags, PageTableIndex};
use x86_64::{PhysAddr, registers};

use crate::fat::DirectoryEntry;
use crate::fat::get_root_cluster;
use crate::fat::{self, get_sector_by_cluster};
use crate::frame_manager::FrameID;
use crate::gdt::call_app;
use crate::graphic::console;
use crate::kprintln;
use crate::logger;
use crate::message;
use crate::serial_println;
use crate::task::TASK_MANAGER;
use crate::task::TaskManagerTrait;
use crate::types::VirtAddr;
use crate::{TASK_IDS, frame_manager};
use crate::{kprint, serial_print};

const HISTORY_SIZE: usize = 8;
const PROMPT: &str = "$ ";

pub struct Terminal {
    line_buffer: String,
    cursor: usize,
    display_line_buffer: String,
    displayed_count: usize,
    history: VecDeque<String>,
    history_idx: isize,
}

impl Terminal {
    pub fn new() -> Self {
        let mut history = VecDeque::new();
        history.resize(HISTORY_SIZE, String::new());

        Self {
            line_buffer: String::new(),
            cursor: 0,
            display_line_buffer: "$ ".to_string(),
            displayed_count: 0,
            history,
            history_idx: -1,
        }
    }

    pub fn input_key(&mut self, key: DecodedKey) {
        match key {
            DecodedKey::Unicode(character) => match character.as_ascii().unwrap() {
                ascii::Char::Backspace => {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                        self.line_buffer.remove(self.cursor);
                        self.display_line_buffer.push(character);
                    }
                }
                ascii::Char::LineFeed => {
                    kprintln!("");
                    self.execute_line();

                    self.history.pop_back();
                    self.history.push_front(self.line_buffer.clone());
                    self.history_idx = -1;

                    self.reset_input();
                }
                _ => {
                    self.line_buffer.insert(self.cursor, character);
                    self.cursor += 1;
                    self.display_line_buffer.push(character);
                }
            },
            DecodedKey::RawKey(key) => match key {
                KeyCode::ArrowLeft => {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                        console::move_cursor_left();
                    }
                }
                KeyCode::ArrowRight => {
                    if self.cursor < self.line_buffer.chars().count() {
                        self.cursor += 1;
                        console::move_cursor_right();
                    }
                }
                KeyCode::ArrowUp => {
                    self.history_up_or_down(1);
                }
                KeyCode::ArrowDown => {
                    self.history_up_or_down(-1);
                }
                _ => {}
            },
        }
    }

    pub fn history_up_or_down(&mut self, direction: i8) {
        let prev_history_idx = self.history_idx;

        if direction > 0
            && 0 <= self.history_idx + 1
            && self.history_idx + 1 < (HISTORY_SIZE as isize)
        {
            self.history_idx += 1;
        }

        if direction < 0 && 0 <= self.history_idx {
            self.history_idx -= 1;
        }

        if self.history_idx == -1 {
            console::clear_current_line();
            self.reset_input();
            return;
        }

        if let Some(history) = self.history.get(self.history_idx as usize) {
            let history = history.clone();

            if history.is_empty() {
                self.history_idx = prev_history_idx;
                return;
            }

            console::clear_current_line();
            self.line_buffer = history;
            self.display_line_buffer = format!("$ {}", self.line_buffer);
            self.cursor = self.line_buffer.chars().count();
            self.displayed_count = 0;
        }
    }

    pub fn display_on_console(&mut self) {
        if self.displayed_count < self.display_line_buffer.chars().count() {
            let diff = self.display_line_buffer[self.displayed_count..].to_string();
            kprint!("{}", diff);
            self.displayed_count += diff.chars().count();
        }
    }

    pub fn reset_input(&mut self) {
        self.cursor = 0;
        self.display_line_buffer = "$ ".to_string();
        self.line_buffer.clear();
        self.displayed_count = 0;
    }

    // コマンドが認識されたら true, コマンドが認識されなかったら (例えば空白だけ) false
    pub fn execute_line(&mut self) -> bool {
        let parts = self.line_buffer.as_str().split(' ').collect::<Vec<&str>>();

        let command = parts.iter().find(|&s| !s.is_empty());

        if command.is_none() {
            kprintln!("");
            return false;
        }

        let command = *command.unwrap();

        match command {
            "echo" => {
                let args_index = self.line_buffer.as_str().find(command).unwrap();
                let args = self
                    .line_buffer
                    .as_str()
                    .get(args_index + command.len()..)
                    .unwrap_or("")
                    .trim();

                kprintln!("{args}");
                kprintln!("");
            }
            "ls" => {
                for entry in fat::get_root_dir_entries() {
                    if entry.name[0] == ascii::Char::Null {
                        break;
                    } else if entry.name[0].to_u8() == 0xe5 || entry.attr.is_long_name() {
                        continue;
                    }

                    kprintln!("{}", entry.get_name());
                }

                kprintln!("");
            }
            "cat" => {
                for _ in 0..1 {
                    let args_index = self.line_buffer.as_str().find(command).unwrap();
                    let args = self
                        .line_buffer
                        .as_str()
                        .get(args_index + command.len()..)
                        .unwrap_or("")
                        .trim();

                    let file_entry = fat::find_file(args.as_ascii().unwrap(), get_root_cluster());

                    if file_entry.is_none() {
                        kprintln!("No such file: {}", args);
                        kprintln!("");
                        break;
                    }

                    let file_entry = file_entry.unwrap();

                    let mut buffer = Vec::new();

                    file_entry.read_file_to_vec(&mut buffer);

                    let string = String::from_utf8_lossy(&buffer).to_string();

                    kprintln!("{}", string);
                }
            }
            "clear" => {
                console::clear();
            }
            _ => {
                if let Some(file_entry) =
                    fat::find_file(command.as_ascii().unwrap(), fat::get_root_cluster())
                {
                    let args_index = self.line_buffer.as_str().find(command).unwrap();

                    let mut args = self
                        .line_buffer
                        .as_str()
                        .get(args_index + command.len()..)
                        .unwrap_or("")
                        .trim()
                        .split(' ')
                        .collect::<Vec<&str>>();

                    args.retain(|s| !(*s).is_empty());

                    execute_file(file_entry, &args);
                } else {
                    kprintln!("No such command: {command}");
                    kprintln!("");
                }
            }
        }

        true
    }
}

const STACK_FRAME_ADDR: VirtAddr = VirtAddr::new(0xffff_ffff_ffff_e000);
const ARGS_FRAME_ADDR: VirtAddr = VirtAddr::new(0xffff_ffff_ffff_f000);

fn execute_file(file: &DirectoryEntry, args: &[&str]) {
    let mut buffer = Vec::new();

    file.read_file_to_vec(&mut buffer);

    serial_println!("{:?}", &buffer[1..5]);
    if buffer[0..4] == [0x7f, 0x45, 0x4c, 0x46] {
        let elf = goblin::elf::Elf::parse(&buffer).expect("Failed to parse the elf.");

        load_elf(&buffer, &elf).unwrap();

        setup_page_tables(STACK_FRAME_ADDR, 1).expect("Failed to prepare stack frame.");

        setup_page_tables(ARGS_FRAME_ADDR, 1).expect("Failed to prepare args frame.");

        make_argv(args, ARGS_FRAME_ADDR);

        let argc = args.len();
        let argv = ARGS_FRAME_ADDR.as_u64() as *const *const u8;

        x86_64::instructions::tlb::flush_all();

        unsafe {
            call_app(
                argc,
                argv,
                elf.entry,
                STACK_FRAME_ADDR.as_u64() + 0x1000 - 8,
            );
        }

        let first_addr = get_first_load_address(&elf).unwrap();

        clean_page_tables(first_addr).unwrap();

        return;
    }

    let func: fn(&[&str]) = unsafe { core::mem::transmute(buffer.as_ptr()) };
    func(args);
}

fn make_argv(src: &[&str], dst: VirtAddr) {
    let buf_offset = 8 * src.len();

    let dst = dst.as_u64() as *mut u8;

    let mut written = 0;

    for (i, string) in src.iter().enumerate() {
        unsafe {
            let string_src = *string as *const str as *const u8;
            let string_dst = dst.add(buf_offset + written);

            copy_nonoverlapping(string_src, string_dst, string.len());

            let pointer_dst = dst.add(8 * i) as *mut u64;
            pointer_dst.write_volatile(string_dst as u64);
        }

        written += string.len() + 1;
    }
}

static TERMINAL: Once<Mutex<Terminal>> = Once::new();

pub fn init() {
    TERMINAL.call_once(|| Mutex::new(Terminal::new()));
}

pub fn terminal_task(task_id: u64, _data: u64) {
    without_interrupts(|| {
        *logger::CONSOLE_ENABLED.write() = false;
    });

    let draw_layer_task_id = TASK_IDS.wait().draw_layer_task_id;
    TERMINAL.wait().lock().display_on_console();

    loop {
        if let Some(message::Message::KeyInput(decoded_key)) = without_interrupts(|| {
            TASK_MANAGER
                .wait()
                .lock()
                .receive_message_from_task(task_id)
                .unwrap()
        }) {
            let mut terminal = TERMINAL.wait().lock();
            terminal.input_key(decoded_key);
            terminal.display_on_console();
        } else {
            without_interrupts(|| {
                TASK_MANAGER
                    .wait()
                    .lock()
                    .send_message_to_task(draw_layer_task_id, &message::Message::DrawLayer)
                    .unwrap();
            });

            TASK_MANAGER.wait().sleep(task_id).unwrap();
            continue;
        }

        unsafe { asm!("hlt") }
    }
}

const VIRTUAL_ADDRESS_MIN: VirtAddr = VirtAddr::new(0xffff800000000000);

fn load_elf(src: &[u8], elf: &elf::Elf) -> Result<()> {
    if elf.header.e_type != goblin::elf::header::ET_EXEC {
        return Err(Error::InvalidFormat);
    }

    let first_addr = get_first_load_address(elf).unwrap();

    if first_addr.as_u64() < VIRTUAL_ADDRESS_MIN.as_u64() {
        return Err(Error::InvalidFormat);
    }

    copy_load_segments(src, elf)?;

    Ok(())
}

fn copy_load_segments(src: &[u8], elf: &goblin::elf::Elf) -> Result<()> {
    for ph in elf.program_headers.iter() {
        if ph.p_type == PT_LOAD {
            let vaddr = ph.p_vaddr;
            let mem_size = ph.p_memsz as usize;
            let file_size = ph.p_filesz as usize;
            let offset = ph.p_offset as usize;

            let page_start = VirtAddr::new(vaddr).align_down_(0x1000);
            let page_end = VirtAddr::new(vaddr + mem_size as u64).align_up_(0x1000);
            let page_count_4k = (page_end.as_u64() - page_start.as_u64()) / 0x1000;

            setup_page_tables(page_start, page_count_4k as usize)?;

            let dst = vaddr as *mut u8;
            let src = src.as_ptr();

            unsafe {
                core::ptr::copy(src.add(offset), dst, file_size);
                dst.add(file_size).write_bytes(0, mem_size - file_size);
            }
        }
    }

    Ok(())
}

fn setup_page_tables(addr: VirtAddr, page_count_4k: usize) -> Result<()> {
    let table_4 = {
        let ptr = registers::control::Cr3::read().0.start_address().as_u64() as *mut PageTable;

        unsafe { &mut *ptr }
    };

    if setup_page_table(table_4, PageTableLevel::Four, addr, page_count_4k).is_err() {
        return Err(Error::CannotPreparePageTable);
    }

    Ok(())
}

fn setup_page_table(
    page_table: &mut PageTable,
    level: PageTableLevel,
    mut addr: VirtAddr,
    mut page_count_4k: usize,
) -> Result<Option<usize>> {
    while page_count_4k > 0 {
        let index = addr.page_table_index(level);

        let entry = &mut page_table[index];

        if let PageTableLevel::One = level {
            if !entry.flags().contains(PageTableFlags::PRESENT) {
                let phys_addr = frame_manager::alloc(1)
                    .expect("Failed to allocate a page frame.")
                    .to_addr();

                let ptr = phys_addr.as_u64() as *mut u8;

                unsafe {
                    ptr.write_bytes(0, 0x1000);
                }

                entry.set_addr(phys_addr, entry.flags());
            }

            entry.set_flags(
                entry.flags()
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::PRESENT
                    | PageTableFlags::USER_ACCESSIBLE,
            );

            page_count_4k -= 1;
        } else {
            let child_table = unsafe { set_new_page_table_if_not_present(entry) };

            entry.set_flags(
                entry.flags()
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::PRESENT
                    | PageTableFlags::USER_ACCESSIBLE,
            );

            if let Some(remain_page_count) = setup_page_table(
                unsafe { &mut *child_table },
                level.next_lower_level().unwrap(),
                addr,
                page_count_4k,
            )? {
                page_count_4k = remain_page_count;
            } else {
                return Ok(None);
            }
        }

        let paddr = entry.addr();

        if u16::from(index) as usize == page_table.iter().count() - 1 {
            break;
        }

        serial_print!("{:?}: 0x{:x} ~ ", level, addr.as_u64());

        addr.set_page_table_index(level, PageTableIndex::new(u16::from(index) + 1));

        let mut lv = level;
        while let Some(lower_level) = lv.next_lower_level() {
            addr.set_page_table_index(lower_level, PageTableIndex::new(0));

            lv = lower_level;
        }

        serial_println!("0x{:x}: 0x{:x}", addr.as_u64(), paddr.as_u64());
    }

    Ok(None)
}

/// 渡すエントリがレベル1のテーブルに属していないことをプログラマが保証してください
unsafe fn set_new_page_table_if_not_present(entry: &mut PageTableEntry) -> *mut PageTable {
    match entry.frame() {
        Ok(frame) => frame.start_address().as_u64() as *mut PageTable,
        Err(err) => match err {
            FrameError::FrameNotPresent => {
                let child_page_table = unsafe { new_page_table() };
                entry.set_addr(child_page_table, PageTableFlags::PRESENT);

                child_page_table.as_u64() as *mut PageTable
            }
            FrameError::HugeFrame => {
                panic!("Huge frame error.")
            }
        },
    }
}

unsafe fn new_page_table() -> PhysAddr {
    let addr = frame_manager::alloc(1)
        .expect("Failed to allocate a new page frame")
        .to_addr();

    let ptr = addr.as_u64() as *mut u8;

    unsafe {
        core::ptr::write_bytes(ptr, 0, frame_manager::BYTES_PER_FRAME);
    }

    addr
}

fn clean_page_tables(addr: VirtAddr) -> Result<()> {
    let table_4 = unsafe {
        let ptr = registers::control::Cr3::read().0.start_address().as_u64() as *mut PageTable;

        &mut *ptr
    };

    let table_3 = unsafe {
        let ptr = table_4[addr.p4_index()].addr().as_u64() as *mut PageTable;

        &mut *ptr
    };

    // 複数のレベル4ページテーブルを使用するほど大きなメモリを使うアプリは作らないので一つだけで良いとのこと
    table_4[addr.p4_index()].set_unused();

    clean_page_table(table_3, PageTableLevel::Three)?;

    let frame_id =
        frame_manager::FrameID::from_addr(PhysAddr::new(table_3 as *const PageTable as u64));

    frame_manager::dealloc(frame_id, 1);

    Ok(())
}

fn clean_page_table(table: &mut PageTable, level: PageTableLevel) -> Result<()> {
    for entry in table.iter_mut() {
        if !entry.flags().contains(PageTableFlags::PRESENT) {
            continue;
        }

        if let PageTableLevel::One = level {
        } else {
            let child_table = unsafe { &mut *(entry.addr().as_u64() as *mut PageTable) };

            clean_page_table(child_table, level.next_lower_level().unwrap())?;
        }

        let frame_id = FrameID::from_addr(entry.addr());

        frame_manager::dealloc(frame_id, 1);

        entry.set_unused();
    }

    Ok(())
}

fn get_first_load_address(elf: &goblin::elf::Elf) -> Option<VirtAddr> {
    for ph in elf.program_headers.iter() {
        if ph.p_type == PT_LOAD {
            return Some(VirtAddr::new(ph.p_vaddr));
        }
    }

    None
}

#[derive(Debug)]
enum Error {
    InvalidFormat,
    CannotPreparePageTable,
    VirtAddrInUse,
}

type Result<T> = core::result::Result<T, Error>;
