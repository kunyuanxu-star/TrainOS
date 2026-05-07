use core::alloc::{GlobalAlloc, Layout};

/// Minimal kernel heap using a bump allocator with a fixed region.
/// The region starts at _kernel_end and extends for KERNEL_HEAP_SIZE bytes.

use spin::Mutex;
use super::layout;

const KERNEL_HEAP_SIZE: usize = 4 * 1024 * 1024; // 4MB

struct BumpAllocator {
    next: usize,
    end: usize,
    allocations: usize,
}

impl BumpAllocator {
    const fn new() -> Self {
        BumpAllocator { next: 0, end: 0, allocations: 0 }
    }
}

/// Newtype wrapper to satisfy orphan rule for GlobalAlloc.
struct KernelAllocator(Mutex<BumpAllocator>);

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.0.lock();
        let start = (bump.next + layout.align() - 1) & !(layout.align() - 1);
        let end_val = start.checked_add(layout.size()).unwrap();
        if end_val > bump.end {
            return core::ptr::null_mut();
        }
        bump.next = end_val;
        bump.allocations += 1;
        start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut bump = self.0.lock();
        bump.allocations -= 1;
    }
}

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator(Mutex::new(BumpAllocator::new()));

pub fn init() {
    let heap_start = layout::kernel_end();
    let heap_start = ((heap_start + layout::PAGE_SIZE - 1) / layout::PAGE_SIZE) * layout::PAGE_SIZE;
    let mut bump = ALLOCATOR.0.lock();
    bump.next = heap_start;
    bump.end = heap_start + KERNEL_HEAP_SIZE;
}

pub fn stats() -> (usize, usize) {
    let bump = ALLOCATOR.0.lock();
    (bump.next - layout::kernel_end(), KERNEL_HEAP_SIZE)
}
