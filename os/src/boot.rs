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
    // Save sp at offset 240, sepc at 248, sstatus at 256 (but we sub 256 so -8)
    "    csrr t0, sepc",
    "    sd t0, 248(sp)",
    "    csrr t0, sstatus",
    "    sd t0, 256(sp)",
    // Call the Rust trap handler with sp as argument (pointer to trap frame)
    "    mv a0, sp",
    "    call handle_trap",
    // Restore registers
    "    ld t0, 256(sp)",
    "    csrw sstatus, t0",
    "    ld t0, 248(sp)",
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
    // Test with println!
    crate::println!("========================================");
    crate::println!("  trainOS is booting!");
    crate::println!("========================================");
    crate::println!("  RISC-V64 Architecture");
    crate::println!("  Sv39 Virtual Memory");
    crate::println!("========================================");
    crate::println!();

    // Initialize memory management
    crate::memory::init();

    // Initialize SMP (multi-core) support
    crate::smp::init();

    // Initialize process management
    crate::process::init();

    // Initialize trap handling
    crate::trap::init();

    // Initialize file system
    crate::fs::init();

    crate::println!("[OK] All subsystems initialized");
    crate::println!();

    // Run the first process
    crate::process::run_first_process();
}
