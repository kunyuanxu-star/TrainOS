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
core::arch::global_asm!(
    ".section .text.trap,\"ax\",@progbits",
    ".globl __trap_entry",
    ".align 4",
    "__trap_entry:",
    // Save all general purpose registers (except sp which we'll handle specially)
    "    addi sp, sp, -256",
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
    // Save sepc at offset 240, sstatus at offset 248 (matches TrapFrame struct)
    "    csrr t0, sepc",
    "    sd t0, 240(sp)",
    "    csrr t0, sstatus",
    "    sd t0, 248(sp)",
    // Call the Rust trap handler with sp as argument (pointer to trap frame)
    "    mv a0, sp",
    "    call handle_trap",
    // Restore registers (note: sepc was saved at 240, sstatus at 248)
    "    ld t0, 248(sp)",
    "    csrw sstatus, t0",
    "    ld t0, 240(sp)",
    "    csrw sepc, t0",
    "    ld ra, 0(sp)",
    "    ld gp, 8(sp)",
    "    ld tp, 16(sp)",
    "    ld t0, 24(sp)",
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

    // Enable Sv39 MMU with expanded identity mapping (0x80000000-0x80090000)
    // This covers both page tables and kernel code
    crate::memory::Sv39::enable_sv39();
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
    crate::smp::init();

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

    // Output "Boot 5" directly (before trap init to debug hang)
    unsafe {
        let s = "Boot 5\r\n";
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

    // Initialize trap handling
    crate::trap::init();

    // Initialize file system
    crate::fs::init();

    // Output "Boot 6" directly
    unsafe {
        let s = "Boot 6\r\n";
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

    // Run the first process
    crate::process::run_first_process();
}
