//! IPC (Inter-Process Communication) module
//!
//! Provides message passing channels between processes.

pub mod channel;
pub mod endpoint;
pub mod message;

use spin::Mutex;

// Maximum number of endpoints in the system
pub const MAX_ENDPOINTS: usize = 256;

// Endpoint table - maps port ID to endpoint info
static ENDPOINT_TABLE: Mutex<EndpointTable> = Mutex::new(EndpointTable::new());

// Next available port ID
static NEXT_PORT: Mutex<PortId> = Mutex::new(PortId::min());

pub type PortId = u32;
pub type Pid = u32;

/// Endpoint table entry
#[derive(Debug, Clone)]
pub struct EndpointEntry {
    pub owner_pid: Pid,
    pub port: PortId,
    pub valid: bool,
}

pub struct EndpointTable {
    entries: [Option<EndpointEntry>; MAX_ENDPOINTS],
}

impl EndpointTable {
    pub const fn new() -> Self {
        Self {
            entries: [None; MAX_ENDPOINTS],
        }
    }
}
