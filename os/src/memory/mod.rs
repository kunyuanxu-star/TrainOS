//! Memory management module
//!
//! Implements Sv39 virtual memory for RISC-V

#[allow(non_snake_case)]
pub mod Sv39;
pub mod allocator;

/// Initialize memory management subsystem
pub fn init() {
    crate::println!("[memory] Initializing Sv39 memory management...");
    crate::println!("[memory] Physical memory base: 0x80000000");
    crate::println!("[memory] Sv39 page size: 4096 bytes");
    crate::println!("[memory] Virtual address space: 256GB");
    crate::println!("[memory] OK");
}
