//! Context Switching Implementation
//!
//! Provides task context save/restore for RISC-V

/// Task context structure - saved on context switch
/// This must be 16-byte aligned for proper stack alignment
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TaskContext {
    /// Kernel stack pointer
    pub ra: usize,
    /// sp is saved at offset 8
    pub sp: usize,
    /// s0-s11 saved registers
    pub s0: usize,
    pub s1: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
}

impl TaskContext {
    /// Create a new task context
    pub fn new(ra: usize, sp: usize) -> Self {
        Self {
            ra,
            sp,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
        }
    }
}

/// Task frame - saved on trap (exception/interrupt)
/// This is pushed onto the stack by the hardware/trap entry
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    /// General purpose registers (x0-x31)
    pub ra: usize,
    pub sp: usize,
    pub gp: usize,
    pub tp: usize,
    pub t0: usize,
    pub t1: usize,
    pub t2: usize,
    pub s0: usize,
    pub s1: usize,
    pub a0: usize,
    pub a1: usize,
    pub a2: usize,
    pub a3: usize,
    pub a4: usize,
    pub a5: usize,
    pub a6: usize,
    pub a7: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
    pub t3: usize,
    pub t4: usize,
    pub t5: usize,
    pub t6: usize,
    /// CSR registers
    pub sepc: usize,
    pub sstatus: usize,
}

impl TrapFrame {
    /// Create a trap frame for initial user mode entry
    pub fn new_user_entry(pc: usize, sp: usize, a0: usize) -> Self {
        Self {
            ra: 0,
            sp,
            gp: 0,
            tp: 0,
            t0: 0,
            t1: 0,
            t2: 0,
            s0: 0,
            s1: 0,
            a0,
            a1: 0,
            a2: 0,
            a3: 0,
            a4: 0,
            a5: 0,
            a6: 0,
            a7: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
            t3: 0,
            t4: 0,
            t5: 0,
            t6: 0,
            sepc: pc,
            sstatus: 0,
        }
    }
}

/// Size of trap frame in bytes
pub const TRAP_FRAME_SIZE: usize = core::mem::size_of::<TrapFrame>();

// Context switch assembly - placed at module level
core::arch::global_asm!(
    ".globl context_switch",
    ".type context_switch, @function",
    "context_switch:",
    // Save callee-saved registers
    "    sd ra, 0(a0)",
    "    sd sp, 8(a0)",
    "    sd s0, 16(a0)",
    "    sd s1, 24(a0)",
    "    sd s2, 32(a0)",
    "    sd s3, 40(a0)",
    "    sd s4, 48(a0)",
    "    sd s5, 56(a0)",
    "    sd s6, 64(a0)",
    "    sd s7, 72(a0)",
    "    sd s8, 80(a0)",
    "    sd s9, 88(a0)",
    "    sd s10, 96(a0)",
    "    sd s11, 104(a0)",
    // Load new task's context
    "    ld ra, 0(a1)",
    "    ld sp, 8(a1)",
    "    ld s0, 16(a1)",
    "    ld s1, 24(a1)",
    "    ld s2, 32(a1)",
    "    ld s3, 40(a1)",
    "    ld s4, 48(a1)",
    "    ld s5, 56(a1)",
    "    ld s6, 64(a1)",
    "    ld s7, 72(a1)",
    "    ld s8, 80(a1)",
    "    ld s9, 88(a1)",
    "    ld s10, 96(a1)",
    "    ld s11, 104(a1)",
    // Return to new task
    "    ret",
);

/// Assembly for returning to user mode via sret
/// This switches to the new page table and returns to user mode
core::arch::global_asm!(
    ".globl return_to_user",
    ".type return_to_user, @function",
    "return_to_user:",
    // a0 = trap frame pointer
    // a1 = new satp value
    // a2 = new sp
    // a3 = new pc (sepc)
    // Save kernel sp to t0
    "   mv t0, sp",
    // Set sscratch to trap frame pointer (kernel stack)
    // This is needed so that when a trap occurs in user mode,
    // the CPU can find the kernel stack by exchanging sp and sscratch
    "   mv t1, a0",
    // Set new page table (satp)
    "   csrw satp, a1",
    // Flush TLB
    "   sfence.vma zero, zero",
    // Set sscratch to kernel trap frame pointer
    // When trap occurs in user mode, CPU exchanges sp with sscratch
    "   csrw sscratch, t1",
    // Set up sepc to the user program counter
    "   csrw sepc, a3",
    // Set up sp to user stack
    "   mv sp, a2",
    // Restore trap frame registers
    // TrapFrame layout: ra(0), sp(8), gp(16), tp(24), t0(32), t1(40), t2(48),
    // s0(56), s1(64), a0(72), a1(80), a2(88), a3(96), a4(104), a5(112),
    // a6(120), a7(128), s2(136), s3(144), s4(152), s5(160), s6(168),
    // s7(176), s8(184), s9(192), s10(200), s11(208), t3(216), t4(224),
    // t5(232), t6(240), sepc(248), sstatus(256)
    "   ld ra, 0(a0)",
    "   ld gp, 16(a0)",
    "   ld tp, 24(a0)",
    "   ld t0, 32(a0)",
    "   ld t1, 40(a0)",
    "   ld t2, 48(a0)",
    "   ld s0, 56(a0)",
    "   ld s1, 64(a0)",
    "   ld a0, 72(a0)",
    "   ld a1, 80(a0)",
    "   ld a2, 88(a0)",
    "   ld a3, 96(a0)",
    "   ld a4, 104(a0)",
    "   ld a5, 112(a0)",
    "   ld a6, 120(a0)",
    "   ld a7, 128(a0)",
    "   ld s2, 136(a0)",
    "   ld s3, 144(a0)",
    "   ld s4, 152(a0)",
    "   ld s5, 160(a0)",
    "   ld s6, 168(a0)",
    "   ld s7, 176(a0)",
    "   ld s8, 184(a0)",
    "   ld s9, 192(a0)",
    "   ld s10, 200(a0)",
    "   ld s11, 208(a0)",
    "   ld t3, 216(a0)",
    "   ld t4, 224(a0)",
    "   ld t5, 232(a0)",
    "   ld t6, 240(a0)",
    // Restore original sp (kernel sp) to t0 for now, but sp is already set to user sp above
    // Set sstatus: SPP=0 (user mode), SPIE=1, SIE=0
    // SPP is bit 8, SPIE is bit 5
    "   li t0, 0x00000020",
    "   csrw sstatus, t0",
    // Return to user mode
    "   sret",
);

/// Switch from one task to another
/// a0 = pointer to old TaskContext (saves current state)
/// a1 = pointer to new TaskContext (restores new state)
#[inline(always)]
pub unsafe fn context_switch(old_ctx: *mut TaskContext, new_ctx: *const TaskContext) {
    // Call the assembly function defined in global_asm
    extern "C" {
        fn context_switch(old_ctx: *mut TaskContext, new_ctx: *const TaskContext);
    }
    context_switch(old_ctx, new_ctx);
}

/// Initialize a new task's context for first run
pub fn init_task_context(ctx: &mut TaskContext, entry: usize, sp: usize) {
    ctx.ra = entry;
    ctx.sp = sp;
}

/// Prepare a task to return to user mode
pub fn prepare_trap_frame(tf: &mut TrapFrame, pc: usize, sp: usize, a0: usize) {
    tf.sepc = pc;
    tf.sp = sp;
    tf.a0 = a0;
    // SPP = 0 (user mode), SPIE = 1, SIE = 0
    tf.sstatus = 0x00000020;
}

/// Return to user mode
/// # Safety
/// This function switches to user mode and should only be called after
/// proper setup of the trap frame and page table.
#[inline(always)]
pub unsafe fn return_to_user(tf: *mut TrapFrame, satp: usize, sp: usize, pc: usize) {
    core::arch::asm!(
        "call return_to_user",
        in("a0") tf,
        in("a1") satp,
        in("a2") sp,
        in("a3") pc,
    );
}

/// Switch page table (satp)
#[inline(always)]
pub fn switch_page_table(satp: usize) {
    unsafe {
        core::arch::asm!(
            "csrw satp, {0}",
            "sfence.vma zero, zero",
            in(reg) satp,
        );
    }
}
