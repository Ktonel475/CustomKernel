use crate::device::keyboard::outb;

const PIT_CHANNEL0: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;

const PIT_BASE_FREQUENCY: u32 = 1_193_182;

pub const TICKS_PER_SECOND: u32 = 100;

static mut TICK_COUNT: u64 = 0;

pub fn init() {
    let divisor = PIT_BASE_FREQUENCY / TICKS_PER_SECOND;
    assert!(
        divisor <= 0xffff,
        "PIT: divisor overflow - frequency too low"
    );

    outb(PIT_COMMAND, 0b0011_0110);
    outb(PIT_CHANNEL0, (divisor & 0xff) as u8);
    outb(PIT_CHANNEL0, ((divisor >> 8) & 0xff) as u8);

    crate::interrupt::idt::register_irq_handler(0, on_tick);
}

fn on_tick() {
    crate::serial::print("T");
    let tick = unsafe {
        let ptr: *mut u64 = &raw mut TICK_COUNT;
        *ptr = (*ptr).wrapping_add(1);
        *ptr
    };

    crate::multitask::scheduler::tick(tick);
}

pub fn ticks() -> u64 {
    unsafe { core::ptr::read_volatile(&raw const TICK_COUNT) }
}

pub fn sleep_ms(ms: u64) {
    let ticks_to_wait = (ms * TICKS_PER_SECOND as u64 + 999) / 1000;
    let start = ticks();
    while ticks().wrapping_sub(start) < ticks_to_wait {
        unsafe {
            core::arch::asm!("hlt");
        }
    }
}
