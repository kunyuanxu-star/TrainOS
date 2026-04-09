//! FS service - file system service
//!
//! Provides file operations by translating them to block I/O
//! and sending requests to driver_server.

#![no_std]

pub mod protocol;

use protocol::*;

/// FS inode number
pub type Inode = u64;

/// File descriptor
#[derive(Debug)]
pub struct FileDescriptor {
    pub inode: Inode,
    pub offset: u64,
    pub flags: u32,
}

/// Maximum open files
const MAX_FILES: usize = 32;

/// Open file table
static OPEN_FILES: [Option<FileDescriptor>; MAX_FILES] = [None; MAX_FILES];

/// Find free fd
fn alloc_fd() -> Option<usize> {
    for i in 0..MAX_FILES {
        if OPEN_FILES[i].is_none() {
            return Some(i);
        }
    }
    None
}

/// Send block read request to driver
///
/// Phase 3: Stub implementation - returns -1 (not implemented)
/// Phase 4: Will send BlockReadRequest to DRIVER_PORT and wait for response
fn block_read(sector: u64, count: u32, buf: &mut [u8]) -> i32 {
    // TODO(Phase 4): Implement IPC to driver
    // 1. Create ephemeral reply port
    // 2. Send BlockReadRequest { sector, count, reply_port } to DRIVER_PORT
    // 3. Receive BlockReadResponse on reply port
    // 4. Copy data to buf and return status
    -1
}

/// Send block write request to driver
///
/// Phase 3: Stub implementation - returns -1 (not implemented)
/// Phase 4: Will send BlockWriteRequest to DRIVER_PORT and wait for response
fn block_write(sector: u64, count: u32, data: &[u8]) -> i32 {
    // TODO(Phase 4): Implement IPC to driver
    -1
}

/// Initialize FS service
fn fs_init() {
    // Open the root device
    // Mount the root file system
}

/// Handle open request
fn handle_open(path: &str, flags: u32) -> Option<usize> {
    let fd = alloc_fd()?;

    // For now, just return a dummy file descriptor
    OPEN_FILES[fd] = Some(FileDescriptor {
        inode: 0,
        offset: 0,
        flags,
    });

    Some(fd)
}

/// Handle read request
fn handle_read(fd: usize, buf: &mut [u8]) -> i32 {
    if fd >= MAX_FILES || OPEN_FILES[fd].is_none() {
        return -1;
    }

    let file = OPEN_FILES[fd].as_mut().unwrap();

    // For now, return empty
    0
}

/// Handle write request
fn handle_write(fd: usize, data: &[u8]) -> i32 {
    if fd >= MAX_FILES || OPEN_FILES[fd].is_none() {
        return -1;
    }

    let file = OPEN_FILES[fd].as_mut().unwrap();

    // For now, return success
    data.len() as i32
}

/// Handle close request
fn handle_close(fd: usize) -> i32 {
    if fd >= MAX_FILES || OPEN_FILES[fd].is_none() {
        return -1;
    }

    OPEN_FILES[fd] = None;
    0
}

/// FS service main loop
fn fs_service_main() {
    // 1. Initialize FS
    fs_init();

    // 2. Create endpoint for FS_PORT
    // let fs_port = syscall::endpoint_create(FS_PORT as usize);

    // 3. Register with init (give our PID to init)

    // 4. Main loop: receive requests, process, respond
    loop {
        // Receive request on FS_PORT
        // match request.type {
        //     BlockOp::Read => handle_block_read(...),
        //     BlockOp::Write => handle_block_write(...),
        // }

        // For now, just yield
        unsafe { core::arch::asm!("wfi"); }
    }
}
