//! FS service - file system service
//!
//! Provides file operations by translating them to block I/O
//! and sending requests to driver_server.

#![no_std]

pub mod protocol;

use protocol::*;

// Syscall numbers
const SYS_ENDPOINT_CREATE: usize = 1000;
const SYS_ENDPOINT_DELETE: usize = 1001;
const SYS_SEND: usize = 1002;
const SYS_RECV: usize = 1003;
const SYS_SCHED_YIELD: usize = 124;

/// Driver PID (will be set when driver spawns)
static DRIVER_PID: spin::Mutex<u32> = spin::Mutex::new(0);

/// Set driver PID (called after driver spawns)
pub fn set_driver_pid(pid: u32) {
    *DRIVER_PID.lock() = pid;
}

/// Make a syscall
fn syscall(n: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize) -> isize {
    let ret;
    unsafe {
        core::arch::asm!(
            "mv a7, {syscall_num}",
            "mv a0, {arg0}; mv a1, {arg1}; mv a2, {arg2}; mv a3, {arg3}; mv a4, {arg4}; mv a5, {arg5}",
            "ecall",
            lateout("a0") ret,
            arg0 = in(reg) a0,
            arg1 = in(reg) a1,
            arg2 = in(reg) a2,
            arg3 = in(reg) a3,
            arg4 = in(reg) a4,
            arg5 = in(reg) a5,
            syscall_num = in(reg) n,
        );
    }
    ret
}

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
fn block_read(sector: u64, count: u32, buf: &mut [u8]) -> i32 {
    let driver_pid = *DRIVER_PID.lock();
    if driver_pid == 0 {
        return -1; // Driver not ready
    }

    // Create request buffer (32 bytes header + 512 data = 544)
    let mut req: [u8; 32] = [0; 32];

    // Pack request: op(4) + sector(8) + count(4) + reply_port(4) + padding(12) = 32
    unsafe {
        *(req.as_mut_ptr() as *mut u32) = 0; // op = read
        *(req.as_mut_ptr().add(4) as *mut u64) = sector;
        *(req.as_mut_ptr().add(12) as *mut u32) = count;
    }

    // Create ephemeral reply port
    let reply_port = syscall(SYS_ENDPOINT_CREATE as usize, 0, 0, 0, 0, 0, 0) as u32;
    if reply_port < 2 {
        return -1;
    }
    unsafe {
        *(req.as_mut_ptr().add(16) as *mut u32) = reply_port;
    }

    // Send request to driver
    let result = syscall(SYS_SEND as usize,
                         driver_pid as usize,
                         DRIVER_PORT as usize,
                         req.as_ptr() as usize,
                         32,
                         0, 0);
    if result < 0 {
        let _ = syscall(SYS_ENDPOINT_DELETE as usize, reply_port as usize, 0, 0, 0, 0, 0);
        return -1;
    }

    // Wait for response (4 bytes status + 512 bytes data = 516)
    let mut resp_buf: [u8; 516] = [0; 516];
    let resp_size = syscall(SYS_RECV as usize,
                           reply_port as usize,
                           resp_buf.as_mut_ptr() as usize,
                           516,
                           0, 0, 0) as usize;

    // Clean up reply port
    let _ = syscall(SYS_ENDPOINT_DELETE as usize, reply_port as usize, 0, 0, 0, 0, 0);

    if resp_size < 4 {
        return -1;
    }

    // Parse response: status(4) + data(512)
    let status = unsafe { *(resp_buf.as_ptr() as *const i32) };
    if status == 0 && resp_size >= 516 {
        // Copy data to buffer
        let copy_len = buf.len().min(512);
        buf[..copy_len].copy_from_slice(&resp_buf[4..4 + copy_len]);
    }

    status
}

/// Send block write request to driver
fn block_write(sector: u64, count: u32, data: &[u8]) -> i32 {
    let driver_pid = *DRIVER_PID.lock();
    if driver_pid == 0 {
        return -1;
    }

    // Create request buffer (32 header + up to 512 data = 544)
    let mut req: [u8; 544] = [0; 544];

    unsafe {
        *(req.as_mut_ptr() as *mut u32) = 1; // op = write
        *(req.as_mut_ptr().add(4) as *mut u64) = sector;
        *(req.as_mut_ptr().add(12) as *mut u32) = count;
    }

    // Copy data
    let copy_len = data.len().min(512);
    req[16..16 + copy_len].copy_from_slice(&data[..copy_len]);

    // Create reply port
    let reply_port = syscall(SYS_ENDPOINT_CREATE as usize, 0, 0, 0, 0, 0, 0) as u32;
    if reply_port < 2 {
        return -1;
    }
    unsafe {
        *(req.as_mut_ptr().add(16) as *mut u32) = reply_port;
    }

    // Send request (header is 32 bytes, then data)
    let result = syscall(SYS_SEND as usize,
                         driver_pid as usize,
                         DRIVER_PORT as usize,
                         req.as_ptr() as usize,
                         32 + copy_len,
                         0, 0);
    if result < 0 {
        let _ = syscall(SYS_ENDPOINT_DELETE as usize, reply_port as usize, 0, 0, 0, 0, 0);
        return -1;
    }

    // Wait for response (4 bytes status)
    let mut resp_buf: [u8; 4] = [0; 4];
    let resp_size = syscall(SYS_RECV as usize,
                           reply_port as usize,
                           resp_buf.as_mut_ptr() as usize,
                           4,
                           0, 0, 0) as usize;

    // Clean up reply port
    let _ = syscall(SYS_ENDPOINT_DELETE as usize, reply_port as usize, 0, 0, 0, 0, 0);

    if resp_size < 4 {
        return -1;
    }

    unsafe { *(resp_buf.as_ptr() as *const i32) }
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

    // 2. Create endpoint on FS_PORT for receiving requests
    let fs_port = syscall(SYS_ENDPOINT_CREATE as usize, 0, 0, 0, 0, 0, 0) as u32;

    // 3. Wait for init to send us the driver PID via IPC
    // For now: yield a few times then set a placeholder
    for _ in 0..10 {
        syscall(SYS_SCHED_YIELD as usize, 0, 0, 0, 0, 0, 0);
    }

    // Create a buffer to receive driver PID message
    let mut recv_buf: [u8; 32] = [0; 32];
    let size = syscall(SYS_RECV as usize,
                       fs_port as usize,
                       recv_buf.as_mut_ptr() as usize,
                       32,
                       0, 0, 0) as usize;

    if size > 0 {
        // Parse message: first 4 bytes is the driver PID
        let pid = unsafe { *(recv_buf.as_ptr() as *const u32) };
        set_driver_pid(pid);
    } else {
        // No message received - driver PID not provided via IPC
        // Will need to be set externally or via another mechanism
    }

    // 4. Main loop: receive requests, process, respond
    loop {
        // For now, just yield
        syscall(SYS_SCHED_YIELD as usize, 0, 0, 0, 0, 0, 0);
        unsafe { core::arch::asm!("wfi"); }
    }
}
