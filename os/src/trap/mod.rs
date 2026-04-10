//! Trap handling module
//!
//! Handles exceptions, interrupts, and system calls

/// Trap cause enumeration
#[derive(Debug)]
pub enum TrapCause {
    Exception(ExceptionCause),
    Interrupt(InterruptCause),
}

/// Exception causes
#[derive(Debug)]
pub enum ExceptionCause {
    InstructionMisaligned,
    InstructionFault,
    IllegalInstruction,
    Breakpoint,
    LoadFault,
    StoreFault,
    EcallFromUser,
    EcallFromSupervisor,
    PageFault,
}

/// Interrupt causes
#[derive(Debug)]
pub enum InterruptCause {
    SupervisorSoftware,
    SupervisorTimer,
    SupervisorExternal,
}

/// Enable timer interrupt in sie (Supervisor Interrupt Enable)
/// We need to enable STIE (bit 5) for timer interrupts to fire
pub fn enable_timer_interrupt() {
    // Use a simple approach: load immediate, then set bits in sie
    // Also enable SSIE (bit 1) and SEIE (bit 9) for completeness
    unsafe {
        core::arch::asm!(
            // 0x422 = 0b10000100010 = SSIE(1) | STIE(5) | SEIE(9)
            "li t0, 0x422",
            // Set SSIE, STIE, SEIE bits using csrs (atomic set)
            "csrs sie, t0",
            out("t0") _,
        );
    }
}

/// Initialize trap handling
pub fn init() {
    // Set stvec to trap handler entry point using inline asm
    extern "C" {
        fn __trap_entry();
    }
    unsafe {
        let stvec_val = __trap_entry as *const () as usize;
        core::arch::asm!("csrw stvec, {0}", in(reg) stvec_val);
    }
}

/// Handle a trap - called from assembly trap entry
/// a0 = pointer to trap frame on stack
#[no_mangle]
extern "C" fn handle_trap(trap_frame: *mut crate::process::context::TrapFrame) {
    // Set the current trap frame pointer for the scheduler
    {
        let mut current_tf = crate::process::CURRENT_TRAP_FRAME.lock();
        *current_tf = crate::process::TrapFramePtr(trap_frame);
    }

    // Also set the kernel stack top
    {
        let mut kstack = crate::process::KERNEL_STACK_TOP.lock();
        unsafe {
            let sp = (*trap_frame).sp;
            *kstack = Some(sp);
        }
    }

    #[allow(deprecated)]
    let scause = riscv::register::scause::read();

    let cause: TrapCause = if scause.is_exception() {
        let ex = match scause.code() {
            0 => ExceptionCause::InstructionMisaligned,
            1 => ExceptionCause::InstructionFault,
            2 => ExceptionCause::IllegalInstruction,
            3 => ExceptionCause::Breakpoint,
            5 => ExceptionCause::LoadFault,
            7 => ExceptionCause::StoreFault,
            12 => ExceptionCause::PageFault,
            13 => ExceptionCause::PageFault,
            15 => ExceptionCause::PageFault,
            8 => ExceptionCause::EcallFromUser,
            9 => ExceptionCause::EcallFromSupervisor,
            _ => ExceptionCause::InstructionFault,
        };
        TrapCause::Exception(ex)
    } else if scause.is_interrupt() {
        let intr = match scause.code() {
            1 => InterruptCause::SupervisorSoftware,
            5 => InterruptCause::SupervisorTimer,
            9 => InterruptCause::SupervisorExternal,
            _ => InterruptCause::SupervisorExternal,
        };
        TrapCause::Interrupt(intr)
    } else {
        return;
    };

    match &cause {
        TrapCause::Exception(ex) => {
            match ex {
                ExceptionCause::EcallFromUser | ExceptionCause::EcallFromSupervisor => {
                    // Handle system call - pass trap frame pointer
                    crate::syscall::do_syscall(trap_frame);

                    // Increment sepc to skip the ecall instruction
                    // This is necessary because sepc still points to the ecall
                    unsafe {
                        (*trap_frame).sepc += 4;
                    }

                    // Check if a schedule was requested (e.g., from sys_sched_yield)
                    // Store guard in variable to avoid double-locking
                    let mut schedule_guard = crate::process::SCHEDULE_REQUESTED.lock();
                    if *schedule_guard {
                        *schedule_guard = false;
                        drop(schedule_guard);
                        // Perform the actual task switch
                        crate::process::do_schedule(trap_frame);
                        // Don't return normally - do_schedule has switched context
                        return;
                    }
                }
                ExceptionCause::PageFault => {
                    // Handle page fault - for COW fork and demand paging
                    crate::println!("[trap] Page fault occurred");
                    handle_page_fault(trap_frame);
                }
                _ => {
                    // Print the exception details using raw putchar
                    for c in b"[trap] Exception: scause=" {
                        crate::console::sbi_console_putchar_raw(*c as usize);
                    }
                    // Print scause value
                    let sc: usize;
                    unsafe {
                        core::arch::asm!("csrr {0}, scause", out(reg) sc);
                    }
                    // Print scause as hex
                    let mut hex_buf = [0u8; 16];
                    let mut i = 0;
                    let mut v = sc;
                    while v > 0 && i < 16 {
                        let d = (v & 0xf) as u8;
                        hex_buf[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
                        i += 1;
                        v >>= 4;
                    }
                    for j in (0..i).rev() {
                        crate::console::sbi_console_putchar_raw(hex_buf[j] as usize);
                    }
                    for c in b", sepc=\r\n" {
                        crate::console::sbi_console_putchar_raw(*c as usize);
                    }
                    // Also print sepc
                    let epc: usize;
                    unsafe {
                        core::arch::asm!("csrr {0}, sepc", out(reg) epc);
                    }
                    let mut v = epc;
                    let mut i = 0;
                    while v > 0 && i < 16 {
                        let d = (v & 0xf) as u8;
                        hex_buf[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
                        i += 1;
                        v >>= 4;
                    }
                    for j in (0..i).rev() {
                        crate::console::sbi_console_putchar_raw(hex_buf[j] as usize);
                    }
                    for c in b"\r\n" {
                        crate::console::sbi_console_putchar_raw(*c as usize);
                    }
                    loop {}
                }
            }
        }
        TrapCause::Interrupt(intr) => {
            match intr {
                InterruptCause::SupervisorTimer => {
                    // Timer interrupt - trigger scheduler preemption
                    handle_timer_interrupt();
                }
                InterruptCause::SupervisorSoftware => {
                    // Software interrupt (IPI) - could be used for cross-CPU signals
                    crate::println!("[trap] Software interrupt");
                }
                InterruptCause::SupervisorExternal => {
                    // External interrupt (PLIC) - device interrupt
                    handle_external_interrupt();
                }
            }
        }
    }
}

/// Handle timer interrupt - trigger task scheduling
fn handle_timer_interrupt() {
    // Re-arm the timer for the next quantum using direct CLINT MMIO
    // This avoids the SBI_SET_TIMER hang issue with RustSBI 0.4.0
    let mtime = unsafe { *(0x0200bff8usize as *const u64) };
    let interval = 100_000u64; // 10ms at 10MHz
    let mtimecmp = mtime.wrapping_add(interval);
    let mtimecmp_addr = 0x02004000usize; // CLINT mtimecmp for hart 0
    unsafe {
        *(mtimecmp_addr as *mut u64) = mtimecmp;
    }

    // Request the scheduler to preempt the current task
    crate::process::schedule_preempt();
}

/// Handle external (device) interrupt via PLIC
fn handle_external_interrupt() {
    // Claim the interrupt from PLIC
    let irq = crate::drivers::interrupt::plic_claim();

    if irq != 0 {
        // Handle the IRQ
        crate::drivers::interrupt::handle_irq(irq);

        // Complete the interrupt
        crate::drivers::interrupt::plic_complete(irq);
    }
}

/// Handle page fault - for COW fork and demand paging
fn handle_page_fault(_trap_frame: *mut crate::process::context::TrapFrame) {
    #[allow(deprecated)]
    let stval: usize = riscv::register::stval::read();

    crate::println!("[trap] Page fault occurred");

    // Try to handle COW page
    if !crate::memory::Sv39::handle_cow_page(stval) {
        crate::println!("[trap] Failed to handle page fault");
        loop {}
    }
}
