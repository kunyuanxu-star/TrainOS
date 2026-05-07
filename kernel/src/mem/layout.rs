/// DRAM base physical address
pub const DRAM_BASE: usize = 0x8000_0000;

/// Total DRAM size (128MB)
pub const DRAM_SIZE: usize = 128 * 1024 * 1024;

/// DRAM end
pub const DRAM_END: usize = DRAM_BASE + DRAM_SIZE;

/// Kernel is loaded at this physical address by RustSBI
pub const KERNEL_BASE: usize = 0x8020_0000;

/// Page size
pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SHIFT: usize = 12;

/// Physical address of _kernel_end (set by linker script)
#[cfg(not(test))]
pub fn kernel_end() -> usize {
    extern "C" {
        static _kernel_end: u8;
    }
    unsafe { &_kernel_end as *const u8 as usize }
}

#[cfg(test)]
pub fn kernel_end() -> usize {
    0
}

/// Available physical memory range for allocation
pub fn allocatable_range() -> (usize, usize) {
    let end = kernel_end();
    let start = ((end + PAGE_SIZE - 1) / PAGE_SIZE) * PAGE_SIZE;
    (start, DRAM_END)
}
