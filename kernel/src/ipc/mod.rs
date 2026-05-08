pub mod endpoint;
pub mod message;

use alloc::vec::Vec;
use endpoint::Endpoint;
use spin::Mutex;

static ENDPOINTS: Mutex<Vec<Option<Endpoint>>> = Mutex::new(Vec::new());
static NEXT_EP_ID: Mutex<usize> = Mutex::new(1);

pub fn init() {
    let mut eps = ENDPOINTS.lock();
    eps.push(None); // slot 0 unused
    drop(eps); // release lock before create_endpoint which also locks

    // Pre-create well-known endpoints for system services.
    // EP 1 is the init service endpoint (init hardcodes recv(1, ...)).
    // EP 2 is the FS service endpoint (FS will be changed to recv(2, ...)).
    create_endpoint(); // EP 1
    create_endpoint(); // EP 2
}

pub fn create_endpoint() -> usize {
    let id;
    {
        let mut next = NEXT_EP_ID.lock();
        id = *next;
        *next += 1;
    }
    let mut eps = ENDPOINTS.lock();
    while eps.len() <= id {
        eps.push(None);
    }
    eps[id] = Some(Endpoint::new(id));
    id
}
