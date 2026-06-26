use crate::interrupt;

#[inline]
pub fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        core::arch::asm!(
            "in al, dx",
            out("al") value,
            in("dx") port,
            options(nomem, nostack, preserves_flags),
        );
    }
    value
}

#[inline]
pub fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[rustfmt::skip]
static SCANCODE_TABLE: [u8; 84] = [
    0,    27,   b'1', b'2', b'3', b'4', b'5', b'6',  // 0x00
    b'7', b'8', b'9', b'0', b'-', b'=', 8,   b'\t', // 0x08  (8 = backspace)
    b'q', b'w', b'e', b'r', b't', b'y', b'u', b'i',  // 0x10
    b'o', b'p', b'[', b']', b'\n',0,   b'a', b's',   // 0x18  (0 = ctrl)
    b'd', b'f', b'g', b'h', b'j', b'k', b'l', b';',  // 0x20
    b'\'',b'`', 0,   b'\\',b'z', b'x', b'c', b'v',  // 0x28  (0 = lshift)
    b'b', b'n', b'm', b',', b'.', b'/', 0,   b'*',   // 0x30  (0 = rshift)
    0,    b' ', 0,   0,   0,   0,   0,   0,           // 0x38  (0 = alt, caps, f-keys)
    0,   0,   0,   0,   0,   0,   0,   b'7',          // 0x40
    b'8', b'9', b'-', b'4', b'5', b'6', b'+', b'1',  // 0x48
    b'2', b'3', b'0', b'.',                            // 0x50
];

/// Shifted scancode table (same indices)
#[rustfmt::skip]
static SCANCODE_TABLE_SHIFT: [u8; 84] = [
    0,    27,   b'!', b'@', b'#', b'$', b'%', b'^',  // 0x00
    b'&', b'*', b'(', b')', b'_', b'+', 8,   b'\t',  // 0x08
    b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I',  // 0x10
    b'O', b'P', b'{', b'}', b'\n',0,   b'A', b'S',   // 0x18
    b'D', b'F', b'G', b'H', b'J', b'K', b'L', b':',  // 0x20
    b'"', b'~', 0,   b'|', b'Z', b'X', b'C', b'V',   // 0x28
    b'B', b'N', b'M', b'<', b'>', b'?', 0,   b'*',   // 0x30
    0,    b' ', 0,   0,   0,   0,   0,   0,            // 0x38
    0,   0,   0,   0,   0,   0,   0,   b'7',           // 0x40
    b'8', b'9', b'-', b'4', b'5', b'6', b'+', b'1',   // 0x48
    b'2', b'3', b'0', b'.',                             // 0x50
];

const SC_LSHIFT_DOWN: u8 = 0x2a;
const SC_RSHIFT_DOWN: u8 = 0x36;
const SC_LSHIFT_UP: u8 = 0xAA;
const SC_RSHIFT_UP: u8 = 0xB6;

const BUF_SIZE: usize = 256;

pub struct KeyBuffer {
    buf: [u8; BUF_SIZE],
    head: usize,
    tail: usize,
}

impl KeyBuffer {
    pub const fn new() -> Self {
        Self {
            buf: [0u8; BUF_SIZE],
            head: 0,
            tail: 0,
        }
    }

    pub fn push(&mut self, byte: u8) {
        let next = (self.head + 1) % BUF_SIZE;
        if next != self.tail {
            self.buf[self.head] = byte;
            self.head = next;
        }
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.head == self.tail {
            None
        } else {
            let b = self.buf[self.tail];
            self.tail = (self.tail + 1) % BUF_SIZE;
            Some(b)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }
}

pub struct Keyboard {
    pub buffer: KeyBuffer,
    shift: bool,
}

impl Keyboard {
    pub const fn new() -> Self {
        Self {
            buffer: KeyBuffer::new(),
            shift: false,
        }
    }

    pub fn handle_irq(&mut self) {
        let scancode: u8 = inb(0x60);

        match scancode {
            SC_LSHIFT_DOWN | SC_RSHIFT_DOWN => self.shift = true,
            SC_LSHIFT_UP | SC_RSHIFT_UP => self.shift = false,
            _ => {
                if scancode & 0x80 != 0 {
                    return;
                }
                let idx = scancode as usize;
                let table = if self.shift {
                    &SCANCODE_TABLE_SHIFT
                } else {
                    &SCANCODE_TABLE
                };
                if idx < table.len() {
                    let ascii = table[idx];
                    if ascii != 0 {
                        self.buffer.push(ascii);
                    }
                }
            }
        }
    }
}

static mut GLOBAL_KEYBOARD: Keyboard = Keyboard::new();

pub fn init() {
    outb(0x64, 0x20);
    wait_read();
    let mut ccb = inb(0x60);

    ccb |= 0x01;
    ccb &= !0x10;

    wait_write();
    outb(0x64, 0x60);
    wait_write();
    outb(0x60, ccb);

    interrupt::idt::register_irq_handler(1, on_irq1);
}

fn wait_write() {
    let mut timeout = 100_000u32;
    while timeout > 0 {
        if inb(0x64) & 0x02 == 0 {
            return;
        }
        timeout -= 1;
    }
}

fn wait_read() {
    let mut timeout = 100_000u32;
    while timeout > 0 {
        if inb(0x64) & 0x01 != 0 {
            return;
        }
        timeout -= 1;
    }
}

fn on_irq1() {
    unsafe {
        let ptr: *mut Keyboard = &raw mut GLOBAL_KEYBOARD;
        (*ptr).handle_irq();
    }
}

pub fn global_pop() -> Option<u8> {
    unsafe {
        let ptr: *mut Keyboard = &raw mut GLOBAL_KEYBOARD;
        (*ptr).buffer.pop()
    }
}
