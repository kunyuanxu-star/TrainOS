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
    "    sfence.vma zero, zero",
    "    fence.i",
    "    ret",
);

core::arch::global_asm!(
    ".section .text.trap,\"ax\",@progbits",
    ".globl __trap_entry",
    ".align 4",
    "__trap_entry:",
    // Swap sp and sscratch atomically
    // Before: sp = user_sp, sscratch = kernel_sp (for user traps)
    //         sp = kernel_sp, sscratch = 0 (for kernel traps)
    // After:  sp = kernel_sp, sscratch = user_sp (for user traps)
    //         sp = 0, sscratch = kernel_sp (for kernel traps - need fixup)
    "    csrrw sp, sscratch, sp",
    // Check if from kernel mode (sp == 0 means sscratch was 0)
    "    bnez sp, 1f",
    // From kernel mode: sscratch has old kernel_sp, sp is 0
    "    csrrw sp, sscratch, sp",  // sp = kernel_sp, sscratch = 0
    "    j 2f",
    // From user mode: sp = kernel_sp, sscratch = user_sp (already correct)
    "1:",
    // Save user_sp from sscratch to t0 before we clear sscratch
    "    csrr t0, sscratch",       // t0 = user_sp
    "    csrw sscratch, zero",     // mark kernel mode
    "2:",
    // Allocate trap frame on kernel stack
    "    addi sp, sp, -256",
    // Save all registers (t0 at offset 24 holds user_sp or 0)
    "    sd ra, 0(sp)",
    "    sd gp, 8(sp)",
    "    sd tp, 16(sp)",
    "    sd t0, 24(sp)",  // user_sp (or 0 for kernel traps)
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
    // Save user_sp at offset 252 (from t0 saved at offset 24)
    "    ld t1, 24(sp)",
    "    sd t1, 252(sp)",
    // Call the Rust trap handler with sp as argument (pointer to trap frame)
    "    mv a0, sp",
    "    call handle_trap",
    // Prepare for trap return
    // At this point sp = trap frame base (kernel_sp - 256)
    // We need to set up sscratch properly for the next trap
    // sstatus SPP bit (bit 8): 0 = returning to user, 1 = returning to supervisor
    "    ld t1, 248(sp)",        // t1 = saved sstatus
    "    andi t2, t1, 0x100",    // t2 = SPP bit (0x100 if supervisor, 0 if user)
    "    addi t3, sp, 256",      // t3 = original kernel_sp
    "    beqz t2, 3f",           // SPP == 0 => returning to user mode
    // Returning to supervisor mode: sscratch = 0
    "    csrw sscratch, zero",
    "    j 4f",
    // Returning to user mode: sscratch = kernel_sp (for csrrw at next trap entry)
    "3:  csrw sscratch, t3",
    "4:",
    // Load user_sp from trap frame at offset 252
    "    ld t0, 252(sp)",
    // Restore sepc
    "    ld t1, 240(sp)",
    "    csrw sepc, t1",
    // Restore sstatus (must be last CSR before sret)
    "    ld t1, 248(sp)",
    "    csrw sstatus, t1",
    // Restore all GP registers (except t0 which holds user_sp)
    "    ld ra, 0(sp)",
    "    ld gp, 8(sp)",
    "    ld tp, 16(sp)",
    "    ld t1, 32(sp)",
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
    // Deallocate trap frame and set correct sp for return
    // For user mode: sp = user_sp, sscratch = kernel_sp (set above)
    // For kernel mode: sp = kernel_sp, sscratch = 0 (set above)
    "    addi sp, sp, 256",      // sp = original kernel_sp
    // Check again if returning to user: if t0 != 0, set sp = user_sp
    // t0 holds user_sp (0 for kernel traps, non-zero for user traps)
    "    bnez t0, 5f",
    "    sret",                   // kernel return: sp = kernel_sp
    "5:  mv sp, t0",             // user return: sp = user_sp
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
