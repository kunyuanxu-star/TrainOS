//! Interrupt Handling for Device Drivers
//!
//! Provides infrastructure for handling device interrupts

use spin::Mutex;

/// Interrupt handler function type
pub type IrqHandler = fn() -> bool;

/// Maximum number of IRQ lines
const MAX_IRQS: usize = 256;

/// Registered IRQ handlers
static IRQ_HANDLERS: Mutex<[Option<IrqHandler>; MAX_IRQS]> = Mutex::new([None; MAX_IRQS]);

/// Register an IRQ handler
pub fn register_irq_handler(irq: usize, handler: IrqHandler) -> Result<(), &'static str> {
    if irq >= MAX_IRQS {
        return Err("IRQ number out of range");
    }

    let mut handlers = IRQ_HANDLERS.lock();
    if handlers[irq].is_some() {
        return Err("IRQ handler already registered");
    }

    handlers[irq] = Some(handler);
    Ok(())
}

/// Unregister an IRQ handler
pub fn unregister_irq_handler(irq: usize) {
    if irq < MAX_IRQS {
        IRQ_HANDLERS.lock()[irq] = None;
    }
}

/// Handle an IRQ (call the registered handler)
pub fn handle_irq(irq: usize) -> bool {
    let handlers = IRQ_HANDLERS.lock();
    if irq < MAX_IRQS {
        if let Some(handler) = handlers[irq] {
            return handler();
        }
    }
    false
}

/// Enable IRQ
pub fn enable_irq(_irq: usize) {
    // In RISC-V, this would configure the PLIC to enable the interrupt
}

/// Disable IRQ
pub fn disable_irq(_irq: usize) {
    // In RISC-V, this would configure the PLIC to disable the interrupt
}

// ============================================
// Timer Support via SBI (works with both CLINT and ACLINT)
// ============================================

/// Read the time CSR (mtime on RISC-V)
#[inline(always)]
fn get_time() -> u64 {
    let val: u64;
    unsafe {
        core::arch::asm!("csrr {0}, time", out(reg) val);
    }
    val
}

/// Set timer to fire after `us` microseconds using SBI_SET_TIMER
pub fn set_timer_relative(us: u64) {
    // Read current time and calculate target
    let current = get_time();
    // For 10MHz timebase: 10 ticks per microsecond
    let target = current.wrapping_add(us * 10);

    // Use SBI_SET_TIMER - this is the portable way that works with any SBI implementation
    // SBI function ID 0 = SET_TIMER
    // a0 = time value (absolute mtimecmp)
    unsafe {
        core::arch::asm!(
            "li a7, 0",
            "mv a0, {0}",
            "ecall",
            in(reg) target
        );
    }
}

/// Initialize the timer for preemption
pub fn clint_init() {
    // Set initial timer - will fire after 10ms
    set_timer_relative(10_000);

    // Enable timer interrupt in sie (Supervisor Interrupt Enable)
    // STIE bit (bit 5) enables supervisor timer interrupts
    unsafe {
        let mut sie: usize;
        core::arch::asm!("csrr {}, sie", out(reg) sie);
        sie |= 1 << 5;  // STIE = Supervisor Timer Interrupt Enable
        core::arch::asm!("csrw sie, {}", in(reg) sie);

        // Also enable software interrupts (for IPI)
        sie |= 1 << 1;  // SSIE = Supervisor Software Interrupt Enable
        core::arch::asm!("csrw sie, {}", in(reg) sie);
    }
}

/// PLIC (Platform Level Interrupt Controller) registers
pub const PLIC_BASE: usize = 0x0C00_0000;
pub const PLIC_PRIORITY: usize = PLIC_BASE;
pub const PLIC_PENDING: usize = PLIC_BASE + 0x1000;
pub const PLIC_ENABLE: usize = PLIC_BASE + 0x2000;
pub const PLIC_THRESHOLD: usize = PLIC_BASE + 0x200000;
pub const PLIC_CLAIM: usize = PLIC_BASE + 0x200004;

/// Claim a pending interrupt (for PLIC)
pub fn plic_claim() -> usize {
    unsafe {
        (PLIC_CLAIM as *const u32).read_volatile() as usize
    }
}

/// Complete a handled interrupt (for PLIC)
pub fn plic_complete(_irq: usize) {
    // Write the interrupt ID back to the claim register
    unsafe {
        (PLIC_CLAIM as *mut u32).write_volatile(_irq as u32);
    }
}

/// Set interrupt priority
pub fn plic_set_priority(irq: usize, priority: u8) {
    if irq < 1024 {
        unsafe {
            let ptr = (PLIC_PRIORITY + irq * 4) as *mut u32;
            ptr.write_volatile(priority as u32);
        }
    }
}

/// Enable an interrupt for a hart
pub fn plic_enable(hart: usize, irq: usize) {
    if irq < 1024 && hart < 1 {  // Simplified - assume 1 hart for now
        unsafe {
            let ptr = (PLIC_ENABLE + hart * 0x100) as *mut u32;
            let val = ptr.read_volatile();
            ptr.write_volatile(val | (1 << irq));
        }
    }
}

/// Set interrupt threshold
pub fn plic_set_threshold(hart: usize, threshold: u32) {
    if hart < 1 {
        unsafe {
            let ptr = (PLIC_THRESHOLD + hart * 0x1000) as *mut u32;
            ptr.write_volatile(threshold);
        }
    }
}
