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
// CLINT Timer Support
// ============================================

/// CLINT (Core Local Interrupt Controller) registers
/// QEMU virt machine: CLINT at 0x2000000
pub const CLINT_BASE: usize = 0x200_0000;

/// CLINT memory-mapped registers
pub const CLINT_MTIME: usize = CLINT_BASE + 0xBFF8;      // Read-only, real-time counter
pub const CLINT_MTIMECMP: usize = CLINT_BASE + 0x4000;   // Per-hart timer compare (offset per hart)

/// CLINT register access
#[inline(always)]
fn read_clint(reg: usize) -> u64 {
    unsafe { (reg as *const u64).read_volatile() }
}

#[inline(always)]
fn write_clint(reg: usize, value: u64) {
    unsafe { (reg as *mut u64).write_volatile(value) }
}

/// Get current mtime value
pub fn get_mtime() -> u64 {
    read_clint(CLINT_MTIME)
}

/// Get mtimecmp for current hart (assuming hart 0)
pub fn get_mtimecmp() -> u64 {
    read_clint(CLINT_MTIMECMP)
}

/// Set mtimecmp for current hart (assumes hart 0)
/// This arms the timer to interrupt when mtime >= mtimecmp
pub fn set_mtimecmp(value: u64) {
    write_clint(CLINT_MTIMECMP, value);
}

/// Set timer to fire after `us` microseconds
pub fn set_timer_relative(us: u64) {
    // Read current mtime and set mtimecmp directly
    let mtime = get_mtime();
    let target = mtime.wrapping_add(us * 10);  // 10 MHz timebase
    set_mtimecmp(target);
}

/// Initialize the CLINT timer
pub fn clint_init() {
    // Set a short initial timer - this will fire after 10 seconds
    set_timer_relative(10_000_000); // 10 seconds

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

/// Clear the pending timer interrupt
pub fn clear_timer_interrupt() {
    // In RISC-V, writing to mtimecmp clears the pending interrupt
    // byarm: we just re-arm with a large value temporarily
    let mtime = get_mtime();
    set_mtimecmp(mtime.wrapping_add(u64::MAX / 4));
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
