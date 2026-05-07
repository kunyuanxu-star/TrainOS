core::arch::global_asm!(
    ".section .text.trap, \"ax\", @progbits",
    ".globl __trap_entry",
    ".align 4",
    "__trap_entry:",
    // Atomically swap sp and sscratch
    "    csrrw sp, sscratch, sp",
    // If sp == 0, we were in kernel mode (sscratch was 0)
    "    bnez sp, 1f",
    // Kernel-mode trap: sscratch holds old kernel_sp, sp=0
    "    csrrw sp, sscratch, sp",  // sp=kernel_sp, sscratch=0
    "    j 2f",
    // User-mode trap: sp=kernel_sp, sscratch=user_sp
    "1:",
    "    csrr t0, sscratch",       // t0 = user_sp
    "    csrw sscratch, zero",     // mark in-kernel
    "2:",
    // Allocate trap frame on kernel stack: 35 * 8 = 280 bytes
    "    addi sp, sp, -280",
    // Save GPRs
    "    sd ra, 0*8(sp)",
    "    sd gp, 1*8(sp)",
    "    sd tp, 2*8(sp)",
    "    sd t0, 3*8(sp)",   // user_sp (or junk for kernel trap)
    "    sd t1, 4*8(sp)",
    "    sd t2, 5*8(sp)",
    "    sd s0, 6*8(sp)",
    "    sd s1, 7*8(sp)",
    "    sd a0, 8*8(sp)",
    "    sd a1, 9*8(sp)",
    "    sd a2, 10*8(sp)",
    "    sd a3, 11*8(sp)",
    "    sd a4, 12*8(sp)",
    "    sd a5, 13*8(sp)",
    "    sd a6, 14*8(sp)",
    "    sd a7, 15*8(sp)",
    "    sd s2, 16*8(sp)",
    "    sd s3, 17*8(sp)",
    "    sd s4, 18*8(sp)",
    "    sd s5, 19*8(sp)",
    "    sd s6, 20*8(sp)",
    "    sd s7, 21*8(sp)",
    "    sd s8, 22*8(sp)",
    "    sd s9, 23*8(sp)",
    "    sd s10, 24*8(sp)",
    "    sd s11, 25*8(sp)",
    "    sd t3, 26*8(sp)",
    "    sd t4, 27*8(sp)",
    "    sd t5, 28*8(sp)",
    "    sd t6, 29*8(sp)",
    // Save CSR values
    "    csrr t1, sepc",
    "    sd t1, 30*8(sp)",
    "    csrr t1, sstatus",
    "    sd t1, 31*8(sp)",
    // user_sp already at offset 3*8, also put at 33*8
    "    ld t1, 3*8(sp)",
    "    sd t1, 33*8(sp)",
    // Save stval
    "    csrr t1, stval",
    "    sd t1, 34*8(sp)",
    // Call Rust trap handler with sp = &TrapFrame
    "    mv a0, sp",
    "    call handle_trap",
    // Restore path
    "    addi t3, sp, 280",       // t3 = original kernel_sp
    "    csrw sscratch, t3",      // sscratch = kernel_sp for next trap
    // Load user_sp
    "    ld t0, 33*8(sp)",
    // Restore sepc
    "    ld t1, 30*8(sp)",
    "    csrw sepc, t1",
    // Restore sstatus
    "    ld t1, 31*8(sp)",
    "    csrw sstatus, t1",
    // Restore GPRs (skip t0 which holds user_sp)
    "    ld ra, 0*8(sp)",
    "    ld gp, 1*8(sp)",
    "    ld tp, 2*8(sp)",
    "    ld t1, 4*8(sp)",
    "    ld t2, 5*8(sp)",
    "    ld s0, 6*8(sp)",
    "    ld s1, 7*8(sp)",
    "    ld a0, 8*8(sp)",
    "    ld a1, 9*8(sp)",
    "    ld a2, 10*8(sp)",
    "    ld a3, 11*8(sp)",
    "    ld a4, 12*8(sp)",
    "    ld a5, 13*8(sp)",
    "    ld a6, 14*8(sp)",
    "    ld a7, 15*8(sp)",
    "    ld s2, 16*8(sp)",
    "    ld s3, 17*8(sp)",
    "    ld s4, 18*8(sp)",
    "    ld s5, 19*8(sp)",
    "    ld s6, 20*8(sp)",
    "    ld s7, 21*8(sp)",
    "    ld s8, 22*8(sp)",
    "    ld s9, 23*8(sp)",
    "    ld s10, 24*8(sp)",
    "    ld s11, 25*8(sp)",
    "    ld t3, 26*8(sp)",
    "    ld t4, 27*8(sp)",
    "    ld t5, 28*8(sp)",
    "    ld t6, 29*8(sp)",
    // Deallocate trap frame
    "    addi sp, sp, 280",
    // Restore user_sp for user-mode return
    "    bnez t0, 5f",
    "    sret",                    // kernel return
    "5:  mv sp, t0",              // user return: sp = user_sp
    "    sret",
);
