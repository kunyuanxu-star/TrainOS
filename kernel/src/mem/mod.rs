pub mod bitmanip;
pub mod buddy;
pub mod cache_ops;
pub mod heap;
pub mod layout;
pub mod mseal;
pub mod ptr_mask;
pub mod sv39;
pub mod sv48;
pub mod svinval;
pub mod svnapot;
pub mod svpbmt;
pub mod txmmu;
pub mod vector;

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
