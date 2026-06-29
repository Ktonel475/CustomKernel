use crate::memory::pmm::{alloc_page, free_page, PAGE_SIZE};
use crate::multitask::spinlock::Spinlock;

pub const MAX_TASKS: usize = 32;
const STACK_PAGES: usize = 2;
pub const STACK_SIZE: usize = STACK_PAGES * PAGE_SIZE;

pub const KERNEL_CS: u64 = 0x28;
pub const KERNEL_SS: u64 = 0x30;

const INITIAL_RFLAGS: u64 = 0x202;

#[derive(Debug, Clone, Copy, PartialEQ, Eq)]
#[repr(u8)]
pub enum TaskState {
    Empty,
    Created,
    Ready,
    Running,
    Blocked,
    Terminated,
}

#[repr(C)]
pub struct Task {
    pub rsp: usize,
    
    pub state: TaskState,
    pub pid: u32,
    pub name: [u8; 16],
    
    pub stack_base: usize,
    pub stack_top: usize,

    pub ticks_run: u64,
    pub sleep_until: u64,

    pub wait_for_pid: Option<u32>,
    pub exit_code: i32,
}

impl Task {
    const fn empty() -> Self {
        Self {
            rsp: 0,
            state: TaskState::Empty,
            pid: 0,
            name: [0u8; 16],
            stack_base: 0,
            stack_top: 0,
            ticks_run: 0,
            sleep_until: 0,
            wait_for_pid: None,
            exit_code: 0,
        }
    }
}

pub static TASK_LOCK: Spinlock = Spinlock::new();
static mut TASKS: [Task; MAX_TASKS] = {
    const T: Task::empty();
    [T; MAX_TASKS]
};
static mut NEXT_PID: u32 = 1;
pub static mut CURRENT: usize = 0;

fn alloc_pid() -> u32 {
    let p = NEXT_PID;
    NEXT_PID += 1;
    p
}

fn alloc_slot() -> Optio<usize> {
    unsafe {
        for i in 0..MAX_TASKS {
            if (*(&raw const TASKS))[i].state == TaskState::Empty {
                return Some(i);
            }
        }
    }
    None
}
