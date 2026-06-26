use crate::interrupt::idt::InterruptFrame;

pub const SYS_WRITE: u64 = 0;
pub const SYS_READ: u64 = 1;
pub const SYS_REBOOT: u64 = 2;

pub fn dispatch(frame: &mut InterruptFrame) {
    let result = match frame.rax {
        SYS_WRITE => sys_write(frame.rdi, frame.rsi as *const u8, frame.rdx as usize),
        SYS_READ => sys_read(frame.rdi, frame.rsi as *mut u8, frame.rdx as usize),
        SYS_REBOOT => sys_reboot(),
        other => {
            let _ = other;
            u64::MAX
        }
    };
    frame.rax = result;
}

fn sys_write(_fd: u64, buf: *const u8, len: usize) -> u64 {
    if buf.is_null() || len == 0 || len > 4096 {
        return u64::MAX;
    }
    if let Some(mut term) = crate::ui::terminal::Terminal::new() {
        for i in 0..len {
            let byte = unsafe { *buf.add(i) }; //TODO-add verification: page present, from userspace?, readable?
            term.putc(byte as char);
        }
    }
    len as u64
}

fn sys_read(_fd: u64, buf: *mut u8, len: usize) -> u64 {
    if buf.is_null() || len == 0 {
        return u64::MAX;
    }
    let mut count = 0usize;
    unsafe {
        while count < len {
            match crate::device::keyboard::global_pop() {
                Some(byte) => {
                    *buf.add(count) = byte;
                    count += 1;
                }
                None => break,
            }
        }
    }
    count as u64
}

fn sys_reboot() -> u64 {
    unsafe {
        let idtr: [u8; 10] = [0u8; 10];
        core::arch::asm!(
            "lidt [{idtr}]",
            "int3",
            idtr = in(reg) idtr.as_ptr(),
            options(nostack)
        );
    }
    unreachable!("sys_reboot: triple fault did not reset the machine");
}

#[inline(never)]
pub fn sys_write_wrapper(fd: u64, buf: *const u8, len: usize) -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
            "syscall",
            inout("rax") SYS_WRITE => ret,
            in("rdi") fd,
            in("rsi") buf,
            in("rdx") len,
            options(nostack)
        );
    }
    ret
}

#[inline(never)]
pub fn sys_read_wrapper(fd: u64, buf: *mut u8, len: usize) -> u64 {
    let ret: u64;
    unsafe {
        core::arch::asm!(
                "syscall",
                inout("rax") SYS_READ => ret,
                in("rdi") fd,
                in("rsi") buf,
                in("rdx") len,
                options(nostack)
        );
    }
    ret
}

#[inline(never)]
pub fn sys_reboot_wrapper() -> ! {
    unsafe {
        core::arch::asm!(
            "mov rax, {n}",
            "syscall",
            n = const SYS_REBOOT,
            options(nostack, noreturn)
        );
    }
}
