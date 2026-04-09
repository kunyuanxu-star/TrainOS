//! FS service IPC protocol
//!
//! Defines messages exchanged between fs_server and driver_server.

/// Driver service port
pub const DRIVER_PORT: u32 = 2;

/// FS service port
pub const FS_PORT: u32 = 3;

/// Block I/O command types
#[derive(Debug, Clone, Copy)]
pub enum BlockOp {
    Read = 0,
    Write = 1,
}

/// Maximum sectors per request
pub const MAX_SECTORS: usize = 1;

/// Block read request (FS -> Driver)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct BlockReadRequest {
    pub sector: u64,
    pub count: u32,
    pub reply_port: u32,
}

/// Block read response (Driver -> FS)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct BlockReadResponse {
    pub status: i32,
    pub data: [u8; 512],
}

/// Block write request (FS -> Driver)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct BlockWriteRequest {
    pub sector: u64,
    pub count: u32,
    pub reply_port: u32,
    pub data: [u8; 512],
}

/// Block write response (Driver -> FS)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct BlockWriteResponse {
    pub status: i32,
}

/// IPC message size limits
pub const MAX_IPC_PAYLOAD: usize = 256;