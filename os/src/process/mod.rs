//! Process management module
//!
//! Manages tasks/processes and scheduling

pub mod task;
pub mod processor;
pub mod scheduler;

/// Initialize the process management subsystem
pub fn init() {
    crate::println!("[process] Initializing process management...");
    crate::println!("[process] Creating idle task...");
    crate::println!("[process] OK");
}

/// Run the first process
pub fn run_first_process() -> ! {
    crate::println!("[process] Starting init process...");

    // For now, just halt with a message
    crate::println!();
    crate::println!("========================================");
    crate::println!("  trainOS is running!");
    crate::println!("  No more processes to run.");
    crate::println!("========================================");

    loop {
        // Halt the CPU
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
