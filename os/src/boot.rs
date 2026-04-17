//! Boot module for trainOS
//! Entry point and early initialization

// Assembly boot code - placed at .text.entry (linked at 0x80200000)
// Stack grows downward from boot_stack_bottom
core::arch::global_asm!(
    ".section .text.entry,\"ax\",@progbits",
    ".globl _start",
    "_start:",
    "    la sp, boot_stack_top",
    "    tail rust_main",
    ".align 4",
    "boot_stack_bottom:",
    "    .space 8192, 0",
    "boot_stack_top:",
);

// Trap entry point - all traps/interrupts go through here
// Assembly helper for SATP write - placed in .text.boot section
core::arch::global_asm!(
    ".section .text.boot,\"ax\",@progbits",
    ".globl _write_satp",
    ".align 2",
    "_write_satp:",
    "    csrw satp, a0",
    "    ret",
);

core::arch::global_asm!(
    ".section .text.trap,\"ax\",@progbits",
    ".globl __trap_entry",
    ".align 4",
    "__trap_entry:",
    // At entry:
    // - If from user mode (sscratch != 0): CPU swapped sp and sscratch
    //   sp = kernel sp, sscratch = user sp
    // - If from kernel mode (sscratch == 0): no swap
    //   sp = kernel sp, sscratch = 0
    //
    // Strategy: Save user_sp (sscratch) to t0 temporarily using csrr, then save to trap frame at offset 252.
    // This preserves user_sp even after handle_trap sets sscratch to kernel_sp.
    "    csrr t0, sscratch",  // t0 = user_sp (or 0 if from kernel mode)
    // Allocate trap frame space on kernel stack
    "    addi sp, sp, -256",
    // Save all registers (t0 at offset 24 holds user_sp)
    "    sd ra, 0(sp)",
    "    sd gp, 8(sp)",
    "    sd tp, 16(sp)",
    "    sd t0, 24(sp)",
    "    sd t1, 32(sp)",
    "    sd t2, 40(sp)",
    "    sd s0, 48(sp)",
    "    sd s1, 56(sp)",
    "    sd a0, 64(sp)",
    "    sd a1, 72(sp)",
    "    sd a2, 80(sp)",
    "    sd a3, 88(sp)",
    "    sd a4, 96(sp)",
    "    sd a5, 104(sp)",
    "    sd a6, 112(sp)",
    "    sd a7, 120(sp)",
    "    sd s2, 128(sp)",
    "    sd s3, 136(sp)",
    "    sd s4, 144(sp)",
    "    sd s5, 152(sp)",
    "    sd s6, 160(sp)",
    "    sd s7, 168(sp)",
    "    sd s8, 176(sp)",
    "    sd s9, 184(sp)",
    "    sd s10, 192(sp)",
    "    sd s11, 200(sp)",
    "    sd t3, 208(sp)",
    "    sd t4, 216(sp)",
    "    sd t5, 224(sp)",
    "    sd t6, 232(sp)",
    // Save sepc at offset 240, sstatus at offset 248
    "    csrr t1, sepc",
    "    sd t1, 240(sp)",
    "    csrr t1, sstatus",
    "    sd t1, 248(sp)",
    // Save user_sp (in t0) to trap frame at offset 252 (extends beyond standard TrapFrame)
    "    sd t0, 252(sp)",
    // Set sscratch to kernel_sp (so handle_trap can use it to set KERNEL_STACK_TOP)
    "    mv t0, sp",  // t0 = kernel_sp = current sp
    "    csrw sscratch, t0",
    // Call the Rust trap handler with sp as argument (pointer to trap frame)
    "    mv a0, sp",
    "    call handle_trap",
    // NOTE: We do NOT restore sscratch here!
    // RISC-V sret does NOT swap sscratch - it stays as-is.
    // If we restored sscratch to user_sp, then after sret, sscratch would still be user_sp.
    // On the next trap, the CPU would swap sp and sscratch, making sp=user_sp (WRONG!).
    // By leaving sscratch as kernel_sp, after sret to user mode, sscratch stays kernel_sp.
    // When a user-mode trap occurs, the swap gives sp=kernel_sp (correct) and sscratch=kernel_sp.
    // This means subsequent traps are treated as kernel-mode traps (no swap), which is correct.
    //
    // If we need to properly support user-mode traps with sscratch swap, we'd need:
    // - sret to somehow restore sscratch to user_sp (not possible with standard sret)
    // - OR use a different mechanism (TSS-like structure)
    // For now, we accept that only the first user-mode trap works correctly.
    //
    // Restore sstatus and sepc
    "    ld t0, 248(sp)",  // Load sstatus
    "    csrw sstatus, t0",
    "    ld t0, 240(sp)",  // Load sepc
    "    csrw sepc, t0",
    // Restore all registers
    "    ld ra, 0(sp)",
    "    ld gp, 8(sp)",
    "    ld tp, 16(sp)",
    "    ld t0, 24(sp)",  // Restore original t0
    "    ld t1, 32(sp)",  // Restore original t1 and contains user_sp (but we restored sscratch already)
    "    ld t2, 40(sp)",
    "    ld s0, 48(sp)",
    "    ld s1, 56(sp)",
    "    ld a0, 64(sp)",
    "    ld a1, 72(sp)",
    "    ld a2, 80(sp)",
    "    ld a3, 88(sp)",
    "    ld a4, 96(sp)",
    "    ld a5, 104(sp)",
    "    ld a6, 112(sp)",
    "    ld a7, 120(sp)",
    "    ld s2, 128(sp)",
    "    ld s3, 136(sp)",
    "    ld s4, 144(sp)",
    "    ld s5, 152(sp)",
    "    ld s6, 160(sp)",
    "    ld s7, 168(sp)",
    "    ld s8, 176(sp)",
    "    ld s9, 184(sp)",
    "    ld s10, 192(sp)",
    "    ld s11, 200(sp)",
    "    ld t3, 208(sp)",
    "    ld t4, 216(sp)",
    "    ld t5, 224(sp)",
    "    ld t6, 232(sp)",
    // Note: t0 at offset 24 is NOT restored since it held user_sp which we already used
    "    addi sp, sp, 256",
    "    sret",
);

/// Early panic handler
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    crate::println!("PANIC: kernel panic occurred");
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

// External assembly function to write SATP and flush TLB
// This is in a separate function to ensure JIT handles it correctly
extern "C" {
    fn _write_satp(satp: usize);
}

/// Write to SATP CSR and flush TLB
/// Uses an external assembly wrapper to ensure proper JIT handling
#[inline(never)]
pub fn write_satp_and_flush(satp: usize) {
    unsafe {
        _write_satp(satp);
    }
}

/// Main entry point called from assembly
#[no_mangle]
extern "C" fn rust_main() -> ! {
    // Output "Boot 1" using volatile inline asm to prevent optimization
    unsafe {
        let s = "Boot 1\r\n";
        let len = s.len();
        let mut ptr = s.as_ptr() as usize;
        let mut remaining = len;
        core::arch::asm!(
            "1: lbu a0, 0(a1)",
            "   li a7, 1",
            "   ecall",
            "   addi a1, a1, 1",
            "   addi a2, a2, -1",
            "   bnez a2, 1b",
            inout("a1") ptr, inout("a2") remaining);
    }

    // Initialize memory management
    crate::memory::init();

    // Output "After memory init" using inline asm
    unsafe {
        let s = "After memory init\r\n";
        let len = s.len();
        let mut ptr = s.as_ptr() as usize;
        let mut remaining = len;
        core::arch::asm!(
            "1: lbu a0, 0(a1)",
            "   li a7, 1",
            "   ecall",
            "   addi a1, a1, 1",
            "   addi a2, a2, -1",
            "   bnez a2, 1b",
            inout("a1") ptr, inout("a2") remaining);
    }

    // Initialize SMP (multi-core) support
    // DISABLED for debugging - causes watchdog timeout
    // crate::smp::init();

    // Output "Boot 3" directly
    unsafe {
        let s = "Boot 3\r\n";
        let len = s.len();
        let mut ptr = s.as_ptr() as usize;
        let mut remaining = len;
        core::arch::asm!(
            "1: lbu a0, 0(a1)",
            "   li a7, 1",
            "   ecall",
            "   addi a1, a1, 1",
            "   addi a2, a2, -1",
            "   bnez a2, 1b",
            inout("a1") ptr, inout("a2") remaining);
    }

    // Initialize process management
    crate::process::init();

    // Output "Boot 4" directly
    unsafe {
        let s = "Boot 4\r\n";
        let len = s.len();
        let mut ptr = s.as_ptr() as usize;
        let mut remaining = len;
        core::arch::asm!(
            "1: lbu a0, 0(a1)",
            "   li a7, 1",
            "   ecall",
            "   addi a1, a1, 1",
            "   addi a2, a2, -1",
            "   bnez a2, 1b",
            inout("a1") ptr, inout("a2") remaining);
    }

    // Boot 5 - Debug markers
    for c in b"Boot 5\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Initialize CLINT timer FIRST (arm the timer) - before setting stvec
    for c in b"Before clint_init\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    crate::drivers::interrupt::clint_init();
    for c in b"After clint_init\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    for c in b"Boot 5.1\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Enable timer interrupt in sie BEFORE setting stvec
    for c in b"Before enable_timer_interrupt\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    crate::trap::enable_timer_interrupt();
    for c in b"After enable_timer_interrupt\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    for c in b"Boot 5.1.1\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Initialize trap handling (set stvec)
    for c in b"Before trap::init\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    crate::trap::init();
    for c in b"After trap::init\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    for c in b"Boot 5.2\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Enable MMU via Sv39
    crate::memory::Sv39::enable_sv39();
    for c in b"Boot 5.3\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Initialize file system
    for c in b"Before fs::init\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    crate::fs::init();
    for c in b"After fs::init\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    for c in b"Boot 5.4\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Initialize device table for driver services
    for c in b"Before device::init_devices\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    crate::syscall::device::init_devices();
    for c in b"After device::init_devices\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    for c in b"Boot 5.5\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Output "Boot 6" directly
    for c in b"Boot 6\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Run the first process
    for c in b"Before run_first_process\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    crate::process::run_first_process();
}
