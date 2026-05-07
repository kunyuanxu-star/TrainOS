pub mod message;
pub mod endpoint;

use endpoint::Endpoint;
use alloc::vec::Vec;
use spin::Mutex;

static ENDPOINTS: Mutex<Vec<Option<Endpoint>>> = Mutex::new(Vec::new());
static NEXT_EP_ID: Mutex<usize> = Mutex::new(1);

pub fn init() {
    let mut eps = ENDPOINTS.lock();
    eps.push(None); // slot 0 unused
}

pub fn create_endpoint() -> usize {
    let id;
    {
        let mut next = NEXT_EP_ID.lock();
        id = *next;
        *next += 1;
    }
    let mut eps = ENDPOINTS.lock();
    while eps.len() <= id { eps.push(None); }
    eps[id] = Some(Endpoint::new(id));
    id
}
