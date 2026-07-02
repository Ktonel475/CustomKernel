use crate::memory::vmm::read_cr2;
use core::arch::naked_asm;
use paste::paste;
use seq_macro::seq;

pub const KERNEL_CS: u16 = 0x28;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    _zero: u32,
}

impl IdtEntry {
    const fn zero() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_mid: 0,
            offset_high: 0,
            _zero: 0,
        }
    }

    fn set(&mut self, handler: u64, selector: u16, type_attr: u8) {
        self.offset_low = (handler & 0xffff) as u16;
        self.offset_mid = ((handler >> 16) & 0xffff) as u16;
        self.offset_high = ((handler >> 32) & 0xffff_ffff) as u32;
        self.selector = selector;
        self.ist = 0;
        self.type_attr = type_attr;
        self._zero = 0;
    }
}

#[repr(C, packed)]
struct Idtr {
    limit: u16,
    base: u64,
}

const GATE_INTERRUPT: u8 = 0x8e;

static mut IDT: [IdtEntry; 256] = [IdtEntry::zero(); 256];

#[repr(C)]
pub struct InterruptFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub vector: u64,
    pub error_code: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

pub type IrqHandlerFn = fn();

const IRQ_COUNT: usize = 16;
static mut IRQ_HANDLES: [Option<IrqHandlerFn>; IRQ_COUNT] = [None; IRQ_COUNT];

pub fn register_irq_handler(irq: u8, handler: IrqHandlerFn) {
    assert!(
        (irq as usize) < IRQ_COUNT,
        "register_irq_handler: irq out of range"
    );
    unsafe {
        let table_ptr: *mut [Option<IrqHandlerFn>; IRQ_COUNT] = &raw mut IRQ_HANDLES;
        (*table_ptr)[irq as usize] = Some(handler);
    }
}

macro_rules! make_stub {
    ($name:ident, $vector:expr, has_error_code) => {
        #[unsafe(naked)]
        unsafe extern "C" fn $name() {
            naked_asm!(
                "push {v}",
                "jmp {common}",
                v = const $vector,
                common = sym common_stub,
            );
        }
    };

    ($name:ident, $vector:expr) => {
        #[unsafe(naked)]
        unsafe extern "C" fn $name() {
            naked_asm!(
                "push 0",
                "push {v}",
                "jmp {common}",
                v = const $vector,
                common = sym common_stub,
            );
        }
    };
}
macro_rules! make_vector {
    (8) => {
        make_stub!(stub_8, 8, has_error_code);
    };
    (10) => {
        make_stub!(stub_10, 10, has_error_code);
    };
    (11) => {
        make_stub!(stub_11, 11, has_error_code);
    };
    (12) => {
        make_stub!(stub_12, 12, has_error_code);
    };
    (13) => {
        make_stub!(stub_13, 13, has_error_code);
    };
    (14) => {
        make_stub!(stub_14, 14, has_error_code);
    };
    (17) => {
        make_stub!(stub_17, 17, has_error_code);
    };
    (21) => {
        make_stub!(stub_21, 21, has_error_code);
    };
    (29) => {
        make_stub!(stub_29, 29, has_error_code);
    };
    (30) => {
        make_stub!(stub_30, 30, has_error_code);
    };

    ($n:literal) => {
        paste! {
            make_stub!([<stub_$n>], $n);
        }
    };
}

seq!( N in 0..48 {
    make_vector!(N);
});

make_stub!(stub_128, 128);

#[unsafe(naked)]
unsafe extern "C" fn common_stub() {
    naked_asm!(
        "push rax", "push rbx", "push rcx", "push rdx",
        "push rsi", "push rdi", "push rbp",
        "push r8",  "push r9",  "push r10", "push r11",
        "push r12", "push r13", "push r14", "push r15",
        "mov rdi, rsp",          // arg0 = pointer to InterruptFrame
        "call {handler}",
        "pop r15", "pop r14", "pop r13", "pop r12",
        "pop r11", "pop r10", "pop r9",  "pop r8",
        "pop rbp", "pop rdi", "pop rsi",
        "pop rdx", "pop rcx", "pop rbx", "pop rax",
        "add rsp, 16",           // discard vector + error_code
        "iretq",
        handler = sym common_handler,
    );
}

extern "C" fn common_handler(frame: *mut InterruptFrame) {
    let frame = unsafe { &mut *frame };

    match frame.vector {
        0 => exception_divide_by_zero(frame),
        1 => exception_generic("Debug", frame),
        2 => exception_generic("Non-Maskable Interrupt", frame),
        3 => exception_generic("Breakpoint", frame),
        4 => exception_generic("Overflow", frame),
        5 => exception_generic("Bound Range Exceeded", frame),
        6 => exception_invalid_opcode(frame),
        7 => exception_generic("Device Not Available", frame),
        8 => exception_double_fault(frame),
        9 => exception_generic("Coprocessor Segment Overrun", frame),
        10 => exception_generic("Invalid TSS", frame),
        11 => exception_generic("Segment Not Present", frame),
        12 => exception_generic("Stack-Segment Fault", frame),
        13 => exception_general_protection_fault(frame),
        14 => exception_page_fault(frame),
        16 => exception_generic("x87 Floating-Point Exception", frame),
        17 => exception_generic("Alignment Check", frame),
        18 => exception_generic("Machine Check", frame),
        19 => exception_generic("SIMD Floating-Point Exception", frame),
        20 => exception_generic("Virtualization Exception", frame),
        21 => exception_generic("Control Protection Exception", frame),
        28 => exception_generic("Hypervisor Injection Exception", frame),
        29 => exception_generic("VMM Communication Exception", frame),
        30 => exception_generic("Security Exception", frame),
        15 | 22..=27 | 31 => exception_generic("Reserved", frame),

        32..=47 => {
            crate::device::serial::print("I");
            let irq = (frame.vector - 32) as u8;

            unsafe {
                let table_ptr: *const [Option<IrqHandlerFn>; IRQ_COUNT] = &raw const IRQ_HANDLES;
                if let Some(h) = (*table_ptr)[irq as usize] {
                    h();
                }
            }
            crate::interrupt::pic::send_eoi(irq);
        }

        128 => crate::interrupt::syscall::dispatch(frame),

        _ => exception_generic("Unknown", frame),
    }
}

fn with_term<F: FnOnce(&mut crate::ui::terminal::Terminal)>(f: F) {
    if let Some(mut term) = crate::ui::terminal::Terminal::new() {
        use crate::ui::terminal::{COLOR_BG, COLOR_FG, COLOR_PANIC};
        term.set_color(COLOR_PANIC, COLOR_BG);
        f(&mut term);
        term.set_color(COLOR_FG, COLOR_BG);
    }
}

fn halt_forever() -> ! {
    loop {
        unsafe {
            core::arch::asm!("cli; hlt");
        }
    }
}

fn exception_generic(name: &str, frame: &InterruptFrame) -> ! {
    with_term(|term| {
        term.print("\n!! CPU EXCEPTION: ");
        term.print(name);
        term.print("\n!! Vector: ");
        term.print_hex64(frame.vector);
        term.print("\nError code: ");
        term.print_hex64(frame.error_code);
        term.print("\nRIP: ");
        term.print_hex64(frame.rip);
        term.print("\n");
    });
    halt_forever();
}

fn exception_page_fault(frame: &InterruptFrame) -> ! {
    let cr2 = read_cr2();
    with_term(|term| {
        term.print("\n!! PAGE FAULT !!\n");
        term.print("CR2 (fault addr): ");
        term.print_hex64(cr2);
        term.print("\nError code:      ");
        term.print_hex64(frame.error_code);
        term.print("\nReason:          ");
        if frame.error_code & 1 == 0 {
            term.print("not-present ");
        } else {
            term.print("protection-violation ");
        }
        if frame.error_code & 2 != 0 {
            term.print("write ");
        } else {
            term.print("read ");
        }
        if frame.error_code & 4 != 0 {
            term.print("user-mode ");
        }
        term.print("\nRIP:             ");
        term.print_hex64(frame.rip);
        term.print("\n");
    });
    halt_forever();
}

fn exception_divide_by_zero(frame: &InterruptFrame) -> ! {
    with_term(|term| {
        term.print("\n !! DIVIDE-BY-ZERO EXCEPTION !! \nRIP: ");
        term.print_hex64(frame.rip);
        term.print("\n");
    });
    halt_forever();
}

fn exception_general_protection_fault(frame: &InterruptFrame) -> ! {
    with_term(|term| {
        term.print("\n!! GENERAL PROTECTION FAULT !!\n");
        term.print("Saved RIP:     ");
        term.print_hex64(frame.rip);
        term.print("\nError code:   ");
        term.print_hex64(frame.error_code);
        term.print("\nCS:           ");
        term.print_hex64(frame.cs);
        term.print("\n");
    });
    halt_forever();
}

fn exception_invalid_opcode(frame: &InterruptFrame) -> ! {
    with_term(|term| {
        term.print("\n!! INVALID OPCODE EXCEPTION !! \nRIP: ");
        term.print_hex64(frame.rip);
        term.print("\n");
    });
    halt_forever();
}

fn exception_double_fault(frame: &InterruptFrame) -> ! {
    with_term(|term| {
        term.print("\n!! DOUBLE FAULT !! (register dump)\n");
        term.print("RIP: ");
        term.print_hex64(frame.rip);
        term.print("\n");
        term.print("CS:  ");
        term.print_hex64(frame.cs);
        term.print("\n");
        term.print("RAX: ");
        term.print_hex64(frame.rax);
        term.print("  ");
        term.print("RBX: ");
        term.print_hex64(frame.rbx);
        term.print("\n");
        term.print("RCX: ");
        term.print_hex64(frame.rcx);
        term.print("  ");
        term.print("RDX: ");
        term.print_hex64(frame.rdx);
        term.print("\n");
        term.print("RSI: ");
        term.print_hex64(frame.rsi);
        term.print("  ");
        term.print("RDI: ");
        term.print_hex64(frame.rdi);
        term.print("\n");
        term.print("RBP: ");
        term.print_hex64(frame.rbp);
        term.print("  ");
        term.print("RSP: ");
        term.print_hex64(frame.rsp);
        term.print("\n");
        term.print("R8:  ");
        term.print_hex64(frame.r8);
        term.print("  ");
        term.print("R9:  ");
        term.print_hex64(frame.r9);
        term.print("\n");
        term.print("R10: ");
        term.print_hex64(frame.r10);
        term.print("  ");
        term.print("R11: ");
        term.print_hex64(frame.r11);
        term.print("\n");
        term.print("R12: ");
        term.print_hex64(frame.r12);
        term.print("  ");
        term.print("R13: ");
        term.print_hex64(frame.r13);
        term.print("\n");
        term.print("R14: ");
        term.print_hex64(frame.r14);
        term.print("  ");
        term.print("R15: ");
        term.print_hex64(frame.r15);
        term.print("\n");
        term.print("RFLAGS: ");
        term.print_hex64(frame.rflags);
        term.print("\n");
    });
    halt_forever();
}

macro_rules! install {
    ($idt_ptr:expr, $vector:expr, $stub:expr) => {
        let entry_ptr: *mut IdtEntry = ($idt_ptr as *mut IdtEntry).add($vector);
        (*entry_ptr).set($stub as *const () as u64, KERNEL_CS, GATE_INTERRUPT);
    };
}

pub fn init_idt() {
    unsafe {
        let idt_ptr: *mut [IdtEntry; 256] = &raw mut IDT;

        seq!(N in 0..48 {
            paste! {
                install!(idt_ptr, N, [<stub_ N>]);
            }
        });

        let entry_ptr: *mut IdtEntry = (idt_ptr as *mut IdtEntry).add(128);
        (*entry_ptr).set(stub_128 as *const () as u64, KERNEL_CS, 0xee);

        let idtr = Idtr {
            limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
            base: idt_ptr as u64,
        };
        core::arch::asm!(
            "lidt [{idtr}]",
            idtr = in(reg) &idtr as *const Idtr,
        );
    }
}
