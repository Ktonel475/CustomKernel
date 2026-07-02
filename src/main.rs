#![no_std]
#![no_main]

use core::panic::PanicInfo;

pub mod device {
    pub mod keyboard;
    pub mod serial;
}
pub mod interrupt {
    pub mod idt;
    pub mod pic;
    pub mod pit;
    pub mod syscall;
}
pub mod memory {
    pub mod heap;
    pub mod pmm;
    pub mod vmm;
}
pub mod multitask {
    pub mod demo_tasks;
    pub mod scheduler;
    pub mod spinlock;
    pub mod task;
}
pub mod shell;
pub mod ui {
    pub mod terminal;
}

use crate::{
    device::{keyboard, serial},
    interrupt::{idt, pic, pit, syscall},
    memory::{heap, pmm, vmm},
    multitask::{demo_tasks, scheduler, task},
    ui::terminal::{COLOR_BG, COLOR_FG, COLOR_PANIC, Terminal},
};

#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: limine::BaseRevision = limine::BaseRevision::new();

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        core::arch::asm!("cli");
    }

    serial::print("\n!!! KERNEL PANIC !!!\n");
    if let Some(loc) = info.location() {
        serial::print("at: ");
        serial::print(loc.file());
        serial::print(":");
        serial::print_usize(loc.line() as usize);
        serial::print("\n");
    }

    if let Some(mut term) = Terminal::new() {
        term.set_color(COLOR_PANIC, COLOR_BG);
        term.print("\nKERNEL PANIC\n");
        term.set_color(COLOR_FG, COLOR_BG);
        if let Some(loc) = info.location() {
            term.print("at: ");
            term.print(loc.file());
            term.print("\n");
        }
    }

    loop {
        unsafe {
            core::arch::asm!("cli; hlt");
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    serial::init();
    serial::print("\n[boot] serial ok\n");

    assert!(BASE_REVISION.is_supported());
    serial::print("[boot] limine revision ok\n");

    idt::init_idt();
    serial::print("[boot] idt ok\n");

    pic::remap();
    crate::device::serial::print("[pic] master mask=");
    crate::device::serial::print_hex64(crate::device::keyboard::inb(0x21) as u64);
    crate::device::serial::print("\n");

    crate::device::serial::print("[pic] slave mask=");
    crate::device::serial::print_hex64(crate::device::keyboard::inb(0xA1) as u64);
    crate::device::serial::print("\n");
    serial::print("[boot] pic remapped\n");

    keyboard::init();
    serial::print("[boot] keyboard init ok\n");

    task::init_tasks();
    serial::print("[boot] task table ok\n");

    pit::init();
    serial::print("[boot] pit ok\n");

    unsafe {
        core::arch::asm!("sti");
        core::arch::asm!("int 32")
    }
    serial::print("[boot] interrupts enabled\n");

    vmm::init_vmm();
    serial::print("[boot] vmm ok\n");

    pmm::init_pmm();
    serial::print("[boot] pmm ok, free pages: ");
    serial::print_usize(pmm::get_free_page_count());
    serial::print("\n");

    heap::kmalloc_init(heap::HEAP_START, 1024 * 1024);
    serial::print("[boot] heap ok\n");

    task::new_kernel_task(b"task_a", demo_tasks::task_a).expect("failed to create task_a");
    task::new_kernel_task(b"task_b", demo_tasks::task_b).expect("failed to create task_b");
    task::new_kernel_task(b"shell", shell::simple_shell::run_shell)
        .expect("failed to create shell task");

    serial::print("[boot] tasks created\n");
    task::dump_tasks();

    serial::print("[boot] entering idle\n");
    demo_tasks::idle_task();
}
