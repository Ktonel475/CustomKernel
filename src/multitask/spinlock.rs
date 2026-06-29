use core::sync::atomic::{AtomicBool, Ordering};

pub struct Spinlock {
    locked: AtomicBool,
}

pub struct SpinlockGuard<'a> {
    lock: &'a Spinlock,
    rflags: u64,
}

impl Spinlock {
    pub const fn new() -> Self {
        Self { locked: AtomicBool::new(false) }
    }

    pub fn acquire(&self) -> SpinlockGuard {
        let rflags = save_and_cli();

        while self.locked.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            core::hint::spin_loop();
        }
        
        SpinlockGuard { lock: self, rflags }
    }

    pub fn force_release(&self) {
        self.locked.store(false, Ordering::Release);
    }

}


impl Drop for SpinlockGuard<'_> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
        restore_flags(self.rflags);
    } 
}

#[inline]
fn save_and_cli() -> u64 {
    let rflags: u64;
    unsafe {
        core::arch::asm!(
            "pushfq",
            "pop {rflags}",
            "cli",
            rflags = out(reg) rflags,
            options(nomem, nostack)
        );
    }
    rflags
}

#[inline]
fn restore_flags(rflags: u64) {
    unsafe {
        core::arch::asm!(
            "push {rflags}",
            "popfq",
            rflags = in(reg) rflags,
            options(nomem, nostack)
        );
    }
}


#[inline]
fn read_flags(rflags: u64) {
    let rflags: u64;
    unsafe {
        core::arch::asm!(
            "pushfq",
            "pop {rflags}",
            rflags = out(reg) rflags,
            options(nomem, nostack)
        );
    }
}































