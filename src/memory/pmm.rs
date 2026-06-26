use limine::memmap::MEMMAP_USABLE;
use limine::request::MemmapRequest;

#[used]
#[unsafe(link_section = ".requests")]
static MMAP_REQUEST: MemmapRequest = MemmapRequest::new();

pub const PAGE_SIZE: usize = 4096;

static mut BITMAP_PTR: *mut u8 = core::ptr::null_mut();
static mut BITMAP_LEN: usize = 0;
static mut TOTAL_PAGES: usize = 0;
static mut FREE_COUNT: usize = 0;
static mut USABLE_PAGE: usize = 0;

#[inline]
fn set_used(page: usize) {
    let byte = page / 8;
    let bit = page % 8;
    unsafe {
        *BITMAP_PTR.add(byte) |= 1 << bit;
    }
}

#[inline]
fn set_free(page: usize) {
    let byte = page / 8;
    let bit = page % 8;
    unsafe {
        *BITMAP_PTR.add(byte) &= !(1 << bit);
    }
}

#[inline]
fn is_used(page: usize) -> bool {
    let byte = page / 8;
    let bit = page % 8;
    unsafe { *BITMAP_PTR.add(byte) & (1 << bit) != 0 }
}

pub fn init_pmm() {
    let resp = MMAP_REQUEST
        .response()
        .expect("PMM: Limine did not provide a memory map");
    let entries = resp.entries();

    let mut highest_addr: u64 = 0;
    for entry in entries.iter() {
        let end = entry.base + entry.length;
        if end > highest_addr {
            highest_addr = end;
        }
    }

    assert!(highest_addr > 0, "PMM: memory map is empty");

    let total_pages = (highest_addr as usize + PAGE_SIZE - 1) / PAGE_SIZE;
    let bitmap_bytes = (total_pages + 7) / 8;
    let bitmap_pages = (bitmap_bytes + PAGE_SIZE - 1) / PAGE_SIZE;

    let mut bitmap_phys: usize = 0;
    for entry in entries.iter() {
        if entry.type_ != MEMMAP_USABLE {
            continue;
        }
        if entry.length as usize >= bitmap_bytes {
            let base = entry.base as usize;
            let aligned = (base + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
            if (entry.base + entry.length) as usize >= aligned + bitmap_bytes {
                bitmap_phys = aligned;
                break;
            }
        }
    }

    assert!(
        bitmap_phys != 0,
        "PMM: no single usable region is large enough for the bitmap \
        ({} bytes needed)",
        bitmap_bytes
    );

    let hhdm = crate::vmm::hhdm_offset() as usize;
    let bitmap_virt = bitmap_phys + hhdm;

    unsafe {
        BITMAP_PTR = bitmap_virt as *mut u8;
        BITMAP_LEN = bitmap_bytes;
        TOTAL_PAGES = total_pages;

        core::ptr::write_bytes(BITMAP_PTR, 0xff, bitmap_bytes);
    }

    let bitmap_end_phys = bitmap_phys + bitmap_pages * PAGE_SIZE;

    for entry in entries.iter() {
        if entry.type_ != MEMMAP_USABLE {
            continue;
        }

        let region_start = entry.base as usize;
        let region_end = (entry.base + entry.length) as usize;

        let first_page = (region_start + PAGE_SIZE - 1) / PAGE_SIZE;
        let last_page = region_end / PAGE_SIZE;

        for page in first_page..last_page {
            let phys = page * PAGE_SIZE;

            if phys == 0 {
                continue;
            }

            if phys < 0x10_0000 {
                continue;
            }

            if phys >= bitmap_phys && phys < bitmap_end_phys {
                continue;
            }

            if page < unsafe { TOTAL_PAGES } {
                unsafe {
                    set_free(page);
                    FREE_COUNT += 1;
                    USABLE_PAGE += 1;
                }
            }
        }
    }
}

pub fn alloc_page() -> Option<usize> {
    unsafe {
        for byte_idx in 0..BITMAP_LEN {
            let byte = *BITMAP_PTR.add(byte_idx);
            if byte == 0xff {
                continue;
            }

            let bit = byte.trailing_ones() as usize;
            let page = byte_idx * 8 + bit;
            if page >= TOTAL_PAGES {
                break;
            }
            set_used(page);
            FREE_COUNT -= 1;
            return Some(page * PAGE_SIZE);
        }
    }
    None
}

pub fn free_page(phys: usize) {
    assert_eq!(
        phys % PAGE_SIZE,
        0,
        "free_page: Address {:#x} is not page-aligned",
        phys
    );

    let page = phys / PAGE_SIZE;

    assert!(
        page < unsafe { TOTAL_PAGES },
        "free_page: address {:#x} is beyond tracked range ({} pages)",
        phys,
        unsafe { TOTAL_PAGES }
    );

    assert!(
        is_used(page),
        "free_page: double-free detected at physical address {:#3}",
        phys
    );

    unsafe {
        set_free(page);
        FREE_COUNT += 1;
    }
}

pub fn get_free_page_count() -> usize {
    unsafe { FREE_COUNT }
}

pub fn get_usable_page_count() -> usize {
    unsafe { USABLE_PAGE }
}

pub fn get_total_page_count() -> usize {
    unsafe { TOTAL_PAGES }
}

pub fn get_free_memory() -> usize {
    unsafe { FREE_COUNT * PAGE_SIZE }
}

pub fn get_usable_memory() -> usize {
    unsafe { USABLE_PAGE * PAGE_SIZE }
}
