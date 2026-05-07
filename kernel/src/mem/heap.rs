use core::alloc::{GlobalAlloc, Layout};

/// Minimal kernel heap using a bump allocator with a fixed region.
/// The region starts at _kernel_end and extends for KERNEL_HEAP_SIZE bytes.

use spin::Mutex;

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

pub fn init_range(start: usize, end: usize) {
    let mut bump = ALLOCATOR.0.lock();
    bump.next = start;
    bump.end = end;
}

