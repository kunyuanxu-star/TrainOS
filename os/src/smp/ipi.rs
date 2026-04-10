//! Inter-Processor Interrupt (IPI) handling
//!
//! IPIs are used for communication between CPU cores

use spin::Mutex;

/// IPI message types
#[derive(Debug, Clone, Copy)]
pub enum IpiMsg {
    /// Wake up a CPU from sleep
    WakeUp = 0,
    /// Request a CPU to handle an interrupt
    Interrupt,
    /// Request a CPU to reschedule
    Reschedule,
    /// Request a CPU to flush its TLB
    TlbFlush,
    /// Custom system call IPI
    Syscall,
}

/// IPI status flags
#[derive(Debug, Clone, Copy)]
pub struct IpiStatus {
    pub pending: bool,
    pub msg: IpiMsg,
}

/// IPI control register bits
pub const IPI_SOFT: usize = 1 << 0;  // Software interrupt
pub const IPI_TIMER: usize = 1 << 1; // Timer interrupt (for this CPU)
pub const IPI_RESCHEDULE: usize = 1 << 2; // Reschedule interrupt

/// Global IPI state
static IPI_PENDING: Mutex<[bool; 8]> = Mutex::new([false; 8]);

/// Initialize IPI handling
pub fn init() {
    // Output 'I' using inline asm
    for c in b"ipi::init start\n" {
        crate::console::sbi_console_putchar_raw(*c as usize);
    }
    crate::console::console_flush();
}

/// Send an IPI to a specific CPU
pub fn send_ipi(hartid: usize, _msg: IpiMsg) {
    if hartid >= 8 {
        return;
    }

    let mut pending = IPI_PENDING.lock();
    pending[hartid] = true;

    // Trigger the software interrupt on the target CPU
    // In RISC-V, this is done by setting the bit in the SSIP register
    // via the SBI
    unsafe {
        // Use SBI to send IPI
        core::arch::asm!(
            "li a7, 0",
            "mv a0, {0}",
            "ecall",
            in(reg) hartid
        );
    }

    crate::println!("[ipi] Sent IPI");
}

/// Check and clear IPI for current CPU
pub fn check_and_clear_ipi() -> Option<IpiMsg> {
    // Get current HART ID - simplified
    let hartid = 0;

    let mut pending = IPI_PENDING.lock();
    if pending[hartid] {
        pending[hartid] = false;
        Some(IpiMsg::Reschedule)  // Default to reschedule
    } else {
        None
    }
}

/// Handle received IPI
pub fn handle_ipi(msg: IpiMsg) {
    match msg {
        IpiMsg::WakeUp => {
            // CPU is waking up from sleep
        }
        IpiMsg::Interrupt => {
            // Handle external interrupt request
        }
        IpiMsg::Reschedule => {
            // Force reschedule - called from interrupt context
            // This would call the scheduler
        }
        IpiMsg::TlbFlush => {
            // Flush TLB for this CPU
            unsafe {
                core::arch::asm!("sfence.vma zero, zero");
            }
        }
        IpiMsg::Syscall => {
            // Handle cross-CPU syscall
        }
    }
}
