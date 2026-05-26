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

// ── Virtual Address Space Constants ────────────────────────────────────

/// Sv39 virtual address space bounds (512 GiB total).
pub const USER_SPACE_START_SV39: usize = 0x0000_0000_0000_0000;
pub const USER_SPACE_END_SV39: usize = 0x0000_003F_FFFF_FFFF;
pub const KERNEL_SPACE_START_SV39: usize = 0xFFFF_FFC0_0000_0000;

/// Sv48 virtual address space bounds (256 TiB total).
pub const USER_SPACE_START_SV48: usize = 0x0000_0000_0000_0000;
pub const USER_SPACE_END_SV48: usize = 0x0000_FFFF_FFFF_FFFF;
pub const KERNEL_SPACE_START_SV48: usize = 0xFFFF_8000_0000_0000;

/// Sv57 virtual address space bounds (128 PiB total).
pub const USER_SPACE_START_SV57: usize = 0x0000_0000_0000_0000;
pub const USER_SPACE_END_SV57: usize = 0x00FF_FFFF_FFFF_FFFF;
pub const KERNEL_SPACE_START_SV57: usize = 0xFF00_0000_0000_0000;

/// SATP mode values for page-based address translation.
pub const SATP_MODE_SV39: usize = 8;
pub const SATP_MODE_SV48: usize = 9;
pub const SATP_MODE_SV57: usize = 10;

/// Maximum physical address supported (56-bit for Sv39/Sv48/Sv57).
pub const MAX_PHYS_ADDR: usize = (1usize << 56) - 1;

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
