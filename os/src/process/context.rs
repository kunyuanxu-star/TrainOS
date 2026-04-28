//! Context Switching Implementation
//!
//! Provides task context save/restore for RISC-V

/// Flag indicating whether MMU is available and enabled
/// This is set by enable_sv39() after attempting to enable MMU
pub static MMU_ENABLED: spin::Mutex<bool> = spin::Mutex::new(false);

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
/// a0 = trap frame pointer (unused, for debug compatibility)
/// a1 = new satp value
/// a2 = new sp
/// a3 = new pc (sepc)
core::arch::global_asm!(
    ".globl return_to_user_asm",
    ".type return_to_user_asm, @function",
    "return_to_user_asm:",
    // Set sscratch to a0 (trap frame pointer)
    "   csrw sscratch, a0",
    // Set new page table (satp)
    "   csrw satp, a1",
    // Flush TLB
    "   sfence.vma zero, zero",
    // Set up sepc to the user program counter
    "   csrw sepc, a3",
    // Set sstatus: SPP=0 (user mode), SPIE=1, SIE=0
    "   li t0, 0x00000020",
    "   csrw sstatus, t0",
    // Set up sp to user stack
    "   mv sp, a2",
    // Return to user mode
    "   sret",
    // If sret returns (shouldn't happen), loop
    "1: j 1b"
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

/// Return to user mode (without MMU - physical addressing)
/// NOTE: Without MMU, user programs cannot run because their virtual
/// addresses don't match physical addresses. This function acknowledges
/// the limitation and reports that user mode cannot be entered.
#[inline(never)]
pub unsafe fn return_to_user_no_mmu(_tf: *mut TrapFrame, _satp: usize, _sp: usize, _pc: usize) -> ! {
    crate::println!("[return_to_user_no_mmu] ERROR: Cannot return to user mode without MMU!");
    crate::print!("[return_to_user_no_mmu] User VA = 0x");
    crate::console::print_hex(_pc);
    crate::println!(" (not equal to PA without MMU)");
    crate::print!("[return_to_user_no_mmu] User stack VA = 0x");
    crate::console::print_hex(_sp);
    crate::println!(" (not accessible as PA)");
    crate::println!("[return_to_user_no_mmu] QEMU 10.2.2 has a bug: csrw satp hangs");
    crate::println!("[return_to_user_no_mmu] Without MMU, user programs cannot run");
    crate::println!("[return_to_user_no_mmu] System halted");
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// Return to user mode (with MMU - virtual addressing)
/// Sets up sscratch and switches to user page table, then sret to user mode.
/// a0 = trap frame pointer (kernel_sp - 256, or just kernel_sp)
/// a1 = satp value
/// a2 = user sp
/// a3 = user pc (sepc)
#[inline(never)]
unsafe fn return_to_user_with_mmu(_tf: *mut TrapFrame, satp: usize, sp: usize, pc: usize) {
    // Print entry debug
    for c in b"[rtu] Starting user mode at pc=0x" { crate::console::sbi_console_putchar_raw(*c as usize); }
    print_hex(pc);
    for c in b", sp=0x" { crate::console::sbi_console_putchar_raw(*c as usize); }
    print_hex(sp);
    for c in b"\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Reverting debug: use the original user page table and sret to user mode
    unsafe {
        core::arch::asm!(
            // Switch to user page table
            "csrw satp, {satp}",
            "sfence.vma zero, zero",
            "fence.i",
            // Set sepc to user entry point
            "csrw sepc, {pc}",
            // Set sstatus for user mode: SPP=0 (user), SPIE=1
            "li t0, 0x20",
            "csrw sstatus, t0",
            // Set sscratch = kernel_sp (trap frame pointer) for csrrw at next trap entry
            "csrw sscratch, {ksp}",
            // Set sp to user stack
            "mv sp, {usp}",
            // Return to user mode
            "sret",
            satp = in(reg) satp,
            pc = in(reg) pc,
            usp = in(reg) sp,
            ksp = in(reg) _tf,
            out("t0") _,
            options(nostack),
        );
    }

    // Should never reach here
    for c in b"[rtu] ERROR: sret returned!\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    loop {}
}

fn print_hex(value: usize) {
    let hex = b"0123456789abcdef";
    let mut started = false;
    for i in (0..16).rev() {
        let nibble = (value >> (i * 4)) & 0xf;
        if nibble != 0 || started || i == 0 {
            started = true;
            crate::console::sbi_console_putchar_raw(hex[nibble as usize] as usize);
        }
    }
}

/// Return to user mode
/// # Safety
/// This function switches to user mode and should only be called after
/// proper setup of the trap frame and page table.
#[inline(never)]
pub unsafe fn return_to_user(tf: *mut TrapFrame, satp: usize, sp: usize, pc: usize) {
    let mmu_enabled = *MMU_ENABLED.lock();
    if mmu_enabled {
        return_to_user_with_mmu(tf, satp, sp, pc);
    } else {
        return_to_user_no_mmu(tf, satp, sp, pc);
    }
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
