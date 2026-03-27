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

    crate::println!("[trap] OK");
}

/// Handle a trap - called from assembly trap entry
#[no_mangle]
extern "C" fn handle_trap() {
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
                    // Handle system call
                    crate::syscall::do_syscall();
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
                    // Timer interrupt - for preemption
                }
                _ => {
                    // Other interrupts
                }
            }
        }
    }
}
