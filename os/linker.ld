OUTPUT_ARCH(riscv)
ENTRY(_start)

BASE_ADDRESS = 0x80200000;

SECTIONS
{
    .text BASE_ADDRESS : {
        *(.text.entry)
        *(.text)
        *(.text.*)
    }

    /* Place rodata right after text to keep within PC-relative range */
    .rodata (BASE_ADDRESS + 0x100000) : {
        *(.rodata)
        *(.rodata.*)
        *(.srodata)
    }

    .data (BASE_ADDRESS + 0x200000) : {
        *(.data)
        *(.data.*)
    }

    .bss.uninit (BASE_ADDRESS + 0x300000) : {
        *(.bss.uninit)
    }

    .bss (BASE_ADDRESS + 0x400000) : {
        *(.bss)
        *(.bss.*)
        *(.sbss)
    }

    PROVIDE(end = BASE_ADDRESS + 0x500000);
}
