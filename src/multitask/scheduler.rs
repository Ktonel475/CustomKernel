use crate::multitask::task::{
    self, CURRENT, MAX_TASKS, TASK_LOCK, Task, TaskState, current_task, get_task,
    wake_sleeping_tasks,
};
use core::arch::naked_asm;

const TIME_SLICE_TICKS: u64 = 3;
static mut TICKS_ON_CURRENT: u64 = 0;

#[unsafe(naked)]
extern "C" fn switch_to(curr_rsp: *mut usize, next_rsp: *const usize) {
    naked_asm!("mov [rdi], rsp", "mov rsp, [rsi]", "ret",);
}

pub fn schedule() {
    unsafe {
        let _guard = TASK_LOCK.acquire();

        let curr_idx = CURRENT;
        let curr = get_task(curr_idx);

        if (*curr).state == TaskState::Running {
            (*curr).state = TaskState::Ready;
        }

        let mut next_idx = (curr_idx + 1) % MAX_TASKS;
        let mut found = false;

        for _ in 0..MAX_TASKS {
            let t = get_task(next_idx);
            if (*t).state == TaskState::Ready {
                found = true;
                break;
            }
            next_idx = (next_idx + 1) % MAX_TASKS;
        }

        if !found {
            if (*curr).state != TaskState::Running {
                (*curr).state = TaskState::Running;
            }
            return;
        }

        if next_idx == curr_idx {
            (*curr).state = TaskState::Running;
            return;
        }

        let next = get_task(next_idx);
        CURRENT = next_idx;
        (*next).state = TaskState::Running;
        TICKS_ON_CURRENT = 0;

        crate::device::serial::print("[sched] switch ");
        crate::device::serial::print_usize(curr_idx);
        crate::device::serial::print(" -> ");
        crate::device::serial::print_usize(next_idx);
        crate::device::serial::print("\n");

        let curr_rsp_ptr = &raw mut (*curr).rsp;
        let next_rsp_ptr = &raw mut (*next).rsp as *const usize;

        drop(_guard);

        switch_to(curr_rsp_ptr, next_rsp_ptr);
    }
}

pub fn tick(current_tick: u64) {
    unsafe {
        wake_sleeping_tasks(current_tick);

        let curr = current_task();
        (*curr).ticks_run += 1;
        TICKS_ON_CURRENT += 1;

        if TICKS_ON_CURRENT >= TIME_SLICE_TICKS {
            TICKS_ON_CURRENT = 0;
            schedule();
        }
    }
}

pub fn yield_cpu() {
    unsafe {
        core::arch::asm!("cli");
    }

    let curr = current_task();
    unsafe {
        (*curr).state = TaskState::Ready;
    }
    schedule();
}

pub fn sleep_ms(ms: u64) {
    let ticks_to_wait = (ms * crate::interrupt::pit::TICKS_PER_SECOND as u64) / 1000;
    let wake_at = crate::interrupt::pit::ticks() + ticks_to_wait.max(1);

    unsafe {
        core::arch::asm!("cli");
        task::block_until(CURRENT, wake_at);
    }

    schedule();
}

pub fn exit(code: i32) -> ! {
    unsafe {
        core::arch::asm!("cli");
        let curr_idx = CURRENT;

        let our_pid = (*get_task(curr_idx)).pid;
        for i in 0..MAX_TASKS {
            let t = get_task(i);
            if (*t).state == TaskState::Blocked
                && matches!((*t).wait_for_pid, Some(pid) if pid == our_pid)
            {
                (*t).state = TaskState::Ready;
                (*t).wait_for_pid = None;
                (*t).exit_code = code;
            }
        }
        task::terminate(curr_idx, code);
    }
    schedule();
    unreachable!("exit: schedule() returned");
}

pub fn wait_for(pid: u32) -> i32 {
    loop {
        unsafe {
            core::arch::asm!("cli");
            match task::find_pid(pid) {
                None => {
                    core::arch::asm!("sti");
                    return -1;
                }
                Some(slot) => {
                    let t = get_task(slot);
                    if (*t).state == TaskState::Terminated {
                        let code = (*t).exit_code;
                        (*t).state = TaskState::Empty;
                        core::arch::asm!("sti");
                        return code;
                    }
                    let curr_idx = CURRENT;
                    let curr = get_task(curr_idx);
                    (*curr).state = TaskState::Blocked;
                    (*curr).wait_for_pid = Some(pid);
                    schedule();
                }
            }
        }
    }
}
