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

// Filesystem constants
const SECTOR_SIZE: usize = 512;
const MAGIC: u32 = 0x4B53544F; // "KSTO"
const INODE_START_SECTOR: usize = 1;
const DATA_START_SECTOR: usize = 64;
const NUM_INODES: usize = 63;

/// Superblock (sector 0)
#[repr(C)]
#[derive(Debug, Clone)]
struct Superblock {
    magic: u32,
    block_size: u32,
    num_blocks: u32,
    inode_start: u32,
    data_start: u32,
}

/// Inode (one per sector, sectors 1-63)
#[repr(C)]
#[derive(Debug, Clone)]
struct Inode {
    used: bool,
    size: u32,
    blocks: [u64; 8],  // 8 direct block pointers
}

/// Simple RAM filesystem
struct SimpleFS {
    driver_pid: u32,
}

impl SimpleFS {
    /// Read a block from storage via driver
    fn read_block(&self, sector: u64, buf: &mut [u8]) -> i32 {
        let driver_pid = self.driver_pid;
        if driver_pid == 0 {
            return -1;
        }

        // Create request buffer
        let mut req: [u8; 32] = [0; 32];
        unsafe {
            *(req.as_mut_ptr() as *mut u32) = 0; // op = read
            *(req.as_mut_ptr().add(4) as *mut u64) = sector;
            *(req.as_mut_ptr().add(12) as *mut u32) = 1; // count = 1
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

        // Wait for response
        let mut resp_buf: [u8; 516] = [0; 516];
        let resp_size = syscall(SYS_RECV as usize,
                               reply_port as usize,
                               resp_buf.as_mut_ptr() as usize,
                               516,
                               0, 0, 0) as usize;

        let _ = syscall(SYS_ENDPOINT_DELETE as usize, reply_port as usize, 0, 0, 0, 0, 0);

        if resp_size < 4 {
            return -1;
        }

        let status = unsafe { *(resp_buf.as_ptr() as *const i32) };
        if status == 0 && resp_size >= 516 {
            let copy_len = buf.len().min(512);
            buf[..copy_len].copy_from_slice(&resp_buf[4..4 + copy_len]);
        }

        status
    }

    /// Write a block to storage via driver
    fn write_block(&self, sector: u64, data: &[u8]) -> i32 {
        let driver_pid = self.driver_pid;
        if driver_pid == 0 {
            return -1;
        }

        // Create request buffer
        let mut req: [u8; 544] = [0; 544];
        unsafe {
            *(req.as_mut_ptr() as *mut u32) = 1; // op = write
            *(req.as_mut_ptr().add(4) as *mut u64) = sector;
            *(req.as_mut_ptr().add(12) as *mut u32) = 1; // count = 1
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

        // Send request
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

        // Wait for response
        let mut resp_buf: [u8; 4] = [0; 4];
        let resp_size = syscall(SYS_RECV as usize,
                               reply_port as usize,
                               resp_buf.as_mut_ptr() as usize,
                               4,
                               0, 0, 0) as usize;

        let _ = syscall(SYS_ENDPOINT_DELETE as usize, reply_port as usize, 0, 0, 0, 0, 0);

        if resp_size < 4 {
            return -1;
        }

        unsafe { *(resp_buf.as_ptr() as *const i32) }
    }

    /// Format the filesystem (write superblock)
    fn format(&mut self) -> i32 {
        let sb = Superblock {
            magic: MAGIC,
            block_size: SECTOR_SIZE as u32,
            num_blocks: 1024,
            inode_start: INODE_START_SECTOR as u32,
            data_start: DATA_START_SECTOR as u32,
        };

        let sb_bytes: &[u8] = unsafe {
            core::slice::from_raw_parts(&sb as *const _ as *const u8, SECTOR_SIZE)
        };
        self.write_block(0, sb_bytes)
    }

    /// Read and verify superblock
    fn read_superblock(&self) -> Option<Superblock> {
        let mut buf = [0u8; SECTOR_SIZE];
        if self.read_block(0, &mut buf) < 0 {
            return None;
        }
        let sb: Superblock = unsafe { core::ptr::read(buf.as_ptr() as *const _) };
        if sb.magic != MAGIC {
            return None;
        }
        Some(sb)
    }

    /// Find an inode by name (simple linear search)
    /// Returns inode index (0-62) or None
    fn find_inode(&self, _name: &str) -> Option<usize> {
        // For simplicity, return inode 0 as the "root" inode
        // Real implementation would search directory entries
        Some(0)
    }

    /// Read an inode
    fn read_inode(&self, idx: usize) -> Option<Inode> {
        if idx >= NUM_INODES {
            return None;
        }
        let mut buf = [0u8; SECTOR_SIZE];
        if self.read_block((INODE_START_SECTOR + idx) as u64, &mut buf) < 0 {
            return None;
        }
        let inode: Inode = unsafe { core::ptr::read(buf.as_ptr() as *const _) };
        Some(inode)
    }

    /// Write an inode
    fn write_inode(&self, idx: usize, inode: &Inode) -> i32 {
        if idx >= NUM_INODES {
            return -1;
        }
        let bytes: &[u8] = unsafe {
            core::slice::from_raw_parts(inode as *const _ as *const u8, SECTOR_SIZE)
        };
        self.write_block((INODE_START_SECTOR + idx) as u64, bytes)
    }

    /// Allocate a new inode, returns index or None
    fn alloc_inode(&self) -> Option<usize> {
        for i in 0..NUM_INODES {
            let mut buf = [0u8; SECTOR_SIZE];
            if self.read_block((INODE_START_SECTOR + i) as u64, &mut buf) < 0 {
                continue;
            }
            let inode: Inode = unsafe { core::ptr::read(buf.as_ptr() as *const _) };
            if !inode.used {
                return Some(i);
            }
        }
        None
    }

    /// Open a file by name, returns fd or None
    fn open(&self, name: &str) -> Option<usize> {
        // Find or create inode
        let inode_idx = self.find_inode(name).or_else(|| self.alloc_inode())?;
        let inode = self.read_inode(inode_idx)?;

        // Allocate fd
        let fd = alloc_fd()?;
        OPEN_FILES[fd] = Some(FileDescriptor {
            inode: inode_idx as u64,
            offset: 0,
            flags: 0,
        });

        Some(fd)
    }

    /// Read from file descriptor
    fn read(&self, fd: usize, buf: &mut [u8]) -> i32 {
        let file = match OPEN_FILES[fd] {
            Some(ref f) => f,
            None => return -1,
        };

        let inode = match self.read_inode(file.inode as usize) {
            Some(i) => i,
            None => return -1,
        };

        if !inode.used {
            return -1;
        }

        // Read from direct blocks
        let mut offset = file.offset as usize;
        let mut remaining = buf.len();
        let mut buf_offset = 0;

        for &block_num in inode.blocks.iter() {
            if block_num == 0 {
                break;
            }
            if remaining == 0 {
                break;
            }

            let mut block_buf = [0u8; SECTOR_SIZE];
            if self.read_block(block_num, &mut block_buf) < 0 {
                break;
            }

            let block_offset = offset.min(SECTOR_SIZE - 1);
            let available = SECTOR_SIZE - block_offset;
            let to_copy = remaining.min(available);

            buf[buf_offset..buf_offset + to_copy]
                .copy_from_slice(&block_buf[block_offset..block_offset + to_copy]);

            buf_offset += to_copy;
            remaining -= to_copy;
            offset = 0; // Reset offset after first block
        }

        // Update file offset
        if let Some(ref mut f) = OPEN_FILES[fd] {
            f.offset += (buf.len() - remaining) as u64;
        }

        (buf.len() - remaining) as i32
    }

    /// Write to file descriptor
    fn write(&self, fd: usize, data: &[u8]) -> i32 {
        let file = match OPEN_FILES[fd] {
            Some(ref f) => f,
            None => return -1,
        };

        // For simplicity, only support writing to existing files
        let mut inode = match self.read_inode(file.inode as usize) {
            Some(i) => i,
            None => return -1,
        };

        if !inode.used {
            return -1;
        }

        // Find first free block or allocate
        let mut block_idx = 0;
        while block_idx < 8 && inode.blocks[block_idx] != 0 {
            block_idx += 1;
        }

        if block_idx >= 8 {
            return -1; // No more direct blocks
        }

        // Allocate block (for simplicity, use data_start + inode_idx)
        let block_num = DATA_START_SECTOR as u64 + file.inode;
        inode.blocks[block_idx] = block_num;

        // Write data to block
        let mut block_buf = [0u8; SECTOR_SIZE];
        let copy_len = data.len().min(SECTOR_SIZE);
        block_buf[..copy_len].copy_from_slice(&data[..copy_len]);

        if self.write_block(block_num, &block_buf) < 0 {
            return -1;
        }

        // Update inode size
        inode.size = (file.offset as u32 + copy_len as u32);
        self.write_inode(file.inode as usize, &inode);

        // Update file offset
        if let Some(ref mut f) = OPEN_FILES[fd] {
            f.offset += copy_len as u64;
        }

        copy_len as i32
    }
}

/// Driver PID (will be set when driver spawns)
static DRIVER_PID: spin::Mutex<u32> = spin::Mutex::new(0);

/// Set driver PID (called after driver spawns) - DEPRECATED
/// Driver PID is now passed directly to fs_init()
#[allow(dead_code)]
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

/// Global FS instance (SimpleFS)
static FS: spin::Mutex<Option<SimpleFS>> = spin::Mutex::new(None);

/// Initialize FS service
fn fs_init(driver_pid: u32) {
    let mut fs = SimpleFS { driver_pid };
    // Format the filesystem (write superblock)
    let _ = fs.format();
    *FS.lock() = Some(fs);
}

/// Handle open request
fn handle_open(path: &str, flags: u32) -> Option<usize> {
    let fs_guard = FS.lock();
    let fs = fs_guard.as_ref()?;
    fs.open(path)
}

/// Handle read request
fn handle_read(fd: usize, buf: &mut [u8]) -> i32 {
    let fs_guard = FS.lock();
    let fs = match fs_guard.as_ref() {
        Some(f) => f,
        None => return -1,
    };
    fs.read(fd, buf)
}

/// Handle write request
fn handle_write(fd: usize, data: &[u8]) -> i32 {
    let fs_guard = FS.lock();
    let fs = match fs_guard.as_ref() {
        Some(f) => f,
        None => return -1,
    };
    fs.write(fd, data)
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
    // 1. Create endpoint on FS_PORT for receiving requests
    let fs_port = syscall(SYS_ENDPOINT_CREATE as usize, 0, 0, 0, 0, 0, 0) as u32;

    // 2. Wait for init to send us the driver PID via IPC
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

    let driver_pid = if size > 0 {
        // Parse message: first 4 bytes is the driver PID
        unsafe { *(recv_buf.as_ptr() as *const u32) }
    } else {
        0
    };

    // 3. Initialize FS with driver PID
    fs_init(driver_pid);

    // 4. Main loop: receive requests, process, respond
    loop {
        syscall(SYS_SCHED_YIELD as usize, 0, 0, 0, 0, 0, 0);
        unsafe { core::arch::asm!("wfi"); }
    }
}
