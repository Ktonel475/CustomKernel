use crate::device::serial;
use crate::multitask::scheduler;

pub fn task_a() {
    serial::print("[task_a] starting\n");
    let mut count = 0u32;
    loop {
        serial::print("[task_a] tick");
        serial::print_usize(count as usize);
        serial::print("\n");

        if let Some(mut term) = crate::ui::terminal::Terminal::new() {
            term.print("Process A (count=");

            if count == 0 {
                term.putc('0');
            } else {
                let mut buf = [b'0'; 10];
                let mut n = count;
                let mut i = 10;
                while n > 0 {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                }
                for &b in &buf[i..] {
                    term.putc(b as char);
                }
            }
            term.print(")\n");
        }

        count += 1;
        scheduler::sleep_ms(500);
    }
}

pub fn task_b() {
    serial::print("[task_b] starting\n");
    let mut count = 0u32;
    loop {
        serial::print("[task_b] tick");
        serial::print_usize(count as usize);
        serial::print("\n");

        if let Some(mut term) = crate::ui::terminal::Terminal::new() {
            term.print("Process B (count=");

            if count == 0 {
                term.putc('0');
            } else {
                let mut buf = [b'0'; 10];
                let mut n = count;
                let mut i = 10;
                while n > 0 {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                }
                for &b in &buf[i..] {
                    term.putc(b as char);
                }
            }
            term.print(")\n");
        }

        count += 1;
        scheduler::sleep_ms(700);
    }
}

pub fn idle_task() -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
