OUTPUT_FORMAT("elf64-littleriscv")
OUTPUT_ARCH(riscv:rv64)
ENTRY(_start)

MEMORY {
    RAM (wxa!ri) : ORIGIN = 0x00400000, LENGTH = 128M
}

HEAP_SIZE = 0x1000000;
STACK_SIZE = 0x100000;

SECTIONS {
    .text 0x00400000 : {
        _start = .;
        *(.text.start)
        *(.text)
        *(.rodata)
    } > RAM

    .data : {
        *(.data)
        *(.sdata)
    } > RAM

    .bss (NOLOAD) : {
        _bss_start = .;
        *(.bss)
        *(.sbss)
        _bss_end = .;
    } > RAM

    .heap : {
        _heap_start = .;
        . = . + HEAP_SIZE;
        _heap_end = .;
    } > RAM

    .stack (NOLOAD) : {
        . = . + STACK_SIZE;
        _stack_top = .;
    } > RAM

    _end = .;
}
