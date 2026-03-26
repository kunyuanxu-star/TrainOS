//! Physical Memory Allocator
//!
//! Implements a simple physical page allocator using a bitmap.

use spin::Mutex;

/// Physical memory layout for QEMU virt machine:
/// - 0x80000000: DRAM base
/// - We manage pages from 0x80000000 to end of RAM

/// Number of physical pages (assuming 8GB RAM max for now)
const MAX_PAGES: usize = 1024 * 1024;

/// Bitmap for physical page allocation
pub struct BitmapPageAllocator {
    /// Bitmap where each bit represents a page (1 = allocated, 0 = free)
    bitmap: [usize; MAX_PAGES / (8 * 8)],
    /// First page number that we can allocate (skip kernel pages)
    pub base_page: usize,
}

impl BitmapPageAllocator {
    /// Create a new allocator with all pages free
    pub const fn new() -> Self {
        Self {
            bitmap: [0; MAX_PAGES / 64],
            base_page: 0x80000,  // Start at 0x80000000 / 4096
        }
    }

    /// Allocate a physical page
    pub fn alloc(&mut self) -> Option<usize> {
        for i in 0..self.bitmap.len() {
            if self.bitmap[i] != usize::MAX {
                // Find first free bit
                for j in 0..64 {
                    let bit = 1 << j;
                    if (self.bitmap[i] & bit) == 0 {
                        self.bitmap[i] |= bit;
                        let page_num = i * 64 + j + self.base_page;
                        return Some(page_num * PAGE_SIZE);
                    }
                }
            }
        }
        None
    }

    /// Free a physical page
    pub fn free(&mut self, addr: usize) {
        let page_num = addr / PAGE_SIZE;
        if page_num < self.base_page {
            return;  // Skip kernel pages
        }
        let idx = page_num - self.base_page;
        let bitmap_idx = idx / 64;
        let bit = 1 << (idx % 64);
        self.bitmap[bitmap_idx] &= !bit;
    }

    /// Allocate multiple contiguous pages
    pub fn alloc_pages(&mut self, count: usize) -> Option<usize> {
        // Simple: just allocate one at a time for now
        if count == 1 {
            return self.alloc();
        }
        // For larger allocations, find consecutive free pages
        for i in 0..self.bitmap.len() {
            let mut found = 0;
            for j in 0..64 {
                let bit = 1 << j;
                if (self.bitmap[i] & bit) == 0 {
                    found += 1;
                    if found == count {
                        // Allocate all pages
                        let start_idx = i * 64 + j + 1 - count;
                        for k in 0..count {
                            let idx = start_idx + k;
                            let bi = idx / 64;
                            let bj = idx % 64;
                            self.bitmap[bi] |= 1 << bj;
                        }
                        let page_num = (i * 64 + j + 1 - count) + self.base_page;
                        return Some(page_num * PAGE_SIZE);
                    }
                } else {
                    found = 0;
                }
            }
        }
        None
    }
}

pub const PAGE_SIZE: usize = 4096;

/// Global page allocator - lazy initialized
pub static PAGE_ALLOCATOR: Mutex<BitmapPageAllocator> = Mutex::new(BitmapPageAllocator::new());

/// Initialize the physical memory allocator
pub fn init() {
    // Page allocator is statically initialized
}

/// Get the global page allocator
pub fn get_allocator() -> &'static Mutex<BitmapPageAllocator> {
    &PAGE_ALLOCATOR
}
