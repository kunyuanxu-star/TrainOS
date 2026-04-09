OUTPUT_ARCH(riscv)
ENTRY(_start)

BASE_ADDRESS = 0x80200000;

/* Embedded binaries are large (driver.bin ~741KB, shell.bin ~801KB, fs.bin ~6KB)
 * Total ~1.5MB, so we need more space for .rodata
 * We also need to avoid overlap between sections
 */
SECTIONS
{
    .text BASE_ADDRESS : {
        *(.text.entry)
        *(.text)
        *(.text.*)
    }

    /* Place rodata right after text - expanded to 2MB to hold embedded binaries */
    .rodata (BASE_ADDRESS + 0x100000) : {
        *(.rodata)
        *(.rodata.*)
        *(.srodata)
    }

    /* .data starts after rodata - ensure no overlap with .rodata which can be ~2MB with binaries */
    .data (BASE_ADDRESS + 0x300000) : {
        *(.data)
        *(.data.*)
        . = ALIGN(4096);
    }

    /DISCARD/ : {
        *(.debug_abbrev)
        *(.debug_info)
        *(.debug_line)
        *(.debug_str)
        *(.debug_ranges)
        *(.debug_aranges)
        *(.comment)
    }

    .bss.uninit (BASE_ADDRESS + 0x400000) : {
        *(.bss.uninit)
    }

    .bss (BASE_ADDRESS + 0x500000) : {
        *(.bss)
        *(.bss.*)
        *(.sbss)
    }

    PROVIDE(end = BASE_ADDRESS + 0x600000);
}
