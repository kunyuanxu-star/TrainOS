pub mod ops;
pub mod types;

use alloc::vec::Vec;
use spin::Mutex;
use types::Resource;

static RESOURCES: Mutex<Vec<Option<Resource>>> = Mutex::new(Vec::new());
static NEXT_RESOURCE_ID: Mutex<usize> = Mutex::new(1);

pub fn init() {
    let mut res = RESOURCES.lock();
    res.push(None); // slot 0 = null
}
