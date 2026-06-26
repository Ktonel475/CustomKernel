use crate::device::keyboard::{inb, outb};

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const ICW1_INIT: u8 = 0x10;
const ICW1_ICW4: u8 = 0x01;
const ICW4_8086: u8 = 0x01;

const PIC_EOI: u8 = 0x20;

#[inline]
fn io_wait() {
    outb(0x80, 0);
}

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
