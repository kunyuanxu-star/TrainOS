// V29: GPU Memory Management (GART/GTT-style)
//
// Manages GPU-visible physical memory allocations with tracking:
//   - Physical page allocation from buddy allocator
//   - Region tracking with GPU VA -> physical page mappings
//   - Up to 64 regions, each up to 16 pages (64KB)

use crate::mem::buddy;

pub(crate) const MAX_GPU_MEM_REGIONS: usize = 64;

/// A GPU memory region: contiguous in GPU VA space, backed by physical pages.
#[derive(Clone, Copy)]
pub(crate) struct GpuMemoryRegion {
    pub gpu_va: usize,
    pub size: usize,
    pub phys_pages: [usize; 16], // up to 64KB tracked inline
    pub page_count: usize,
    pub in_use: bool,
}

pub(crate) static mut GPU_MEM_REGIONS: [GpuMemoryRegion; MAX_GPU_MEM_REGIONS] = [GpuMemoryRegion {
    gpu_va: 0,
    size: 0,
    phys_pages: [0usize; 16],
    page_count: 0,
    in_use: false,
}; MAX_GPU_MEM_REGIONS];

/// Allocate physical pages and register as a GPU memory region.
/// Returns the slot index and fills phys_pages.
pub(crate) fn alloc_region_pages(num_pages: usize) -> Option<(usize, [usize; 16])> {
    unsafe {
        // Find a free region slot
        let mut slot = None;
        for i in 0..MAX_GPU_MEM_REGIONS {
            if !GPU_MEM_REGIONS[i].in_use {
                slot = Some(i);
                break;
            }
        }
        let slot = slot?;

        // Allocate physical pages
        let mut phys_pages = [0usize; 16];
        for i in 0..num_pages {
            phys_pages[i] = buddy::alloc_page()?;
        }
        Some((slot, phys_pages))
    }
}

/// Register a region with all its data filled in.
pub(crate) fn register_region(slot: usize, gpu_va: usize, size: usize, phys_pages: [usize; 16], page_count: usize) {
    unsafe {
        GPU_MEM_REGIONS[slot] = GpuMemoryRegion {
            gpu_va,
            size,
            phys_pages,
            page_count,
            in_use: true,
        };
    }
}

/// Free GPU memory given a GPU virtual address.
pub(crate) fn gpu_free(gpu_va: usize) -> bool {
    unsafe {
        for i in 0..MAX_GPU_MEM_REGIONS {
            if GPU_MEM_REGIONS[i].in_use && GPU_MEM_REGIONS[i].gpu_va == gpu_va {
                // Free physical pages back to buddy allocator
                for j in 0..GPU_MEM_REGIONS[i].page_count {
                    if GPU_MEM_REGIONS[i].phys_pages[j] != 0 {
                        buddy::free_page(GPU_MEM_REGIONS[i].phys_pages[j], 0);
                    }
                }
                GPU_MEM_REGIONS[i].in_use = false;
                GPU_MEM_REGIONS[i].phys_pages = [0usize; 16];
                GPU_MEM_REGIONS[i].page_count = 0;
                return true;
            }
        }
        false
    }
}

/// Get physical address for a GPU virtual address (GART translation).
pub(crate) fn gpu_va_to_phys(gpu_va: usize) -> Option<usize> {
    unsafe {
        for i in 0..MAX_GPU_MEM_REGIONS {
            if GPU_MEM_REGIONS[i].in_use {
                let region = &GPU_MEM_REGIONS[i];
                if gpu_va >= region.gpu_va && gpu_va < region.gpu_va + region.size {
                    let offset = gpu_va - region.gpu_va;
                    let page_idx = offset >> 12;
                    if page_idx < region.page_count {
                        let page_offset = offset & 0xFFF;
                        return Some(region.phys_pages[page_idx] + page_offset);
                    }
                }
            }
        }
        None
    }
}

/// Returns total GPU memory allocated.
pub(crate) fn gpu_mem_used() -> usize {
    unsafe {
        let mut total = 0;
        for i in 0..MAX_GPU_MEM_REGIONS {
            if GPU_MEM_REGIONS[i].in_use {
                total += GPU_MEM_REGIONS[i].size;
            }
        }
        total
    }
}
