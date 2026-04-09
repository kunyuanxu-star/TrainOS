//! IPC (Inter-Process Communication) module
//!
//! Provides message passing channels between processes.

pub mod channel;
pub mod endpoint;
pub mod message;

use spin::Mutex;

// Re-export for convenience
pub use message::{IpcMessage, MessageHeader, MAX_MESSAGE_SIZE};
pub use endpoint::{lookup_endpoint, create_endpoint, delete_endpoint};

// Maximum number of endpoints in the system
pub const MAX_ENDPOINTS: usize = 256;

// Endpoint table - maps port ID to endpoint info
static ENDPOINT_TABLE: Mutex<EndpointTable> = Mutex::new(EndpointTable::new());

// Next available port ID
static NEXT_PORT: Mutex<PortId> = Mutex::new(PortId::MIN);

pub type PortId = u32;
pub type Pid = u32;

/// Endpoint table entry
#[derive(Debug, Clone, Copy)]
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
