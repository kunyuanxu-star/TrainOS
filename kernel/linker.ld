OUTPUT_ARCH(riscv)
ENTRY(_start)

BASE_ADDRESS = 0x80200000;

SECTIONS
{
    . = BASE_ADDRESS;
    .text : {
        *(.text.entry)
        *(.text)
        *(.text.*)
    }

    . = ALIGN(4096);
    .rodata : {
        *(.rodata)
        *(.rodata.*)
    }

    . = ALIGN(4096);
    .data : {
        *(.data)
        *(.data.*)
    }

    . = ALIGN(4096);
    .bss : {
        PROVIDE(_bss_start = .);
        *(.bss)
        *(.bss.*)
        *(.sbss)
        PROVIDE(_bss_end = .);
    }

    . = ALIGN(4096);
    PROVIDE(_end = .);
    PROVIDE(_kernel_end = .);

    /DISCARD/ : {
        *(.debug_abbrev)
        *(.debug_info)
        *(.debug_line)
        *(.debug_str)
        *(.debug_ranges)
        *(.debug_aranges)
        *(.comment)
    }
}
