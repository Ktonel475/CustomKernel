use crate::{
    device::keyboard::{inb, outb},
    memory::vmm::PTE_NO_CACHE,
};

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const ICW1_INIT: u8 = 0x10;
const ICW1_ICW4: u8 = 0x01;
const ICW4_8086: u8 = 0x01;

const PIC_EOI: u8 = 0x20;

const APIC_BASE_MSR: u32 = 0x1B;
const LAPIC_VIRT: usize = 0xffff_ffff_fee0_0000;

pub fn remap() {
    let mask1 = inb(PIC1_DATA);
    let mask2 = inb(PIC2_DATA);

    outb(PIC1_CMD, ICW1_INIT | ICW1_ICW4);
    io_wait();
    outb(PIC2_CMD, ICW1_INIT | ICW1_ICW4);
    io_wait();

    outb(PIC1_DATA, 32);
    io_wait();
    outb(PIC2_DATA, 40);
    io_wait();

    outb(PIC1_DATA, 0b0000_0100);
    io_wait();
    outb(PIC2_DATA, 2);
    io_wait();

    outb(PIC1_DATA, ICW4_8086);
    io_wait();
    outb(PIC2_DATA, ICW4_8086);
    io_wait();

    let _ = (mask1, mask2);
    outb(PIC1_DATA, 0b1111_1100);
    outb(PIC2_DATA, 0b1111_1111);

    enable_lapic_pic_mode();
}

fn read_msr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") msr,
            out("eax") lo,
            out("edx") hi,
            options(nomem, nostack)
        );
    }
    ((hi as u64) << 32) | lo as u64
}

pub fn enable_lapic_pic_mode() {
    let apic_phys = (read_msr(APIC_BASE_MSR) & 0xFFFF_F000) as usize;

    crate::memory::vmm::map_page(
        LAPIC_VIRT,
        apic_phys,
        crate::memory::vmm::PTE_PRESENT | crate::memory::vmm::PTE_WRITABLE | PTE_NO_CACHE,
    );

    let svr_ptr = (LAPIC_VIRT + 0x0F0) as *mut u32;
    let lint0_ptr = (LAPIC_VIRT + 0x350) as *mut u32;

    unsafe {
        let svr = svr_ptr.read_volatile();
        svr_ptr.write_volatile(svr | 0x1FF);

        lint0_ptr.write_volatile(0x700);
    }

    crate::device::serial::print("[lapic] LINT0 set to ExtInT mode\n");
}

#[inline]
fn io_wait() {
    for _ in 0..10 {
        core::hint::spin_loop();
    }
}

pub fn send_eoi(irq: u8) {
    if irq >= 8 {
        outb(PIC2_CMD, PIC_EOI);
    }
    outb(PIC1_CMD, PIC_EOI);
}

pub fn set_mark(irq: u8) {
    let port = if irq < 8 { PIC1_DATA } else { PIC2_DATA };
    let line = if irq < 8 { irq } else { irq - 8 };
    let value = inb(port) | (1 << line);
    outb(port, value);
}

pub fn read_irr() -> u8 {
    outb(PIC1_CMD, 0x0A);
    inb(PIC1_CMD)
}

pub fn read_isr() -> u8 {
    outb(PIC1_CMD, 0x0B);
    inb(PIC1_CMD)
}
