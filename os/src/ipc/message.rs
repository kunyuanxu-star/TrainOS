//! IPC message structures

use crate::ipc::{Pid, PortId};

/// Maximum message size (4KB - one page)
pub const MAX_MESSAGE_SIZE: usize = 4096;

/// Message header (16 bytes)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MessageHeader {
    pub from: Pid,        // Source PID
    pub to: Pid,          // Destination PID
    pub port: PortId,     // Destination port
    pub size: u32,       // Payload size
    pub reply_port: PortId, // Reply port (0 if no reply expected)
}

/// Full IPC message with header and inline payload
/// Total size: 16 + 4080 = 4096 bytes (one page)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IpcMessage {
    pub header: MessageHeader,
    pub payload: [u8; MAX_MESSAGE_SIZE - 16],
}

impl IpcMessage {
    pub fn new(from: Pid, to: Pid, port: PortId, size: u32) -> Self {
        Self {
            header: MessageHeader {
                from,
                to,
                port,
                size,
                reply_port: 0,
            },
            payload: [0; MAX_MESSAGE_SIZE - 16],
        }
    }

    pub fn with_reply(from: Pid, to: Pid, port: PortId, size: u32, reply_port: PortId) -> Self {
        let mut msg = Self::new(from, to, port, size);
        msg.header.reply_port = reply_port;
        msg
    }
}
