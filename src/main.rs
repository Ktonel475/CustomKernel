#![no_std]
#![no_main]

mod device;
mod interrupt;
mod memory;
mod shell;
mod ui;

use crate::{
    device::keyboard,
    interrupt::{idt, pic, pit},
    memory::{heap, pmm, vmm},
    shell::simpleShell,
    ui::terminal::{COLOR_BG, COLOR_FG, COLOR_PANIC, Terminal},
};
use core::panic::PanicInfo;

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: limine::BaseRevision = limine::BaseRevision::new();

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    if let Some(mut term) = Terminal::new() {
        term.set_color(COLOR_PANIC, COLOR_BG);
        term.print("\nKERNEL PANIC\n");
        term.set_color(COLOR_FG, COLOR_BG);
        if let Some(loc) = _info.location() {
            term.print("at: ");
            term.print(loc.file());
            term.print("\n");

            let mut n = loc.line();
            if n == 0 {
                term.print("0");
            } else {
                let mut buf = [b'0'; 10];
                let mut i = 10;
                while n > 0 {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                }
                for &b in &buf[i..] {
                    term.putc(b as char);
                }
                term.print("\n");
            }
        }
    }

    loop {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    assert!(BASE_REVISION.is_supported());

    idt::init_idt();
    pic::remap();
    keyboard::init();
    pit::init();

    unsafe {
        core::arch::asm!("sti");
    }

    vmm::init_vmm();
    pmm::init_pmm();
    heap::kmalloc_init(heap::HEAP_START, 1024 * 1024);
    simpleShell::run_shell();
}
