use core::arch::naked_asm;
use core::ptr::copy_nonoverlapping;

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use log::debug;
use num_enum::TryFromPrimitive;
use slotmap::{DefaultKey, SlotMap};
use spin::Once;

use crate::gdt::{get_kernel_cs, get_kernel_ss};
use crate::message::Message;
use crate::mutex::{Mutex, MutexGuard};
use crate::timer::{self, TIMER_FREQ, Timer, TimerKind};
use crate::util::read_cr3_raw;

pub const TASK_TIMER_PERIOD: u64 = (TIMER_FREQ as u64 / 100) * 2;
pub static TASK_MANAGER: Once<Mutex<TaskManager>> = Once::new();

#[repr(i8)]
#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Copy, Clone, TryFromPrimitive)]
pub enum PriorityLevel {
    Level0 = 0,
    Level1 = 1,
    Level2 = 2,
    Level3 = 3,
    Unchanged = -1,
}

pub fn init() {
    TASK_MANAGER.call_once(|| Mutex::new(TaskManager::new()));

    let mut timer_manager = timer::TIMER_MANAGER.lock();
    let current_tick = timer_manager.get_current_tick();
    timer_manager.add_timer(Timer::new(
        TASK_TIMER_PERIOD + current_tick,
        TimerKind::PreemptiveMultitask,
    ));
}

#[naked]
pub unsafe extern "C" fn restore_context(ctx: *const TaskContext) {
    unsafe {
        naked_asm!(
            "push qword ptr [rdi + 0x28]", // SS
            "push qword ptr [rdi + 0x70]", // RSP
            "push qword ptr [rdi + 0x10]", // RFLAGS
            "push qword ptr [rdi + 0x20]", // CS
            "push qword ptr [rdi + 0x08]", // RIP
            // コンテキストの復帰
            "fxrstor [rdi + 0xc0]",
            "mov rax, [rdi + 0x00]",
            "mov cr3, rax",
            "mov rax, [rdi + 0x30]",
            "mov fs, ax",
            "mov rax, [rdi + 0x38]",
            "mov gs, ax",
            "mov rax, [rdi + 0x40]",
            "mov rbx, [rdi + 0x48]",
            "mov rcx, [rdi + 0x50]",
            "mov rdx, [rdi + 0x58]",
            "mov rsi, [rdi + 0x68]",
            "mov rbp, [rdi + 0x78]",
            "mov r8,  [rdi + 0x80]",
            "mov r9,  [rdi + 0x88]",
            "mov r10, [rdi + 0x90]",
            "mov r11, [rdi + 0x98]",
            "mov r12, [rdi + 0xa0]",
            "mov r13, [rdi + 0xa8]",
            "mov r14, [rdi + 0xb0]",
            "mov r15, [rdi + 0xb8]",
            "mov rdi, [rdi + 0x60]",
            "iretq",
        )
    }
}

#[naked]
pub unsafe extern "C" fn switch_context(
    next_ctx: *const TaskContext,
    current_ctx: *mut TaskContext,
) {
    unsafe {
        naked_asm!(
            "mov [rsi + 0x40], rax",
            "mov [rsi + 0x48], rbx",
            "mov [rsi + 0x50], rcx",
            "mov [rsi + 0x58], rdx",
            "mov [rsi + 0x60], rdi",
            "mov [rsi + 0x68], rsi",
            "lea rax, [rsp + 8]",
            "mov [rsi + 0x70], rax", // RSP
            "mov [rsi + 0x78], rbp",
            "mov [rsi + 0x80], r8",
            "mov [rsi + 0x88], r9",
            "mov [rsi + 0x90], r10",
            "mov [rsi + 0x98], r11",
            "mov [rsi + 0xa0], r12",
            "mov [rsi + 0xa8], r13",
            "mov [rsi + 0xb0], r14",
            "mov [rsi + 0xb8], r15",
            "mov rax, cr3",
            "mov [rsi + 0x00], rax", // CR3
            "mov rax, [rsp]",
            "mov [rsi + 0x08], rax", // RIP
            "pushfq",
            "pop qword ptr [rsi + 0x10]", // RFLAGS
            "xor rax, rax",
            "mov ax, cs",
            "mov [rsi + 0x20], rax",
            "mov ax, ss",
            "mov [rsi + 0x28], rax",
            "mov ax, fs",
            "mov [rsi + 0x30], rax",
            "mov ax, gs",
            "mov [rsi + 0x38], rax",
            "fxsave [rsi + 0xc0]",
            // iret 用のスタックフレーム
            "push qword ptr [rdi + 0x28]", // SS
            "push qword ptr [rdi + 0x70]", // RSP
            "push qword ptr [rdi + 0x10]", // RFLAGS
            "push qword ptr [rdi + 0x20]", // CS
            "push qword ptr [rdi + 0x08]", // RIP
            // コンテキストの復帰
            "fxrstor [rdi + 0xc0]",
            "mov rax, [rdi + 0x00]",
            "mov cr3, rax",
            "mov rax, [rdi + 0x30]",
            "mov fs, ax",
            "mov rax, [rdi + 0x38]",
            "mov gs, ax",
            "mov rax, [rdi + 0x40]",
            "mov rbx, [rdi + 0x48]",
            "mov rcx, [rdi + 0x50]",
            "mov rdx, [rdi + 0x58]",
            "mov rsi, [rdi + 0x68]",
            "mov rbp, [rdi + 0x78]",
            "mov r8,  [rdi + 0x80]",
            "mov r9,  [rdi + 0x88]",
            "mov r10, [rdi + 0x90]",
            "mov r11, [rdi + 0x98]",
            "mov r12, [rdi + 0xa0]",
            "mov r13, [rdi + 0xa8]",
            "mov r14, [rdi + 0xb0]",
            "mov r15, [rdi + 0xb8]",
            "mov rdi, [rdi + 0x60]",
            "iretq",
        )
    }
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct TaskContextInner {
    // offset 0x00
    pub cr3: u64,
    pub rip: u64,
    pub rflags: u64,
    pub reserved_1: u64,
    // offset: 0x20
    pub cs: u64,
    pub ss: u64,
    pub fs: u64,
    pub gs: u64,
    // offset: 0x40
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rsp: u64,
    pub rbp: u64,
    // offset: 0x80
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    // offset: 0xc0
    pub fxsave_area: [u8; 512],
}

#[derive(Debug, Copy, Clone)]
#[repr(align(16))]
pub struct TaskContext(pub TaskContextInner);

impl TaskContext {
    pub const fn zero() -> Self {
        Self(TaskContextInner {
            cr3: 0,
            rip: 0,
            rflags: 0,
            reserved_1: 0,
            cs: 0,
            ss: 0,
            fs: 0,
            gs: 0,
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rdi: 0,
            rsi: 0,
            rsp: 0,
            rbp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            fxsave_area: [0; 512],
        })
    }
}

pub type TaskFunc = fn(u64, u64);

const DEFAULT_STACK_SIZE: u64 = 128 * 8 * 1024;

#[derive(Debug)]
pub struct Task {
    stack_size: u64,
    id: u64,
    stack: Vec<u64>,
    context: TaskContext,
    messages: VecDeque<Message>,
    is_running: bool,
    level: u8,
}

impl Task {
    pub fn new(id: u64) -> Self {
        Self {
            stack_size: DEFAULT_STACK_SIZE,
            id,
            stack: Vec::new(),
            context: TaskContext::zero(),
            messages: VecDeque::new(),
            level: DEFAULT_LEVEL,
            is_running: false,
        }
    }

    pub fn init_context(&mut self, f: TaskFunc, data: u64) -> &mut Self {
        let stack_size = self.stack_size / size_of::<u64>() as u64;
        self.stack.resize(stack_size as usize, 0);

        let stack_end_ref: &u64 = &self.stack[self.stack.len() - 1];
        let stack_end = stack_end_ref as *const u64 as u64 + size_of::<u64>() as u64;

        self.context.0.cr3 = unsafe { read_cr3_raw() };
        self.context.0.rflags = 0x202;
        self.context.0.cs = get_kernel_cs().0 as u64;
        self.context.0.ss = get_kernel_ss().0 as u64;
        self.context.0.rsp = (stack_end & !0xf) - 8;
        self.context.0.rip = f as u64;
        self.context.0.rdi = self.id;
        self.context.0.rsi = data;

        unsafe {
            let mut ptr = &self.context.0.fxsave_area[24] as *const u8 as *mut u32;
            *ptr = 0x1f80;
        }

        self
    }

    pub fn get_context<'a>(&'a self) -> &'a TaskContext {
        &self.context
    }

    pub fn get_context_mut<'a>(&'a mut self) -> &'a mut TaskContext {
        &mut self.context
    }

    pub fn get_id(&self) -> u64 {
        self.id
    }

    fn set_level(&mut self, level: u8) -> &mut Self {
        self.level = level;
        self
    }

    fn set_is_running(&mut self, state: bool) -> &mut Self {
        self.is_running = state;
        self
    }
}

// impl PriorityLevel {
//     pub const fn iter()
// }

// pub const MAX_LEVEL: PriorityLevel = PriorityLevel::Level3;
pub const MAX_LEVEL: u8 = 3;
pub const DEFAULT_LEVEL: u8 = 1;

#[derive(Debug)]
pub struct TaskManager {
    latest_id: u64 = 0,
    pub tasks: SlotMap<slotmap::DefaultKey, Task>,
    pub running_queues: [VecDeque<slotmap::DefaultKey>; MAX_LEVEL as usize + 1],
    pub current_level: u8,
    /// is_level_changed というか、current_level よりも優先度の高いタスクが存在しているかどうかを表すフラグ
    is_level_changed: bool,
}

impl TaskManager {
    pub fn new() -> Self {
        let mut self_ = Self {
            tasks: SlotMap::new(),
            latest_id: 0,
            running_queues: core::array::from_fn(|_| VecDeque::new()),
            current_level: MAX_LEVEL,
            is_level_changed: false,
        };

        let current_level = self_.current_level;

        let main_task = self_
            .new_task()
            .set_level(current_level)
            .set_is_running(true);
        let rip = main_task.context.0.rip;
        debug!("rip: {:x}", rip);
        let main_task_id = main_task.id;
        let main_task_key = self_.get_key_from_id(main_task_id).unwrap();

        self_.running_queues[current_level as usize].push_back(main_task_key);

        self_
    }

    pub fn get_key_from_id(&self, id: u64) -> Result<DefaultKey> {
        if let Some((key, _)) = self.tasks.iter().find(|(_, t)| t.id == id) {
            return Ok(key);
        }

        Err(Error::TaskNotFound)
    }

    pub fn get_current_task_id(&self) -> u64 {
        let key = self.running_queues[self.current_level as usize]
            .front()
            .unwrap();
        let task = self.tasks.get(*key).unwrap();
        task.id
    }

    pub fn new_task(&mut self) -> &mut Task {
        self.latest_id += 1;
        let key = self.tasks.insert(Task::new(self.latest_id));
        self.tasks.get_mut(key).unwrap()
    }

    /// レベルが負の場合はレベルを変更しない
    fn wakeup_by_key(&mut self, key: DefaultKey, level: Option<u8>) -> Result<()> {
        let task = if let Some(t) = self.tasks.get_mut(key) {
            t
        } else {
            return Err(Error::TaskNotFound);
        };

        if task.is_running {
            self.change_level_in_running_queue(key, level)?;
            return Ok(());
        }

        let level = level.unwrap_or(task.level);

        task.set_level(level);
        task.set_is_running(true);

        self.running_queues[level as usize].push_back(key);

        // 今実行しているタスクとは違うタスクのレベルについて言っている
        if level > self.current_level {
            self.is_level_changed = true;
        }

        Ok(())
    }

    /// level 引数は新しくタスクを生成したときに、そのタスクの優先度を明示的に指定するのではなくて、デフォルトの優先度を適用したいときにのみ None を渡してください。
    /// すでに存在しているタスクについて操作するときは level を明示的に指定してください。
    pub fn wakeup(&mut self, id: u64, level: Option<u8>) -> Result<()> {
        if let Some((key, _)) = self.tasks.iter().find(|(_, t)| t.id == id) {
            self.wakeup_by_key(key, level)?;
        } else {
            return Err(Error::TaskNotFound);
        }
        Ok(())
    }

    fn change_level_in_running_queue(&mut self, key: DefaultKey, level: Option<u8>) -> Result<()> {
        if level.is_none() {
            return Ok(());
        }

        let level = level.unwrap();

        assert!(level <= MAX_LEVEL);

        let task = if let Some(t) = self.tasks.get_mut(key) {
            t
        } else {
            return Err(Error::TaskNotFound);
        };

        // optimization
        if level == task.level {
            return Ok(());
        }

        let current_level = self.current_level;

        // 現在実行中 (running_queueに入っているとかじゃなくて今まさに実行している) 場合
        match self.running_queues[current_level as usize].front() {
            Some(k) => {
                if *k != key {
                    // change level of other task
                    self.running_queues[task.level as usize].retain(|x| *x != key);
                    self.running_queues[level as usize].push_back(key);
                    task.set_level(level);

                    if level > self.current_level {
                        self.is_level_changed = true;
                    }
                    return Ok(());
                }
            }
            None => return Err(Error::TaskNotFound),
        }

        // change level myself
        self.running_queues[current_level as usize].pop_front();
        self.running_queues[level as usize].push_front(key);
        task.set_level(level);
        if level >= current_level {
            self.current_level = level;
        } else {
            self.current_level = level;
            // is_level_changed はレベルが変わったかどうかというよりも、
            // current_level よりも優先度が高いタスクが存在しているかどうかを表すフラグとして理解したほうが良さそう
            self.is_level_changed = true;
        }

        Ok(())
    }

    pub fn send_message_to_task(&mut self, id: u64, message: &Message) -> Result<()> {
        if let Some((key, task)) = self.tasks.iter_mut().find(|(_, t)| t.id == id) {
            task.messages.push_back(*message);
            self.wakeup_by_key(key, None)?;
        } else {
            return Err(Error::TaskNotFound);
        }

        Ok(())
    }

    pub fn receive_message_from_task(&mut self, id: u64) -> Result<Option<Message>> {
        if let Some((_key, task)) = self.tasks.iter_mut().find(|(_, t)| t.id == id) {
            if task.messages.is_empty() {
                return Ok(None);
            }

            return Ok(task.messages.pop_front());
        }

        Err(Error::TaskNotFound)
    }
}

pub trait TaskManagerTrait {
    fn switch_task(&self, current_ctx: &TaskContext);
    fn sleep_by_key(&self, key: DefaultKey) -> Result<()>;
    fn sleep(&self, id: u64) -> Result<()>;
}

fn rotate_current_run_queue(
    self_: &mut MutexGuard<'_, TaskManager>,
    current_sleep: bool,
) -> DefaultKey {
    debug_assert!(self_.running_queues.len() > self_.current_level as usize);
    let current_level = self_.current_level;
    let level_queue = unsafe {
        self_
            .running_queues
            .get_unchecked_mut(current_level as usize)
    };
    let current_task = *level_queue
        .front()
        .expect("the current level queue is empty");
    level_queue.pop_front();
    if !current_sleep {
        level_queue.push_back(current_task);
    }
    if level_queue.is_empty() {
        self_.is_level_changed = false;

        for level in (0..=MAX_LEVEL).rev() {
            let level_queue = unsafe { self_.running_queues.get_unchecked(level as usize) };
            if !level_queue.is_empty() {
                self_.current_level = level;
                break;
            }
        }
    }

    if self_.is_level_changed {
        self_.is_level_changed = false;

        for level in (0..=MAX_LEVEL).rev() {
            let level_queue = unsafe { self_.running_queues.get_unchecked(level as usize) };
            if !level_queue.is_empty() {
                self_.current_level = level;
                break;
            }
        }

        // for (int lv = kMaxLevel; lv >= 0; --lv) {
        //         if (!running_[lv].empty()) {
        //                 current_level_ = lv;
        //                 break;
        //         }
        // }
    }

    current_task
}

fn get_current_task_key(task_manager: &MutexGuard<'_, TaskManager>) -> DefaultKey {
    let current_level = task_manager.current_level;
    debug_assert!(
        task_manager
            .running_queues
            .get(current_level as usize)
            .is_some()
    );
    let level_queue = unsafe {
        task_manager
            .running_queues
            .get_unchecked(current_level as usize)
    };
    debug_assert!(level_queue.front().is_some());
    *level_queue.front().unwrap()
}

impl TaskManagerTrait for Mutex<TaskManager> {
    fn switch_task(&self, current_ctx: &TaskContext) {
        let mut _self = self.lock();

        // タスクマネージャ側のカレントタスクコンテキストを更新する
        unsafe {
            let current_task_key = get_current_task_key(&_self);
            let current_ctx_dst = _self
                .tasks
                .get_unchecked_mut(current_task_key)
                .get_context() as *const TaskContext
                as *mut TaskContext;

            copy_nonoverlapping(current_ctx as *const TaskContext, current_ctx_dst, 1);
        }

        // キューの先頭のタスク(現在実行中のタスク)を末尾に移動する
        let old_current_task_key = rotate_current_run_queue(&mut _self, false);
        let new_current_task_key = get_current_task_key(&_self);

        if old_current_task_key != new_current_task_key {
            // キューのタスクを移動した結果、先頭のタスク(現在実行中のタスク)が変化した場合
            // (変化しない場合とは、例えばキューにタスクが1つしかない場合など)
            unsafe {
                // 新しいタスクのコンテキストを復元する
                let new_context =
                    &_self.tasks.get_unchecked(new_current_task_key).context as *const TaskContext;
                // ロックを解除する
                drop(_self);
                restore_context(new_context);
            }
        }
    }

    fn sleep_by_key(&self, key: DefaultKey) -> Result<()> {
        let mut _self = self.lock();

        let task = if let Some(t) = _self.tasks.get_mut(key) {
            t
        } else {
            return Err(Error::TaskNotFound);
        };

        if !task.is_running {
            return Ok(());
        }

        task.set_is_running(false);

        debug_assert!(!_self.running_queues[_self.current_level as usize].is_empty());
        if *_self.running_queues[_self.current_level as usize]
            .front()
            .unwrap()
            == key
        {
            let old_task_key = rotate_current_run_queue(&mut _self, true);
            let new_task_key = get_current_task_key(&_self);
            unsafe {
                let old_context =
                    &mut _self.tasks.get_unchecked_mut(old_task_key).context as *mut TaskContext;
                let new_context =
                    &_self.tasks.get_unchecked(new_task_key).context as *const TaskContext;

                drop(_self);

                switch_context(new_context, old_context);
                return Ok(());
            }
        }

        let current_level = _self.current_level;
        _self.running_queues[current_level as usize].retain(|k| *k != key);

        Ok(())
    }

    fn sleep(&self, id: u64) -> Result<()> {
        let self_ = self.lock();

        let key = if let Some((k, _)) = self_.tasks.iter().find(|(_, t)| t.id == id) {
            k
        } else {
            return Err(Error::TaskNotFound);
        };

        core::mem::drop(self_);

        self.sleep_by_key(key)?;

        Ok(())
    }
}

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    TaskNotFound,
}
