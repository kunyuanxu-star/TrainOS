//! Memory management module
//!
//! Implements Sv39 virtual memory for RISC-V

#[allow(non_snake_case)]
pub mod Sv39;
pub mod allocator;

/// Fixed page table pool region - use PA 0x80080000 for page tables
/// This is at PA 0x80080000-0x80090000, which is in PMP6 RWX region
/// The general allocator uses PA 0x80071000+, so we start PT pool after that
const PAGE_TABLE_POOL_PA: usize = 0x80080000;
const PAGE_TABLE_POOL_COUNT: usize = 64;  // 64 pages = 256KB for page tables

/// Initialize memory management subsystem
pub fn init() {
    // Initialize the page table allocator with a FIXED region in PMP6 RWX memory.
    Sv39::init_page_table_allocator_with_pool(PAGE_TABLE_POOL_PA, PAGE_TABLE_POOL_COUNT);

    // Initialize the kernel page table
    Sv39::init_kernel_page_table();

    // Output "memory init done" using inline asm to prevent optimization issues
    // Note: Using inline asm directly instead of console functions due to
    // LLVM optimizer issue with inline asm + spin::Mutex in release mode
    unsafe {
        let s = "memory init done\n";
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
