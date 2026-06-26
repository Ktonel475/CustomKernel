use crate::pmm::{PAGE_SIZE, alloc_page, free_page};
use crate::vmm::{PTE_NO_EXEC, PTE_PRESENT, PTE_WRITABLE, map_page};

pub const HEAP_START: usize = 0xffff_8001_0000_0000;

const HEAP_MAX_SIZE: usize = 64 * 1024 * 1024;
const MAGIC_FREE: u32 = 0xFEE1_DEAD;
const MAGIC_USED: u32 = 0xCAFE_BABE;

struct BlockHeader {
    size: usize,
    used: u8,
    magic: u32,
    _pad: [u8; 3],
    next: *mut BlockHeader,
}

const HEADER_SIZE: usize = core::mem::size_of::<BlockHeader>();
const ALIGN: usize = 16;

static mut HEAP_HEAD: *mut BlockHeader = core::ptr::null_mut();
static mut HEAP_END: usize = 0;
static mut HEAP_INITED: bool = false;

#[inline]
fn align_up(n: usize) -> usize {
    (n + ALIGN - 1) & !(ALIGN - 1)
}

fn map_fresh_pages(virt: usize, n_pages: usize) {
    for i in 0..n_pages {
        let phys = alloc_page().expect("heap: OOM while expanding - no physical pages left");
        map_page(
            virt + i * PAGE_SIZE,
            phys,
            PTE_PRESENT | PTE_WRITABLE | PTE_NO_EXEC,
        );
    }
}

pub fn kmalloc_init(start: usize, size_bytes: usize) {
    assert!(!unsafe { HEAP_INITED }, "kmalloc_init called twice");
    assert_eq!(start % PAGE_SIZE, 0, "heap start must be page-aligned");
    assert!(size_bytes > HEADER_SIZE + ALIGN, "heap too small");

    let pages = (size_bytes + PAGE_SIZE - 1) / PAGE_SIZE;
    let total = pages * PAGE_SIZE;

    unsafe {
        map_fresh_pages(start, pages);

        let hdr = start as *mut BlockHeader;
        (*hdr) = BlockHeader {
            size: total - HEADER_SIZE,
            used: 0,
            magic: MAGIC_FREE,
            _pad: [0; 3],
            next: core::ptr::null_mut(),
        };

        HEAP_HEAD = hdr;
        HEAP_END = start + total;
        HEAP_INITED = true;
    }
}

pub fn kmalloc(size: usize) -> *mut u8 {
    assert!(
        unsafe { HEAP_INITED },
        "kmalloc called before  kmalloc_init"
    );
    assert!(size > 0, "kmalloc(0) is not supported");

    let needed = align_up(size);
    unsafe {
        let mut cur = HEAP_HEAD;
        while !cur.is_null() {
            let hdr = &mut *cur;

            assert!(
                hdr.magic == MAGIC_FREE || hdr.magic == MAGIC_USED,
                "kmalloc: heap corruption detected (bad magic {:#x})",
                hdr.magic,
            );

            if hdr.used == 0 && hdr.size >= needed {
                let leftover = hdr.size.saturating_sub(needed + HEADER_SIZE + ALIGN);
                if leftover > 0 {
                    let new_hdr_addr = (cur as usize) + HEADER_SIZE + needed;
                    let new_hdr = new_hdr_addr as *mut BlockHeader;
                    (*new_hdr) = BlockHeader {
                        size: hdr.size - needed - HEADER_SIZE,
                        used: 0,
                        magic: MAGIC_FREE,
                        _pad: [0; 3],
                        next: hdr.next,
                    };
                    hdr.next = new_hdr;
                    hdr.size = needed;
                }
                hdr.used = 1;
                hdr.magic = MAGIC_USED;
                return (cur as *mut u8).add(HEADER_SIZE);
            }
            cur = hdr.next;
        }
    }
    expand_heap(needed);
    kmalloc(size)
}

fn expand_heap(needed: usize) {
    unsafe {
        let extra_bytes = align_up(needed + HEADER_SIZE + PAGE_SIZE);
        let extra_pages = extra_bytes / PAGE_SIZE;

        assert!(
            HEAP_END + extra_bytes - HEAP_START <= HEAP_MAX_SIZE,
            "heap: exceeded maximum size ({} MiB)",
            HEAP_MAX_SIZE / (1024 * 1024)
        );

        map_fresh_pages(HEAP_END, extra_pages);
        let new_hdr = HEAP_END as *mut BlockHeader;
        (*new_hdr) = BlockHeader {
            size: extra_pages * PAGE_SIZE - HEADER_SIZE,
            used: 0,
            magic: MAGIC_FREE,
            _pad: [0; 3],
            next: core::ptr::null_mut(),
        };

        let mut cur = HEAP_HEAD;
        while !(*cur).next.is_null() {
            cur = (*cur).next;
        }
        (*cur).next = new_hdr;

        HEAP_END += extra_pages * PAGE_SIZE;
    }
}

pub fn kfree(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    assert!(unsafe { HEAP_INITED }, "kfree called before kmalloc_init");

    unsafe {
        let hdr = &mut *((ptr as usize - HEADER_SIZE) as *mut BlockHeader);

        assert!(
            hdr.magic != MAGIC_FREE,
            "kfree: double free detected at {:p}",
            ptr
        );
        assert!(
            hdr.magic == MAGIC_USED,
            "kfree: invalid pointer or heap corruption at {:p} (magic={:#x})",
            ptr,
            hdr.magic,
        );

        hdr.used = 0;
        hdr.magic = MAGIC_FREE;

        coalesce(hdr as *mut BlockHeader);
    }
}

pub fn kmalloc_dump(term: &mut crate::ui::terminal::Terminal) {
    term.print("\n---- Heap Dump ----\n");
    unsafe {
        let mut cur = HEAP_HEAD;
        let mut idx = 0usize;
        while !cur.is_null() {
            let hdr = &*cur;
            let status = if hdr.used != 0 { "USED" } else { "free" };
            term.print("[");
            term.print_usize(idx);
            term.print(" ] addr=");
            print_hex_usize(term, cur as usize);
            term.print(" size=");
            term.print_usize(hdr.size);
            term.print(" ");
            term.print(status);
            term.print("\n");
            cur = hdr.next;
            idx += 1;
        }
    }
    term.print("----- end -----\n");
}

fn coalesce(cur: *mut BlockHeader) {
    unsafe {
        let next = (*cur).next;
        if next.is_null() {
            return;
        }
        if (*next).used != 0 {
            return;
        }

        (*cur).size += HEADER_SIZE + (*next).size;
        (*cur).next = (*next).next;
    }
}

fn print_hex_usize(term: &mut crate::ui::terminal::Terminal, mut v: usize) {
    term.print("0x");
    let mut buf = [b'0'; 16];
    for j in (0..16).rev() {
        let nib = (v & 0xf) as u8;
        buf[j] = if nib < 10 {
            b'0' + nib
        } else {
            b'a' + nib - 10
        };
        v >>= 4;
    }
    for &b in &buf {
        term.putc(b as char);
    }
}
