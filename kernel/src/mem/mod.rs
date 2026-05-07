pub mod buddy;
pub mod heap;
pub mod layout;
pub mod sv39;

use layout::allocatable_range;

pub fn init() {
    let (start, end) = allocatable_range();
    buddy::init(start, end);
    heap::init();
    sv39::init_root_pt();
    unsafe { sv39::setup_kernel_mapping(); }
}
