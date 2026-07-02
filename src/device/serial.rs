use crate::device::keyboard::{inb, outb};

const COM1: u16 = 0x3F8;

pub fn init() {
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x80);
    outb(COM1 + 0, 0x03);
    outb(COM1 + 1, 0x00);
    outb(COM1 + 3, 0x03);
    outb(COM1 + 2, 0xC7);
    outb(COM1 + 4, 0x0B);
}

fn wait_ready() {
    while inb(COM1 + 5) & 0x20 == 0 {
        core::hint::spin_loop();
    }
}

pub fn putc(c: u8) {
    wait_ready();
    outb(COM1, c);
}

pub fn print(s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            putc(b'\r');
        }
        putc(b);
    }
}

pub fn print_hex64(v: u64) {
    print("0x");
    let mut buf = [b'0'; 16];
    let mut n = v;
    for i in (0..16).rev() {
        let nib = (n & 0xf) as u8;
        buf[i] = if nib < 10 {
            b'0' + nib
        } else {
            b'a' + nib - 10
        };
        n >>= 4;
    }
    for &b in &buf {
        putc(b);
    }
}

pub fn print_usize(v: usize) {
    if v == 0 {
        putc(b'0');
        return;
    }
    let mut buf = [b'0'; 20];
    let mut n = v;
    let mut i = 20;
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    for &b in &buf[i..] {
        putc(b);
    }
}
