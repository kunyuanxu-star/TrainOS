//! SMP (Symmetric Multi-Processing) Support
//!
//! Provides multi-core infrastructure for TrainOS

pub mod cpu;
pub mod ipi;
pub mod hart;

use spin::Mutex;

/// Number of detected HARTs (hardware threads / CPU cores)
static HART_COUNT: Mutex<usize> = Mutex::new(1);

/// Current HART ID (CPU core running this code)
static CURRENT_HARTID: Mutex<usize> = Mutex::new(0);

/// Initialize SMP subsystem
/// Called early during boot to detect and configure cores
pub fn init() {
    crate::println!("[smp] Initializing SMP subsystem...");

    // Detect number of HARTs from DT
    detect_harts();

    let harts = *HART_COUNT.lock();
    crate::println!("[smp] Detected HARTs");

    // Initialize per-CPU structures for each hart
    cpu::init_per_cpu();

    // Set up IPI (Inter-Processor Interrupt) handling
    ipi::init();

    crate::println!("[smp] SMP initialization complete");
}

/// Detect available HARTs
/// In QEMU virt machine, we typically have 1 HART unless otherwise configured
fn detect_harts() {
    // For now, default to 1 HART (UP system)
    // In a real implementation, we would parse the DT/ACPI tables
    let mut count = HART_COUNT.lock();
    *count = 1;

    // If we had multiple HARTs, we would set up them here:
    // - Each HART needs its own stack
    // - Each HART needs to know where to jump during startup
    // - We need to use IPI to wake up secondary HARTs
}

/// Get the current HART ID
pub fn current_hartid() -> usize {
    *CURRENT_HARTID.lock()
}

/// Set the current HART ID
pub fn set_current_hartid(hartid: usize) {
    let mut current = CURRENT_HARTID.lock();
    *current = hartid;
}

/// Get the number of available HARTs
pub fn hart_count() -> usize {
    *HART_COUNT.lock()
}

/// Memory barrier to ensure ordering of memory operations
#[inline]
pub fn membarrier() {
    unsafe {
        core::arch::asm!("fence rw, rw");
    }
}
