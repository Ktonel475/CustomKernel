use crate::pmm::{PAGE_SIZE, alloc_page};

pub const PTE_PRESENT: u64 = 1 << 0;
pub const PTE_WRITABLE: u64 = 1 << 1;
pub const PTE_USER: u64 = 1 << 2;
pub const PTE_NO_EXEC: u64 = 1 << 63;
pub const PTE_NO_CACHE: u64 = 1 << 4;

const ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;

fn read_cr3() -> u64 {
    let v: u64;
    unsafe {
        core::arch::asm!("mov {}, cr3", out(reg) v, options(nomem, nostack));
    }
    v
}

fn write_cr3(v: u64) {
    unsafe {
        core::arch::asm!("mov cr3, {}", in(reg) v, options(nomem, nostack));
    }
}

pub fn read_cr2() -> u64 {
    let v: u64;
    unsafe {
        core::arch::asm!("mov {}, cr2", out(reg) v, options(nomem, nostack));
    }
    v
}

fn invlpg(virt: usize) {
    unsafe {
        core::arch::asm!("invlpg [{v}]", v = in(reg) virt, options(nomem, nostack));
    }
}

use limine::request::HhdmRequest;

#[used]
#[unsafe(link_section = ".requests")]
static HHDMREQUEST: HhdmRequest = HhdmRequest::new();

static mut HHDM_OFFSET: u64 = 0;

pub fn hhdm_offset() -> u64 {
    unsafe { HHDM_OFFSET }
}

#[inline]
pub fn phys_to_virt(phys: u64) -> *mut u64 {
    (phys + unsafe { HHDM_OFFSET }) as *mut u64
}

fn next_table_or_create(table: *mut u64, idx: usize) -> *mut u64 {
    unsafe {
        let entry = table.add(idx).read_volatile();
        if entry & PTE_PRESENT != 0 {
            phys_to_virt(entry & ADDR_MASK)
        } else {
            let phys = alloc_page().expect("VMM:OOM allocating page table") as u64;
            let virt = phys_to_virt(phys);
            core::ptr::write_bytes(virt, 0, PAGE_SIZE / 8);
            table
                .add(idx)
                .write_volatile(phys | PTE_PRESENT | PTE_WRITABLE);
            virt
        }
    }
}

pub fn init_vmm() {
    unsafe {
        let resp = HHDMREQUEST
            .response()
            .expect("Limine did not provide HHDM offset");
        HHDM_OFFSET = resp.offset;
    }
}

pub fn map_page(virt: usize, phys: usize, flags: u64) {
    assert_eq!(virt % PAGE_SIZE, 0, "map_page: virt not aligned");
    assert_eq!(phys % PAGE_SIZE, 0, "map_page: phys not aligned");

    let (pml4_idx, pdpt_idx, pd_idx, pt_idx) = split_indices(virt);

    let pml4_phys = read_cr3() & ADDR_MASK;
    let pml4 = phys_to_virt(pml4_phys);

    unsafe {
        let pdpt = next_table_or_create(pml4, pml4_idx);
        let pd = next_table_or_create(pdpt, pdpt_idx);
        let pt = next_table_or_create(pd, pd_idx);
        pt.add(pt_idx)
            .write_volatile((phys as u64) | flags | PTE_PRESENT);
        invlpg(virt);
    }
}

pub fn unmap_page(virt: usize) {
    assert_eq!(virt % PAGE_SIZE, 0, "unmap_page: vitr not aligned");

    unsafe {
        if let Some((pt, pt_idx, _e1)) = walk_existing(virt) {
            pt.add(pt_idx).write_volatile(0);
            invlpg(virt);
        }
    }
}

pub fn get_phys_addr(virt: usize) -> Option<usize> {
    let (_pt, _pt_idx, e1) = walk_existing(virt)?;
    if e1 & PTE_PRESENT == 0 {
        return None;
    }
    Some(((e1 & ADDR_MASK) as usize) | (virt & 0xfff))
}

#[inline]
fn split_indices(virt: usize) -> (usize, usize, usize, usize) {
    (
        (virt >> 39) & 0x1ff,
        (virt >> 30) & 0x1ff,
        (virt >> 21) & 0x1ff,
        (virt >> 12) & 0x1ff,
    )
}

fn walk_existing(virt: usize) -> Option<(*mut u64, usize, u64)> {
    let (pml4_idx, pdpt_idx, pd_idx, pt_idx) = split_indices(virt);

    let pml4_phys = read_cr3() & ADDR_MASK;
    let pml4 = phys_to_virt(pml4_phys);

    unsafe {
        let e4 = pml4.add(pml4_idx).read_volatile();
        if e4 & PTE_PRESENT == 0 {
            return None;
        }
        let pdpt = phys_to_virt(e4 & ADDR_MASK);

        let e3 = pdpt.add(pdpt_idx).read_volatile();
        if e3 & PTE_PRESENT == 0 {
            return None;
        }
        let pd = phys_to_virt(e3 & ADDR_MASK);

        let e2 = pd.add(pd_idx).read_volatile();
        if e2 & PTE_PRESENT == 0 {
            return None;
        }
        let pt = phys_to_virt(e2 & ADDR_MASK);

        let e1 = pt.add(pt_idx).read_volatile();
        Some((pt, pt_idx, e1))
    }
}
