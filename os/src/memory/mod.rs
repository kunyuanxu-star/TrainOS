//! Memory management module
//!
//! Implements Sv39 virtual memory for RISC-V

#[allow(non_snake_case)]
pub mod Sv39;
pub mod allocator;

/// Initialize memory management subsystem
pub fn init() {
    // Initialize the kernel page table using the existing one from RustSBI
    Sv39::init_kernel_page_table();

    // Using inline asm directly to avoid any function call issues
    unsafe {
        let s = "memory init start\n";
        let len = s.len();
        let mut ptr = s.as_ptr() as usize;
        let mut remaining = len;
        core::arch::asm!(
            "1: lbu a0, 0(a1)",
            "   li a7, 1",
            "   ecall",
            "   addi a1, a1, 1",
            "   addi a2, a2, -1",
            "   bnez a2, 1b",
            inout("a1") ptr, inout("a2") remaining);
    }
}
