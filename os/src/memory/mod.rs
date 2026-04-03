//! Memory management module
//!
//! Implements Sv39 virtual memory for RISC-V

#[allow(non_snake_case)]
pub mod Sv39;
pub mod allocator;

/// Fixed page table pool region - use the first 256KB of RAM for page tables
/// This is at PA 0x80000000-0x80040000, which is identity-mapped by RustSBI
const PAGE_TABLE_POOL_PA: usize = 0x80000000;
const PAGE_TABLE_POOL_COUNT: usize = 64;  // 64 pages = 256KB for page tables

/// Initialize memory management subsystem
pub fn init() {
    // Initialize the page table allocator with a FIXED region in low memory.
    // This region (0x80000000-0x80040000) is guaranteed to be identity-mapped
    // by RustSBI, so all page table allocations will be accessible.
    Sv39::init_page_table_allocator_with_pool(PAGE_TABLE_POOL_PA, PAGE_TABLE_POOL_COUNT);

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
