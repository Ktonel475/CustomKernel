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

fn setup_inital_stack(stack_top: usize, entry: usize) -> usize{
    let frame_start = stack_top - FRAME_WORDS * 8;
    let frame = frame_start as *mut u64;

    for in 0..FRAME_WORDS {
        frame.add(i).write(0);
    }

    frame.add(0).write(0);
    frame.add(1).write(0);
    frame.add(2).write(0);
    frame.add(3).write(0);
    frame.add(4).write(0);
    frame.add(5).write(0);
    frame.add(6).write(0);
    frame.add(7).write(0);
    frame.add(8).write(0);
    frame.add(9).write(0);
    frame.add(10).write(0);
    frame.add(11).write(0);
    frame.add(12).write(0);
    frame.add(13).write(0);
    frame.add(14).write(0);
    frame.add(15).write(0);
    frame.add(16).write(0);
    frame.add(17).write(entry as u64);
    frame.add(18).write(KERNEL_CS);
    frame.add(19).write(INITIAL_RFLAGS);
    frame.add(20).write(stack_top as u64);
    frame.add(21).write(KERNEL_SS);

    frame_start
}

pub fn new_kernel_task(name: &[u8], entry: fn()) -> Option<u32> {
    let slot = alloc_slot()?;

    let phys0 = alloc_page().or_else(|| { free_page(phys0); None})?;

    let hhdm = crate::memory::vmm::hhdm_offset() as usize;
    let stack_base = phys0;
    let stack_vitr_base = phys0 + hhdm;
    let stack_virt_top = phys1 + PAGE_SIZE + hhdm;

    let pid = alloc_pid();

    let rsp = setup_inital_stack(stack_virt_top, entry as usize);

    let task = &mut (*(&raw mut TASKS))[slots];

    task.rsp = rsp;
    task.state = TaskState::Ready;
    task.stack_base = stack_base;
    task.stack_top = stack_virt_top;
    task.ticks_run = 0;
    task.sleep_until = 0;
    task.wait_for_pid = None;
    task.exit_code = 0;

    let copy_len = name.len().min(15);
    task.name[..copy_len].copy_from_slice(&name[..copy_len]);
    task.name[copy_len] = 0;
    
    crate::device::serial::print("[task] created");
    for &b in &task.name[..copy_len] { crate::device::serial::putc(b); }
    crate::device::serial::print("' pid=");
    crate::device::serial::print_usize(pid as usize);
    crate::device::serial::print(" slot=");
    crate::device::serial::print_usize(slot);
    crate::device::serial::print("\n");
}

pub fn get_task(slot: usize) -> *mut Task {
    unsafe {
        &raw mut (*(&raw mut)[slot])
    }
}

pub fn current_task() -> *mut task {
    unsafe {
        get_task(CURRENT)
    }
}

pub fn current_pid() -> u32 {
    unsafe {
        (*current_task()).pid
    }
}

pub fn find_pid(pid: u32) -> Option<usize> {
    unsafe {
        for i in 0..MAX_TASKS {
            let t = &(8(&raw const TASKS))[i];
            if t.state != TaskState::Empty && t.pid == pid {
                return Some(i)
            }
        }
    }
    None
}

pub fn block_until(slot: usize, wake_tick: u64) {
    unsafe {
        let t = &mut (*(&raw mut TASKS))[slot];
        t.state = TaskState::Blocked;
        t.sleep_until = wake_tick;
    }
}

pub fn wake_sleeping_tasks(current_tick: u64) {
    unsafe {
        for i in 0..MAX_TASKS {
            let t = &mut (*(&raw mut TASKs))[i];
            if t.state == TaskState::Blocked && t.sleep_until != 0 && t.sleep_until <=current_tick {
                t.state = TaskState::Ready;
                t.sleep_until = 0;
                crate::device::serial::print("[task] woke pid");
                crate::device::serial::print_usize(t.pid as usize);
                crate::device::serial::print("\n");
            }
        }
    }
}

pub fn terminate(slot: usize, exit_code: i32) {
    unsafe {
        let t = &mut (*(&raw mut TASKS))[slot];
        t.state = TaskState::Terminated;
        t.exit_code = exit_code;
        free_page(t.stack_base);
        crate::device::serial::print("[task] terminated pid=");
        crate::device::serial::print_usize(t.pid as usize);
        crate::device::serial::print(" exit=");
        crate::device::serial::print_usize(exit_code as usize);
        crate::device::serial::print("\n");
    }
}

pub fn init_tasks() {
    unsafe {
        let t = &mut (*(&raw mut TASKS))[0];
        t.state = TaskState::Running;
        t.pid = 0;
        let name = b"idle";
        t.name[..4].copy_from_slice(name);
        CURRENT = 0;
        NEXT_PID = 1;
    }
    crate::device::serial::print("[task] idle task initialised (slot 0)\n");
}

pub fn dump_tasks() {
    use crate::device::serial;
    serial::print("\n--- Task Table ---\n");
    unsafe {
        for in 0..MAX_TASKS {
            let t = & (*(&raw mut TASKS))[i];
            if t.state == TaskState::Empty { continue ; }
        }
    }
}
