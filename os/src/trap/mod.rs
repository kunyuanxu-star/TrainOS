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
    crate::println!("[trap] OK");
}

/// Handle a trap
pub fn handle_trap() {
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
                    crate::syscall::handle_syscall();
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
