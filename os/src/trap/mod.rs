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

/// Initialize trap handling
pub fn init() {
    crate::println!("[trap] Initializing trap handling...");
    crate::println!("[trap] Setting up sstatus, stvec...");

    // Set stvec to trap handler entry point
    // Using Direct mode: stvec points to a single entry point
    extern "C" {
        fn __trap_entry();
    }
    unsafe {
        // Set stvec: bits[1:0] = mode (0=Direct), bits[MAX:2] = address
        let stvec_val = __trap_entry as *const () as usize;
        core::arch::asm!("csrw stvec, {0}", in(reg) stvec_val);

        // Enable interrupts - set SIE bit (Supervisor Interrupt Enable) in sstatus
        // SIE is bit 1 in sstatus
        let sie_bit = 1usize << 1;
        core::arch::asm!(
            "csrr t0, sstatus",
            "or t0, t0, {0}",
            "csrw sstatus, t0",
            in(reg) sie_bit,
            out("t0") _
        );
    }

    // Initialize CLINT timer
    crate::drivers::interrupt::clint_init();

    crate::println!("[trap] OK");
}

/// Handle a trap - called from assembly trap entry
/// a0 = pointer to trap frame on stack
#[no_mangle]
extern "C" fn handle_trap(trap_frame: *mut crate::process::context::TrapFrame) {
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
                }
                ExceptionCause::PageFault => {
                    // Handle page fault - for COW fork and demand paging
                    crate::println!("[trap] Page fault occurred");
                    handle_page_fault(trap_frame);
                }
                _ => {
                    crate::println!("[trap] Exception occurred");
                    loop {}
                }
            }
        }
        TrapCause::Interrupt(intr) => {
            match intr {
                InterruptCause::SupervisorTimer => {
                    // Timer interrupt - trigger scheduler preemption
                    crate::println!("[trap] Timer interrupt");
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
    // Re-arm the timer for the next quantum
    // Use 10ms time slice
    crate::drivers::interrupt::set_timer_relative(10_000);  // 10ms in microseconds

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
