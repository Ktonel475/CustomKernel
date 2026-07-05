use crate::{device::serial, ui::terminal::Terminal};

const CMD_BUF_SIZE: usize = 256;
const PROMPT: &str = "kernel> ";

pub struct Shell {
    buf: [u8; CMD_BUF_SIZE],
    len: usize,
}

impl Shell {
    pub const fn new() -> Self {
        Self {
            buf: [0u8; CMD_BUF_SIZE],
            len: 0,
        }
    }

    pub fn show_prompt(&self, term: &mut Terminal) {
        term.print(PROMPT);
    }

    pub fn handle_key(&mut self, term: &mut Terminal, key: u8) {
        match key {
            b'\n' => {
                term.putc('\n');
                self.execute(term);
                self.len = 0;
                self.show_prompt(term);
            }
            8 => {
                if self.len > 0 {
                    self.len -= 1;
                    term.backspace();
                }
            }
            _ if key >= 32 && key < 127 => {
                if self.len < CMD_BUF_SIZE - 1 {
                    self.buf[self.len] = key;
                    self.len += 1;
                    term.putc(key as char);
                }
            }
            _ => {}
        }
    }

    fn execute(&mut self, term: &mut Terminal) {
        let cmd_bytes = &self.buf[..self.len];
        let cmd_str = trim_ascii(cmd_bytes);
        if cmd_str.is_empty() {
            return;
        }

        let (verb, rest) = split_first_word(cmd_str);

        match verb {
            // ── Basic commands ───────────────────────────────────────────────
            b"help" => {
                term.print("Available commands:\n");
                term.print("  help              - show this help\n");
                term.print("  cls               - clear the screen\n");
                term.print("  color <fg> <bg>   - set colors (hex RRGGBB)\n");
                term.print("  echo <text>       - print text\n");
                term.print("  reboot            - reboot the machine\n");
                term.print("  meminfo           - show PMM free/total pages\n");
                term.print("  memtest           - run heap alloc/free test\n");
                term.print("  heap              - dump heap block list\n");
                term.print("  pgfault           - trigger intentional page fault\n");
                term.print("  divzero           - trigger divide-by-zero exception\n");
                term.print("  intopcode         - trigger invalid opcode exception\n");
                term.print("  breakpoint        - trigger int3 (returns normally)\n");
                term.print("  dfault            - trigger a double fault (stack overflow)\n");
                term.print("  uptime            - show ticks/seconds since boot\n");
                term.print("  sleep <secs>      - sleep N seconds using the PIT\n");
                term.print("  synwrite <text>   - write text via the int 0x80 syscall path\n");
            }

            b"cls" => {
                term.clear();
            }

            b"echo" => {
                for &b in trim_ascii(rest) {
                    term.putc(b as char);
                }
                term.putc('\n');
            }

            b"color" => {
                let (fg_s, rest2) = split_first_word(trim_ascii(rest));
                let (bg_s, _) = split_first_word(trim_ascii(rest2));
                match (parse_hex6(fg_s), parse_hex6(bg_s)) {
                    (Some(fg), Some(bg)) => {
                        term.set_color(fg, bg);
                        term.print("Color updated.\n");
                    }
                    _ => term.print("Usage: color RRGGBB RRGGBB\n"),
                }
            }

            b"reboot" => {
                term.print("Rebooting...\n");
                unsafe {
                    let idtr: [u8; 10] = [0u8; 10];
                    core::arch::asm!(
                        "lidt [{idtr}]",
                        "int3",
                        idtr = in(reg) idtr.as_ptr(),
                        options(nostack)
                    );
                }
                loop {
                    core::hint::spin_loop();
                }
            }

            // ── Memory commands ──────────────────────────────────────────────
            b"meminfo" => {
                let free = crate::memory::pmm::get_free_page_count();
                let total = crate::memory::pmm::get_total_page_count();
                let used = total - free;
                term.print("PMM status:\n");
                term.print("  Free  pages : ");
                print_usize(term, free);
                term.print(" (");
                print_usize(term, free * 4);
                term.print(" KiB)\n");
                term.print("  Used  pages : ");
                print_usize(term, used);
                term.print(" (");
                print_usize(term, used * 4);
                term.print(" KiB)\n");
                term.print("  Total pages : ");
                print_usize(term, total);
                term.print(" (");
                print_usize(term, total * 4);
                term.print(" KiB)\n");
            }

            b"memtest" => {
                term.print("Running heap allocator test...\n");

                // Allocate a variety of sizes.
                let a = crate::memory::heap::kmalloc(64);
                let b = crate::memory::heap::kmalloc(128);
                let c = crate::memory::heap::kmalloc(1024);
                let d = crate::memory::heap::kmalloc(37);

                // Write known patterns to verify no overlap.
                unsafe {
                    core::ptr::write_bytes(a, 0xAA, 64);
                    core::ptr::write_bytes(b, 0xBB, 128);
                    core::ptr::write_bytes(c, 0xCC, 1024);
                    core::ptr::write_bytes(d, 0xDD, 37);
                }

                // Verify patterns.
                let mut ok = true;
                unsafe {
                    for i in 0..64 {
                        if *a.add(i) != 0xAA {
                            ok = false;
                        }
                    }
                    for i in 0..128 {
                        if *b.add(i) != 0xBB {
                            ok = false;
                        }
                    }
                    for i in 0..1024 {
                        if *c.add(i) != 0xCC {
                            ok = false;
                        }
                    }
                    for i in 0..37 {
                        if *d.add(i) != 0xDD {
                            ok = false;
                        }
                    }
                }

                term.print(if ok {
                    "  Pattern check : PASS\n"
                } else {
                    "  Pattern check : FAIL\n"
                });

                // Free and re-allocate to test coalescing.
                crate::heap::kfree(b);
                crate::heap::kfree(c);
                // These two are adjacent — after coalescing one alloc of 1152
                // bytes should fit in the merged hole.
                let e = crate::heap::kmalloc(1152);
                term.print(if !e.is_null() {
                    "  Coalesce test : PASS\n"
                } else {
                    "  Coalesce test : FAIL\n"
                });

                // Clean up.
                crate::heap::kfree(a);
                crate::heap::kfree(d);
                crate::heap::kfree(e);

                term.print("Test complete. Heap dump follows:\n");
                crate::heap::kmalloc_dump(term);
            }

            b"heap" => {
                crate::heap::kmalloc_dump(term);
            }

            b"pgfault" => {
                term.print("Triggering page fault at 0xdeadbeef...\n");
                unsafe {
                    let bad_ptr = 0xdeadbeefu64 as *const u64;
                    let _ = bad_ptr.read_volatile();
                }
            }

            b"divzero" => {
                term.print("Triggering divide-by-zero...\n");
                unsafe {
                    let zero: u64;
                    core::arch::asm!("xor edx, edx", out("edx") zero, options(nomem, nostack));
                    let a = 10u64;
                    let _ = a / zero;
                }
            }

            b"intopcode" => {
                term.print("Triggering invalid opcode (ud2)...\n");
                unsafe {
                    core::arch::asm!("ud2");
                }
            }

            b"breakpoint" => {
                term.print("Triggering breakpoint (int3)...\n");
                unsafe {
                    core::arch::asm!("int3");
                }
                term.print("Returned from breakpoint handler.\n");
            }

            b"dfault" => {
                term.print("Triggering double fault (recursive stack overflow)...\n");
                unsafe {
                    core::arch::asm!("call {f}", f = sym recurse_forever);
                }
            }

            b"uptime" => {
                let ticks = crate::interrupt::pit::ticks();
                let secs = ticks / crate::interrupt::pit::TICKS_PER_SECOND as u64;
                term.print("Uptime: ");
                print_usize(term, secs as usize);
                term.print("s (");
                print_usize(term, ticks as usize);
                term.print(" ticks)\n");
            }

            b"sleep" => {
                let (secs_s, _) = split_first_word(trim_ascii(rest));
                let secs: u64 = parse_decimal(secs_s).unwrap_or(1) as u64;
                term.print("Sleeping ");
                print_usize(term, secs as usize);
                term.print("s...\n");
                crate::multitask::scheduler::sleep_ms(secs * 1000);
                term.print("Awake.\n");
            }

            b"synwrite" => {
                let text = trim_ascii(rest);
                term.print("via syscall: ");
                crate::interrupt::syscall::sys_write_wrapper(1, text.as_ptr(), text.len());
                term.putc('\n');
            }

            _ => {
                term.print("Unknown command: ");
                for &b in verb {
                    term.putc(b as char);
                }
                term.print("\nType 'help' for a list of commands.\n");
            }
        }
    }
}

pub fn run_shell() {
    serial::print("[Shell] started");

    let mut term = Terminal::new().expect("framebuffer must exist");
    let mut shell = Shell::new();

    term.print("Kernel shell ready. Type 'help' for commands.\n\n");
    shell.show_prompt(&mut term);

    loop {
        while let Some(key) = crate::device::keyboard::global_pop() {
            shell.handle_key(&mut term, key);
        }
        crate::multitask::scheduler::yield_cpu();
    }
}

// ── no_std string helpers ────────────────────────────────────────────────────

fn trim_ascii(s: &[u8]) -> &[u8] {
    let s = match s.iter().position(|&b| b != b' ') {
        Some(i) => &s[i..],
        None => return &[],
    };
    match s.iter().rposition(|&b| b != b' ') {
        Some(i) => &s[..=i],
        None => s,
    }
}

fn split_first_word(s: &[u8]) -> (&[u8], &[u8]) {
    match s.iter().position(|&b| b == b' ') {
        Some(i) => (&s[..i], &s[i + 1..]),
        None => (s, &[]),
    }
}

fn parse_hex6(s: &[u8]) -> Option<u32> {
    if s.is_empty() || s.len() > 6 {
        return None;
    }
    let mut val: u32 = 0;
    for &b in s {
        let nib = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => return None,
        };
        val = (val << 4) | nib as u32;
    }
    Some(val)
}

fn parse_decimal(s: &[u8]) -> Option<u32> {
    if s.is_empty() {
        return None;
    }
    let mut val: u32 = 0;
    for &b in s {
        if !b.is_ascii_digit() {
            return None;
        }
        val = val.checked_mul(10)?.checked_add((b - b'0') as u32)?;
    }
    Some(val)
}

/// Recurses without limit until the guard page below the stack faults.
/// `#[inline(never)]` plus a volatile local stops the compiler from
/// turning this into a tail call (which would never grow the stack).
#[inline(never)]
extern "C" fn recurse_forever() {
    let mut sink = [0u8; 64];
    unsafe {
        core::ptr::write_volatile(&mut sink[0], 1);
    }
    recurse_forever();
}

fn print_usize(term: &mut Terminal, mut v: usize) {
    if v == 0 {
        term.putc('0');
        return;
    }
    let mut buf = [b'0'; 20];
    let mut i = 20;
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    for &b in &buf[i..] {
        term.putc(b as char);
    }
}
