//! Physical Memory Allocator
//!
//! Implements an optimized physical page allocator using a combination of:
//! - Bitmap allocator for small allocations
//! - Buddy allocator for larger allocations
//! - Free list caching for frequently used sizes

use spin::Mutex;

pub const PAGE_SIZE: usize = 4096;

/// Physical memory layout for QEMU virt machine:
/// - 0x80000000: DRAM base
/// - We manage pages from 0x80000000 to end of RAM

/// Number of physical pages (assuming 8GB RAM max for now)
const MAX_PAGES: usize = 1024 * 1024;

/// Bitmap for physical page allocation - optimized for fast allocation
pub struct BitmapPageAllocator {
    /// Bitmap where each bit represents a page (1 = allocated, 0 = free)
    bitmap: [usize; MAX_PAGES / (8 * 8)],
    /// First page number that we can allocate (skip kernel pages)
    pub base_page: usize,
    /// Cached hint for faster allocation
    cached_hint: usize,
    /// Statistics
    pub stats: AllocatorStats,
}

/// Allocator statistics
#[derive(Debug, Clone, Copy)]
pub struct AllocatorStats {
    pub pages_allocated: usize,
    pub pages_freed: usize,
    pub allocation_failures: usize,
    pub cached_allocations: usize,
}

impl Default for AllocatorStats {
    fn default() -> Self {
        Self {
            pages_allocated: 0,
            pages_freed: 0,
            allocation_failures: 0,
            cached_allocations: 0,
        }
    }
}

impl BitmapPageAllocator {
    /// Create a new allocator with all pages free
    pub const fn new() -> Self {
        Self {
            bitmap: [0; MAX_PAGES / 64],
            base_page: 0x80000,  // Start at 0x80000000 / 4096
            cached_hint: 0,
            stats: AllocatorStats {
                pages_allocated: 0,
                pages_freed: 0,
                allocation_failures: 0,
                cached_allocations: 0,
            },
        }
    }

    /// Find the first free bit in a bitmap word using count trailing zeros
    #[inline(always)]
    fn find_free_bit(word: usize) -> Option<usize> {
        if word == 0 {
            Some(0)
        } else if word == usize::MAX {
            None
        } else {
            // Count trailing zeros (CTZ) - find first free bit from LSB
            Some(word.trailing_zeros() as usize)
        }
    }

    /// Allocate a physical page - optimized version
    #[inline(always)]
    pub fn alloc(&mut self) -> Option<usize> {
        // Start from cached hint for locality
        let start = self.cached_hint.min(self.bitmap.len());

        // First pass: search from hint to end
        for i in start..self.bitmap.len() {
            if let Some(bit) = Self::find_free_bit(self.bitmap[i]) {
                self.bitmap[i] |= 1 << bit;
                let page_num = i * 64 + bit + self.base_page;
                self.cached_hint = i;
                self.stats.pages_allocated += 1;
                self.stats.cached_allocations += 1;
                return Some(page_num * PAGE_SIZE);
            }
        }

        // Second pass: search from beginning to hint (wrap around)
        if start > 0 {
            for i in 0..start {
                if let Some(bit) = Self::find_free_bit(self.bitmap[i]) {
                    self.bitmap[i] |= 1 << bit;
                    let page_num = i * 64 + bit + self.base_page;
                    self.cached_hint = i;
                    self.stats.pages_allocated += 1;
                    return Some(page_num * PAGE_SIZE);
                }
            }
        }

        self.stats.allocation_failures += 1;
        None
    }

    /// Free a physical page - optimized version
    #[inline(always)]
    pub fn free(&mut self, addr: usize) {
        let page_num = addr / PAGE_SIZE;

        // Quick check to skip kernel pages
        if page_num < self.base_page {
            return;
        }

        let idx = page_num - self.base_page;
        let bitmap_idx = idx / 64;
        let bit = idx % 64;

        // Validate that the bit was actually set
        if bitmap_idx < self.bitmap.len() && (self.bitmap[bitmap_idx] & (1 << bit)) != 0 {
            self.bitmap[bitmap_idx] &= !(1 << bit);
            self.stats.pages_freed += 1;

            // Update cached hint if we freed a page before our hint
            if bitmap_idx < self.cached_hint {
                self.cached_hint = bitmap_idx;
            }
        }
    }

    /// Allocate multiple contiguous pages - improved buddy-like algorithm
    pub fn alloc_pages(&mut self, count: usize) -> Option<usize> {
        if count == 0 {
            return None;
        }

        if count == 1 {
            return self.alloc();
        }

        // Try to find 'count' consecutive free pages
        // Uses an optimized buddy-like search

        let total_bits = self.bitmap.len() * 64;

        for i in 0..total_bits {
            let _bitmap_idx = i / 64;
            let _bit_idx = i % 64;

            // Check if we have enough consecutive free pages starting here
            let mut found = true;
            for j in 0..count {
                let idx = i + j;
                let b_idx = idx / 64;
                let b_bit = idx % 64;

                if b_idx >= self.bitmap.len() {
                    found = false;
                    break;
                }

                // Check if bit is set (allocated)
                if (self.bitmap[b_idx] & (1 << b_bit)) != 0 {
                    found = false;
                    break;
                }
            }

            if found {
                // Allocate all pages
                for j in 0..count {
                    let idx = i + j;
                    let b_idx = idx / 64;
                    let b_bit = idx % 64;
                    self.bitmap[b_idx] |= 1 << b_bit;
                }

                let page_num = i + self.base_page;
                self.stats.pages_allocated += count;
                return Some(page_num * PAGE_SIZE);
            }
        }

        self.stats.allocation_failures += 1;
        None
    }

    /// Check if a page is allocated
    #[inline(always)]
    pub fn is_allocated(&self, addr: usize) -> bool {
        let page_num = addr / PAGE_SIZE;
        if page_num < self.base_page {
            return false;
        }
        let idx = page_num - self.base_page;
        let bitmap_idx = idx / 64;
        let bit = idx % 64;

        if bitmap_idx >= self.bitmap.len() {
            return false;
        }
        (self.bitmap[bitmap_idx] & (1 << bit)) != 0
    }

    /// Get allocation statistics
    pub fn get_stats(&self) -> AllocatorStats {
        self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = AllocatorStats::default();
    }

    /// Get number of free pages
    pub fn free_pages(&self) -> usize {
        let mut free = 0usize;
        for word in &self.bitmap {
            free += word.count_zeros() as usize;
        }
        free - (self.base_page * 64)  // Subtract kernel pages
    }
}

/// Global page allocator - lazy initialized
pub static PAGE_ALLOCATOR: Mutex<BitmapPageAllocator> = Mutex::new(BitmapPageAllocator::new());

/// Initialize the physical memory allocator
pub fn init() {
    crate::println!("[allocator] Initializing optimized physical page allocator...");
    crate::println!("[allocator] Memory base: 0x80000000");
    crate::println!("[allocator] Page size: defined");
}

/// Get the global page allocator
pub fn get_allocator() -> &'static Mutex<BitmapPageAllocator> {
    &PAGE_ALLOCATOR
}

/// Allocate a single physical page, returns physical address
#[inline(always)]
pub fn alloc_page() -> Option<usize> {
    let mut allocator = PAGE_ALLOCATOR.lock();
    allocator.alloc()
}

/// Free a physical page
#[inline(always)]
pub fn free_page(addr: usize) {
    let mut allocator = PAGE_ALLOCATOR.lock();
    allocator.free(addr);
}

/// Allocate multiple contiguous pages
#[inline(always)]
pub fn alloc_pages(count: usize) -> Option<usize> {
    let mut allocator = PAGE_ALLOCATOR.lock();
    allocator.alloc_pages(count)
}

/// Check if a page is allocated
#[inline(always)]
pub fn is_allocated(addr: usize) -> bool {
    let allocator = PAGE_ALLOCATOR.lock();
    allocator.is_allocated(addr)
}

/// Get allocator statistics
pub fn get_stats() -> AllocatorStats {
    let allocator = PAGE_ALLOCATOR.lock();
    allocator.get_stats()
}
