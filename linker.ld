OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start)

KERNEL_VMA = 0xFFFFFFFF80000000;   
KERNEL_LMA = 0x200000;          

SECTIONS {
    . = KERNEL_VMA + KERNEL_LMA;
    
    .text ALIGN(4K) : AT(KERNEL_LMA) {
        KEEP(*(.text._start))      
        *(.text .text.*)
    }
    
    .rodata ALIGN(4K) : AT(LOADADDR(.text) + SIZEOF(.text)) {
        *(.rodata .rodata.*)
    }
    
    .data ALIGN(4K) : AT(LOADADDR(.rodata) + SIZEOF(.rodata)) {
        *(.data .data.*)
    }
    
    .bss ALIGN(4K) : AT(LOADADDR(.data) + SIZEOF(.data)) {
        *(COMMON)
        *(.bss .bss.*)
    }
    
    .limine_requests ALIGN(4K) : AT(LOADADDR(.bss) + SIZEOF(.bss)) {
        KEEP(*(.limine_requests))
        KEEP(*(.limine_requests_start))
        KEEP(*(.limine_requests_end))
    }
    
    
    /DISCARD/ : {
        *(.eh_frame)
        *(.eh_frame_hdr)     /* This was the problem! */
        *(.note .note.*)
        *(.comment)
        *(.dynamic)
        *(.dynsym)
        *(.dynstr)
        *(.gnu.hash)
        *(.hash)
        *(.got)
        *(.got.plt)
        *(.plt)
    }
}
