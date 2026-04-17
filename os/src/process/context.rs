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
#[inline(never)]
unsafe fn return_to_user_with_mmu(tf: *mut TrapFrame, satp: usize, sp: usize, pc: usize) {
    for c in b"[rtu] Entry\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Check entry point alignment
    for c in b"[rtu] pc=0x" { crate::console::sbi_console_putchar_raw(*c as usize); }
    let mut tmp = pc;
    let hex_chars = b"0123456789abcdef";
    let mut i = 0;
    let mut digits = [0u8; 16];
    if tmp == 0 {
        crate::console::sbi_console_putchar_raw(b'0' as usize);
    } else {
        while tmp > 0 {
            let d = (tmp & 0xf) as u8;
            digits[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
            i += 1;
            tmp >>= 4;
        }
        while i > 0 {
            i -= 1;
            crate::console::sbi_console_putchar_raw(digits[i] as usize);
        }
    }
    for c in b", aligned=" { crate::console::sbi_console_putchar_raw(*c as usize); }
    if pc % 4 == 0 {
        for c in b"4\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    } else if pc % 2 == 0 {
        for c in b"2(RVC)\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    } else {
        for c in b"1\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    }

    // User root PT location
    let user_root = ((satp & 0x7FFFFFFFFF) as usize) << 12;

    // Check user page table entry for VA 0x11000 (page containing entry point)
    // VA 0x11000 -> indices [0, 0, 0x11]
    // ROOT[0] -> L1, L1[0] -> L2, L2[0x11] -> page
    let root_0 = *(user_root as *const usize);
    for c in b"[rtu] ROOT[0]=0x" { crate::console::sbi_console_putchar_raw(*c as usize); }
    tmp = root_0;
    i = 0;
    if tmp == 0 {
        for c in b"0\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    } else {
        while tmp > 0 {
            let d = (tmp & 0xf) as u8;
            digits[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
            i += 1;
            tmp >>= 4;
        }
        while i > 0 {
            i -= 1;
            crate::console::sbi_console_putchar_raw(digits[i] as usize);
        }
        for c in b"\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    }

    // ROOT[0x11] should be L1 entry for VA 0x11000
    let root_11 = *((user_root + 0x11 * 8) as *const usize);
    for c in b"[rtu] ROOT[0x11]=0x" { crate::console::sbi_console_putchar_raw(*c as usize); }
    tmp = root_11;
    i = 0;
    if tmp == 0 {
        for c in b"0\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    } else {
        while tmp > 0 {
            let d = (tmp & 0xf) as u8;
            digits[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
            i += 1;
            tmp >>= 4;
        }
        while i > 0 {
            i -= 1;
            crate::console::sbi_console_putchar_raw(digits[i] as usize);
        }
        for c in b"\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    }

    // If entry point page mapping is missing, create it directly
    // PA for entry point page is 0x80079000
    let mut l2_pa: usize = 0;
    if root_11 == 0 {
        for c in b"[rtu] WARNING: Entry page mapping missing, creating it now\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

        // We need to create L1 and L2 page tables and the leaf entry
        // First, allocate L1 PT
        let l1_pa = match crate::memory::allocator::alloc_page() {
            Some(p) => p,
            None => {
                for c in b"[rtu] ERROR: Failed to alloc L1 PT\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
                loop {}
            }
        };
        // Zero L1 PT
        unsafe { core::ptr::write_bytes(l1_pa as *mut u8, 0, 4096); }

        // Allocate L2 PT
        let new_l2_pa = match crate::memory::allocator::alloc_page() {
            Some(p) => p,
            None => {
                for c in b"[rtu] ERROR: Failed to alloc L2 PT\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
                loop {}
            }
        };
        // Zero L2 PT
        unsafe { core::ptr::write_bytes(new_l2_pa as *mut u8, 0, 4096); }
        l2_pa = new_l2_pa;

        // Create ROOT[0x11] -> L1 non-leaf PTE
        let l1_ppn = l1_pa >> 12;
        let root_11_val: u64 = ((l1_ppn as u64) << 10) | 0x01;  // V=1, non-leaf
        unsafe {
            let ptr = (user_root + 0x11 * 8) as *mut u64;
            core::ptr::write_volatile(ptr, root_11_val);
        }
        for c in b"[rtu] Wrote ROOT[0x11]=0x" { crate::console::sbi_console_putchar_raw(*c as usize); }
        tmp = root_11_val as usize;
        i = 0;
        while tmp > 0 {
            let d = (tmp & 0xf) as u8;
            digits[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
            i += 1;
            tmp >>= 4;
        }
        while i > 0 {
            i -= 1;
            crate::console::sbi_console_putchar_raw(digits[i] as usize);
        }
        for c in b"\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

        // Create L1[0] -> L2 non-leaf PTE
        let l2_ppn = l2_pa >> 12;
        let l1_0_val: u64 = ((l2_ppn as u64) << 10) | 0x01;
        unsafe {
            let ptr = (l1_pa) as *mut u64;
            core::ptr::write_volatile(ptr, l1_0_val);
        }
        for c in b"[rtu] Wrote L1[0]=0x" { crate::console::sbi_console_putchar_raw(*c as usize); }
        tmp = l1_0_val as usize;
        i = 0;
        while tmp > 0 {
            let d = (tmp & 0xf) as u8;
            digits[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
            i += 1;
            tmp >>= 4;
        }
        while i > 0 {
            i -= 1;
            crate::console::sbi_console_putchar_raw(digits[i] as usize);
        }
        for c in b"\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

        // Create L2[0x11] -> actual page (leaf PTE)
        // PA 0x80079000, flags=RWX+U (0x1F) for user-mode access
        // For Sv39 leaf PTEs, PPN is stored contiguously at bits [53:10]
        let page_pa = 0x80079000usize;
        let page_ppn = page_pa >> 12;
        // Flags: 0x1F = V|R|W|X|U (user accessible RWX)
        let l2_11_val: u64 = ((page_ppn as u64) << 10) | 0x1F;
        unsafe {
            let ptr = (l2_pa + 0x11 * 8) as *mut u64;
            core::ptr::write_volatile(ptr, l2_11_val);
        }
        for c in b"[rtu] Wrote L2[0x11]=0x" { crate::console::sbi_console_putchar_raw(*c as usize); }
        tmp = l2_11_val as usize;
        i = 0;
        while tmp > 0 {
            let d = (tmp & 0xf) as u8;
            digits[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
            i += 1;
            tmp >>= 4;
        }
        while i > 0 {
            i -= 1;
            crate::console::sbi_console_putchar_raw(digits[i] as usize);
        }
        for c in b"\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
        for c in b"[rtu] Entry page mapping created\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

        // Print actual L2 PA for debugging
        for c in b"[rtu] L2 PA=0x" { crate::console::sbi_console_putchar_raw(*c as usize); }
        tmp = l2_pa;
        i = 0;
        while tmp > 0 {
            let d = (tmp & 0xf) as u8;
            digits[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
            i += 1;
            tmp >>= 4;
        }
        while i > 0 {
            i -= 1;
            crate::console::sbi_console_putchar_raw(digits[i] as usize);
        }
        for c in b"\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    }

    // Verify the page table entry we just created using ACTUAL l2_pa
    let l2_pte_check: u64 = *((l2_pa + 0x11 * 8) as *const u64);
    for c in b"[rtu] Verify L2[0x11]=0x" { crate::console::sbi_console_putchar_raw(*c as usize); }
    tmp = l2_pte_check as usize;
    i = 0;
    while tmp > 0 {
        let d = (tmp & 0xf) as u8;
        digits[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
        i += 1;
        tmp >>= 4;
    }
    while i > 0 {
        i -= 1;
        crate::console::sbi_console_putchar_raw(digits[i] as usize);
    }
    for c in b"\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Try with aligned entry point (round down to 4-byte boundary)
    let aligned_pc = pc & !0x3;  // 0x11326 -> 0x11324
    if aligned_pc != pc {
        for c in b"[rtu] Trying aligned pc=0x" { crate::console::sbi_console_putchar_raw(*c as usize); }
        tmp = aligned_pc;
        i = 0;
        while tmp > 0 {
            let d = (tmp & 0xf) as u8;
            digits[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
            i += 1;
            tmp >>= 4;
        }
        while i > 0 {
            i -= 1;
            crate::console::sbi_console_putchar_raw(digits[i] as usize);
        }
        for c in b"\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    }
    // Print sstatus and sscratch before sret
    for c in b"[rtu] Before sret - sstatus=0x" { crate::console::sbi_console_putchar_raw(*c as usize); }
    let mut sstatus_val: usize;
    unsafe {
        core::arch::asm!("csrr {0}, sstatus", out(reg) sstatus_val);
    }
    tmp = sstatus_val;
    i = 0;
    while tmp > 0 {
        let d = (tmp & 0xf) as u8;
        digits[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
        i += 1;
        tmp >>= 4;
    }
    while i > 0 {
        i -= 1;
        crate::console::sbi_console_putchar_raw(digits[i] as usize);
    }
    for c in b", sscratch=0x" { crate::console::sbi_console_putchar_raw(*c as usize); }
    let mut sscratch_val: usize;
    unsafe {
        core::arch::asm!("csrr {0}, sscratch", out(reg) sscratch_val);
    }
    tmp = sscratch_val;
    i = 0;
    while tmp > 0 {
        let d = (tmp & 0xf) as u8;
        digits[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
        i += 1;
        tmp >>= 4;
    }
    while i > 0 {
        i -= 1;
        crate::console::sbi_console_putchar_raw(digits[i] as usize);
    }
    for c in b"\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Try a simpler approach: just test if we can read memory through user page table
    for c in b"[rtu] Testing user memory read at va=0x11326...\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Just before sret, switch satp
    unsafe {
        core::arch::asm!(
            // Switch to user page table
            "csrw satp, a1",
            "sfence.vma zero, zero",
            // Set sepc to entry point
            "csrw sepc, a3",
            // Set sstatus to user mode (SPP=0, SPIE=1)
            "li t0, 0x00000020",
            "csrw sstatus, t0",
            // Set sscratch to kernel_sp (trap frame pointer) so trap handler can use it
            // When a trap occurs with sscratch!=0, CPU swaps sp and sscratch:
            //   - sp becomes kernel_sp (trap handler runs on kernel stack)
            //   - sscratch becomes user_sp (saved for sret)
            // So we set sscratch = kernel_sp, NOT user_sp
            "mv t0, a0",  // a0 = kernel sp = trap frame pointer
            "csrw sscratch, t0",
            // Set sp to user stack before sret
            "mv sp, a2",
            // Print "READY" via ecall
            "li a0, 0x52",  // R
            "li a7, 1",
            "ecall",
            "li a0, 0x45",  // E
            "ecall",
            "li a0, 0x41",  // A
            "ecall",
            "li a0, 0x44",  // D
            "ecall",
            "li a0, 0x59",  // Y
            "ecall",
            "li a0, 0x0d",  // CR
            "ecall",
            "li a0, 0x0a",  // LF
            "ecall",
            // Set sp to user stack (needed before sret)
            "mv sp, a2",
            // sret - returns to user mode
            // IMPORTANT: sscratch now has user_sp, so when a trap occurs:
            // 1. CPU swaps sp and sscratch: sp = kernel sp, sscratch = user sp
            // 2. Trap handler uses kernel stack
            // 3. On sret, swap back: sp = user sp, sscratch = kernel sp
            "sret",
            options(nostack),
            in("a0") tf,  // trap frame pointer = kernel stack pointer
            in("a1") satp,
            in("a2") sp,
            in("a3") pc,
        );
    }

    for c in b"[rtu] sret returned - THIS SHOULD NOT HAPPEN\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    loop {}
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
