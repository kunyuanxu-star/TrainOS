# crt0.s - C runtime startup for TrainOS
# RISC-V 64-bit

.section .text.start
.global _start
.type _start, @function

_start:
    # Initialize stack pointer
    la sp, _stack_top

    # Clear BSS
    la a0, _bss_start
    la a1, _bss_end
    beq a0, a1, clear_done

clear_loop:
    sd zero, 0(a0)
    add a0, a0, 8
    blt a0, a1, clear_loop

clear_done:

    # Initialize global pointer
    .option push
    .option norelax
    la gp, __global_pointer$
    .option pop

    # Call constructors
    call __libc_init_array

    # Get argc and argv
    # In RISC-V, argc is in a0, argv pointer in a1
    # after our syscall return convention, they're in the same places
    mv s0, a0    # argc
    mv s1, a1    # argv

    # Call main
    mv a0, s0    # argc
    mv a1, s1    # argv
    call main

    # Exit with main's return code
    mv a0, a0
    call exit

    # Should not reach here
loop:
    wfi
    j loop

.size _start, .-_start

# Provide a default main if user doesn't define one
.section .text.startup
.global _start_main
.type _start_main, @function
_start_main:
    # Call actual main if it exists
    .weak main
    call main
    # Exit
    mv a0, a0
    tail exit

.size _start_main, .-_start_main

# Exit implementation
.section .text
.global exit
.type exit, @function
exit:
    # Call destructors
    .weak __libc_fini_array
    call __libc_fini_array

    # Call _exit
    tail _exit

.size exit, .-exit

# atexit support
.section .data
.global __atexit
.type __atexit, @object
__atexit:
    .quad 0

# Weak aliases for handlers
.weak __libc_init_array
.weak __libc_fini_array

# Minimal atexit/atexit handlers
.section .text.libc
.global __libc_init_array
.type __libc_init_array, @function
__libc_init_array:
    ret

.global __libc_fini_array
.type __libc_fini_array, @function
__libc_fini_array:
    ret
