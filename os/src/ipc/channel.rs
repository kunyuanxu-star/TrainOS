//! IPC channel operations

use crate::ipc::{Pid, PortId, message::IpcMessage, MAX_MESSAGE_SIZE};

/// Send a message to a process/port
/// Returns 0 on success, -1 on error
pub fn send(to_pid: Pid, port: PortId, data: &[u8]) -> isize {
    if data.len() > MAX_MESSAGE_SIZE - 16 {
        return -1; // Message too large
    }

    let from_pid = get_current_pid();

    // Look up endpoint
    let entry = match crate::ipc::endpoint::lookup_endpoint(port) {
        Some(e) => e,
        None => return -1, // No such endpoint
    };

    if entry.owner_pid != to_pid {
        return -1; // Endpoint doesn't belong to target
    }

    // TODO: Task 2 will add process mailbox - for now just return success
    // Get target process and add message to its mailbox
    let _ = to_pid;
    let _ = port;
    let _ = from_pid;
    let _ = data;
    0
}

/// Receive a message from a port (blocking)
/// Returns number of bytes read, or -1 on error
pub fn recv(port: PortId, buf: &mut [u8]) -> isize {
    let pid = get_current_pid();

    // Verify this endpoint belongs to us
    let entry = match crate::ipc::endpoint::lookup_endpoint(port) {
        Some(e) => e,
        None => return -1,
    };

    if entry.owner_pid != pid {
        return -1; // Not our endpoint
    }

    // TODO: Task 2 will implement blocking recv with actual mailbox
    // For now, return error as mailbox is not yet implemented
    let _ = buf;
    -1
}

// Stub functions for process management (will be implemented in Task 2)

/// Get current process ID
fn get_current_pid() -> Pid {
    // TODO: get from process module - for now return 0
    0
}

/// Yield the current process (for blocking recv)
fn yield_current() {
    // TODO: Task 2 will implement actual yield
}
