use core::alloc::{GlobalAlloc, Layout};

// Minimal kernel heap using a bump allocator with a fixed region.
// The region starts at _kernel_end and extends for KERNEL_HEAP_SIZE bytes.
//
// V21.8: Canary protection — each allocation is guarded by 8-byte canaries
// both before and after the user payload to detect heap corruption.

use spin::Mutex;

const HEAP_CANARY: u64 = 0xDEAD_BEEF_CAFE_BABE;

struct BumpAllocator {
    next: usize,
    end: usize,
    allocations: usize,
}

impl BumpAllocator {
    const fn new() -> Self {
        BumpAllocator {
            next: 0,
            end: 0,
            allocations: 0,
        }
    }
}

/// Newtype wrapper to satisfy orphan rule for GlobalAlloc.
struct KernelAllocator(Mutex<BumpAllocator>);

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.0.lock();
        let aligned = (bump.next + layout.align() - 1) & !(layout.align() - 1);
        // payload starts 8 bytes after the before-canary
        let payload = aligned + 8;
        let payload_end = payload.checked_add(layout.size()).unwrap();
        // after-canary occupies 8 bytes after the payload
        let total_end = payload_end + 8;
        if total_end > bump.end {
            return core::ptr::null_mut();
        }
        // Write before-canary at aligned position
        (aligned as *mut u64).write_volatile(HEAP_CANARY);
        // Write after-canary immediately after the user payload
        (payload_end as *mut u64).write_volatile(HEAP_CANARY);
        bump.next = total_end;
        bump.allocations += 1;
        payload as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut bump = self.0.lock();
        bump.allocations -= 1;
        // Verify before-canary (8 bytes before the user pointer)
        let before = (ptr as usize - 8) as *const u64;
        // Verify after-canary (at the end of the user payload)
        let after = (ptr as usize + layout.size()) as *const u64;
        if before.read_volatile() != HEAP_CANARY || after.read_volatile() != HEAP_CANARY {
            crate::println!("HEAP: canary corrupted");
            crate::idle_loop();
        }
    }
}

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator(Mutex::new(BumpAllocator::new()));

pub fn init_range(start: usize, end: usize) {
    let mut bump = ALLOCATOR.0.lock();
    bump.next = start;
    bump.end = end;
}
