//! Endpoint management

use crate::ipc::{PortId, Pid, ENDPOINT_TABLE, MAX_ENDPOINTS, EndpointEntry};

/// Create a new endpoint for a process
/// Returns (port_id, endpoint_entry) on success
pub fn create_endpoint(owner_pid: Pid) -> Option<(PortId, EndpointEntry)> {
    let mut table = ENDPOINT_TABLE.lock();
    let mut next_port = crate::ipc::NEXT_PORT.lock();

    // Find a free slot
    for i in 0..MAX_ENDPOINTS {
        let port = (*next_port + i as PortId) % (PortId::MAX as usize) as PortId;
        if port == 0 {
            continue; // Skip port 0 (reserved)
        }
        if table.entries[port as usize].is_none() {
            let entry = EndpointEntry {
                owner_pid,
                port,
                valid: true,
            };
            table.entries[port as usize] = Some(entry);
            *next_port = port.wrapping_add(1);
            return Some((port, entry));
        }
    }
    None
}

/// Look up an endpoint by port
pub fn lookup_endpoint(port: PortId) -> Option<EndpointEntry> {
    let table = ENDPOINT_TABLE.lock();
    table.entries[port as usize].clone()
}

/// Delete an endpoint
pub fn delete_endpoint(port: PortId, owner_pid: Pid) -> bool {
    let mut table = ENDPOINT_TABLE.lock();
    if let Some(ref entry) = table.entries[port as usize] {
        if entry.owner_pid == owner_pid && entry.valid {
            table.entries[port as usize] = None;
            return true;
        }
    }
    false
}
