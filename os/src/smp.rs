//! SMP (Symmetric Multi-Processing) Support
//!
//! Provides multi-core infrastructure for TrainOS

pub mod boot;
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
    // Output 'S' using inline asm
    unsafe {
        core::arch::asm!(
            "li a7, 1",
            "li a0, 83",  // 'S'
            "ecall"
        );
    }

    // Initialize boot data structures
    boot::init_boot();

    // Detect number of HARTs from DT
    detect_harts();

    // Initialize per-CPU structures for each hart
    cpu::init_per_cpu();

    // Set up IPI (Inter-Processor Interrupt) handling
    ipi::init();

    // Start other HARTs (secondary cores)
    boot::start_other_harts();

    // Output 'E' for end
    unsafe {
        core::arch::asm!(
            "li a7, 1",
            "li a0, 69",  // 'E'
            "ecall"
        );
    }
}

/// Detect available HARTs
/// In QEMU virt machine, we typically have 1 HART unless otherwise configured
fn detect_harts() {
    // For now, just output a single 'X' to confirm we reached here
    unsafe {
        core::arch::asm!(
            "li a7, 1",
            "li a0, 88",  // 'X'
            "ecall"
        );
    }
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
