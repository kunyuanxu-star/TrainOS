//! Memory management module
//!
//! Implements Sv39 virtual memory for RISC-V

#[allow(non_snake_case)]
pub mod Sv39;
pub mod allocator;

/// Initialize memory management subsystem
pub fn init() {
    // Initialize the page table allocator FIRST with pages from low memory.
    // These pages are identity-mapped by RustSBI, so they are accessible.
    // We allocate 64 page table frames (256KB total) for the pool.
    const PT_POOL_COUNT: usize = 64;
    let mut base_pa = 0;
    let mut pages = 0;
    for i in 0..PT_POOL_COUNT {
        if let Some(pa) = allocator::alloc_page() {
            if i == 0 {
                base_pa = pa;
            }
            pages += 1;
        } else {
            break;
        }
    }
    if pages > 0 {
        Sv39::init_page_table_allocator_with_pool(base_pa, pages);
    }

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
