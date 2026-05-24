pub mod buddy;
pub mod heap;
pub mod layout;
pub mod sv39;
pub mod txmmu;

use layout::allocatable_range;

const KERNEL_HEAP_SIZE: usize = 4 * 1024 * 1024; // 4MB

pub fn init() {
    let (start, end) = allocatable_range();

    // Kernel heap starts at the first allocatable page
    let heap_start = start;
    let heap_end = start + KERNEL_HEAP_SIZE;

    // Buddy allocator manages pages AFTER the kernel heap to avoid overlap
    let buddy_start = heap_end;
    buddy::init(buddy_start, end);

    heap::init_range(heap_start, heap_end);
    sv39::init_root_pt();
    unsafe {
        sv39::setup_kernel_mapping();
    }
}
