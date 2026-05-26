// V22+V35: io_uring-style async I/O subsystem with DMABUF/zero-copy enhancement
//
// Architecture:
//   - Per-process submission queue (SQ) and completion queue (CQ)
//   - Shared memory ring buffers mapped into user space
//   - Operations: READ, WRITE, OPEN, CLOSE, STAT, NOP
//   - V35: Zero-copy send/recv, buffer pool, splice, RWF flags
//   - Zero-copy via shared memory page transfer

use crate::mem::layout::PAGE_SIZE;

const MAX_RINGS: usize = 16;
const RING_ENTRIES: usize = 32;
const MAX_BUF_POOLS: usize = 8;

#[repr(C)]
#[derive(Clone, Copy)]
struct IoUringSqe {
    opcode: u8,
    flags: u8,
    fd: u32,
    addr: u64,
    len: u32,
    off: u64,
    user_data: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct IoUringCqe {
    user_data: u64,
    res: i32,
    flags: u32,
}

#[derive(Clone, Copy)]
struct IoUring {
    pid: u32,
    sq_entries: usize,
    cq_entries: usize,
    sq_head: usize,
    sq_tail: usize,
    cq_head: usize,
    sqes: [IoUringSqe; RING_ENTRIES],
    cqes: [IoUringCqe; RING_ENTRIES],
    active: bool,
    sq_phys: usize,   // physical address of shared SQ ring page
    cq_phys: usize,   // physical address of shared CQ ring page
    sq_va: usize,     // user-space virtual address of SQ page
    cq_va: usize,     // user-space virtual address of CQ page
}

static mut RINGS: [IoUring; MAX_RINGS] = [
    IoUring { pid: 0, sq_entries: 0, cq_entries: 0, sq_head: 0, sq_tail: 0,
              cq_head: 0, sqes: [IoUringSqe { opcode: 0, flags: 0, fd: 0, addr: 0, len: 0, off: 0, user_data: 0 }; RING_ENTRIES],
              cqes: [IoUringCqe { user_data: 0, res: 0, flags: 0 }; RING_ENTRIES], active: false,
              sq_phys: 0, cq_phys: 0, sq_va: 0, cq_va: 0 }; MAX_RINGS
];
static mut RING_COUNT: usize = 0;

// Opcodes (V22 base)
pub const IORING_OP_NOP: u8 = 0;
pub const IORING_OP_READ: u8 = 1;
pub const IORING_OP_WRITE: u8 = 2;
pub const IORING_OP_OPEN: u8 = 3;
pub const IORING_OP_CLOSE: u8 = 4;
pub const IORING_OP_STAT: u8 = 5;

// V35: Zero-copy / DMABUF opcodes
pub const IORING_OP_SEND_ZC: u8 = 10;          // Zero-copy send
pub const IORING_OP_RECV_ZC: u8 = 11;          // Zero-copy receive (pre-registered buffer)
pub const IORING_OP_SPLICE: u8 = 12;           // Splice between fds
pub const IORING_OP_PROVIDE_BUFFERS: u8 = 13;  // Pre-register buffer pool
pub const IORING_OP_REMOVE_BUFFERS: u8 = 14;   // Remove buffer pool

// ── V35: Buffer Pool for Zero-Copy ─────────────────────────────────────────

/// A pool of pre-registered physical pages for zero-copy receive.
#[derive(Clone, Copy)]
struct IoUringBufPool {
    buf_id: u16,
    ring_id: usize,
    buffers: [usize; 16],    // physical page addresses
    buf_count: usize,
    buf_size: usize,
    used_count: usize,
}

const EMPTY_BUF_POOL: IoUringBufPool = IoUringBufPool {
    buf_id: 0, ring_id: 0,
    buffers: [0; 16], buf_count: 0, buf_size: 0, used_count: 0,
};

static mut BUF_POOLS: [IoUringBufPool; MAX_BUF_POOLS] = [EMPTY_BUF_POOL; MAX_BUF_POOLS];
static mut BUF_POOL_COUNT: usize = 0;

/// Register a buffer pool for zero-copy receive.
/// `ring_id` — the io_uring instance id.
/// `buf_id` — user-chosen identifier for this buffer group.
/// `pages` — array of physical page addresses.
/// `buf_size` — size of each buffer (typically 4096 or 2048).
pub fn provide_buffers(ring_id: usize, buf_id: u16, pages: &[usize], buf_size: usize) {
    unsafe {
        if BUF_POOL_COUNT >= MAX_BUF_POOLS { return; }
        let count = pages.len().min(16);
        BUF_POOLS[BUF_POOL_COUNT] = IoUringBufPool {
            buf_id,
            ring_id,
            buffers: [0; 16],
            buf_count: count,
            buf_size,
            used_count: 0,
        };
        for i in 0..count {
            BUF_POOLS[BUF_POOL_COUNT].buffers[i] = pages[i];
        }
        BUF_POOL_COUNT += 1;
    }
}

/// Remove a previously-provided buffer pool.
pub fn remove_buffers(ring_id: usize, buf_id: u16) {
    unsafe {
        for i in 0..BUF_POOL_COUNT {
            if BUF_POOLS[i].ring_id == ring_id && BUF_POOLS[i].buf_id == buf_id {
                // Shift remaining pools
                for j in i..BUF_POOL_COUNT - 1 {
                    BUF_POOLS[j] = BUF_POOLS[j + 1];
                }
                BUF_POOL_COUNT -= 1;
                return;
            }
        }
    }
}

/// Find a buffer pool by ring_id and buf_id.
fn find_buf_pool(ring_id: usize, buf_id: u16) -> Option<&'static mut IoUringBufPool> {
    unsafe {
        for i in 0..BUF_POOL_COUNT {
            if BUF_POOLS[i].ring_id == ring_id && BUF_POOLS[i].buf_id == buf_id {
                return Some(&mut BUF_POOLS[i]);
            }
        }
    }
    None
}

/// Zero-copy send: pass a physical page address to a net service for transmission.
/// Since TrainOS networking is handled by user-space services (EP 3),
/// this function wraps the send into an IPC message to the net service.
/// Returns the number of bytes sent, or -1 on error.
pub fn send_zc(fd: u32, page: usize, offset: usize, len: usize) -> i32 {
    let pid = unsafe {
        crate::sched::current_thread()
            .map(|t| (*t).owner)
            .unwrap_or(0)
    };

    // Create an IPC message to the net service with the physical page info
    let reply_ep = crate::ipc::create_endpoint();
    let mut msg = crate::ipc::message::Message::new(pid, 10); // ZC_SEND opcode

    msg.payload[0] = reply_ep as u8;
    msg.payload[1] = (reply_ep >> 8) as u8;
    msg.payload[2] = fd as u8;
    msg.payload[3] = (fd >> 8) as u8;
    // Physical page address (low bits)
    msg.payload[4] = page as u8;
    msg.payload[5] = (page >> 8) as u8;
    msg.payload[6] = (page >> 16) as u8;
    msg.payload[7] = (page >> 24) as u8;
    msg.payload[8] = offset as u8;
    msg.payload[9] = (offset >> 8) as u8;
    msg.payload[10] = (offset >> 16) as u8;
    msg.payload[11] = (offset >> 24) as u8;
    msg.payload[12] = len as u8;
    msg.payload[13] = (len >> 8) as u8;
    msg.payload_len = 14;

    // Send to net service (EP 3)
    if crate::ipc::endpoint::send(3, pid, msg).is_err() {
        return -1;
    }

    // Wait for completion
    loop {
        match crate::ipc::endpoint::recv(reply_ep, pid) {
            Ok(resp) => {
                let bytes_sent = resp.payload[0] as usize
                    | ((resp.payload[1] as usize) << 8);
                return bytes_sent as i32;
            }
            Err(_) => { crate::sched::schedule(); }
        }
    }
}

/// Zero-copy receive: fill from a pre-registered buffer pool.
/// Returns the number of bytes received, or -1 on error.
pub fn recv_zc(fd: u32, buf_group: u16) -> i32 {
    let pid = unsafe {
        crate::sched::current_thread()
            .map(|t| (*t).owner)
            .unwrap_or(0)
    };

    // Find the buffer pool and allocate a buffer from it
    let ring_id = 0; // Default ring for buffer lookup
    let pool = find_buf_pool(ring_id, buf_group);
    if pool.is_none() { return -1; }
    let pool = pool.unwrap();

    if pool.used_count >= pool.buf_count {
        return -1; // No available buffers
    }

    // Get a buffer from the pool
    let buf_idx = pool.used_count;
    let page_addr = pool.buffers[buf_idx];
    pool.used_count += 1;

    // For now, do a normal recv from the endpoint and write to the buffer
    // In a full implementation, the net service would DMA directly into this page
    let reply_ep = crate::ipc::create_endpoint();
    let mut msg = crate::ipc::message::Message::new(pid, 11); // ZC_RECV opcode
    msg.payload[0] = reply_ep as u8;
    msg.payload[1] = (reply_ep >> 8) as u8;
    msg.payload[2] = fd as u8;
    msg.payload[3] = buf_group as u8;
    msg.payload_len = 4;

    if crate::ipc::endpoint::send(3, pid, msg).is_err() {
        pool.used_count -= 1;
        return -1;
    }

    loop {
        match crate::ipc::endpoint::recv(reply_ep, pid) {
            Ok(resp) => {
                let bytes_recv = resp.payload[0] as usize
                    | ((resp.payload[1] as usize) << 8);
                // Copy received data to the buffer page
                if bytes_recv > 0 && bytes_recv <= pool.buf_size {
                    let kva = crate::mem::sv39::pa_to_kva(page_addr);
                    let buf_data = &resp.payload[2..];
                    let copy_len = bytes_recv.min(buf_data.len());
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            buf_data.as_ptr(),
                            kva as *mut u8,
                            copy_len,
                        );
                    }
                }
                pool.used_count -= 1;
                return bytes_recv as i32;
            }
            Err(_) => { crate::sched::schedule(); }
        }
    }
}

/// Splice data between two file descriptors (simplified).
/// `fd_in` — source fd (must be a file or pipe)
/// `fd_out` — destination fd (must be a file or pipe)
/// `len` — number of bytes to transfer
/// Returns bytes transferred on success.
pub fn splice(fd_in: u32, fd_out: u32, len: usize) -> i32 {
    let pid = unsafe {
        crate::sched::current_thread()
            .map(|t| (*t).owner)
            .unwrap_or(0)
    };

    // Read from fd_in using VFS IPC and write to fd_out
    let mut buf = [0u8; 64];
    let copy_len = len.min(64);

    // Find the path for fd_in
    let in_path = unsafe {
        let entry = crate::syscall::posix::find_fd_internal(pid, fd_in as usize);
        if entry.is_none() { return -1; }
        let e = &*entry.unwrap();
        core::slice::from_raw_parts(e.path.as_ptr(), e.path_len)
    };

    // VFS IPC read
    let reply_ep = crate::ipc::create_endpoint();
    let mut msg = crate::ipc::message::Message::new(pid, 2); // READ
    msg.payload[0] = reply_ep as u8;
    msg.payload[1] = (reply_ep >> 8) as u8;
    let plen = in_path.len().min(31);
    msg.payload[2] = plen as u8;
    for i in 0..plen { msg.payload[3 + i] = in_path[i]; }
    msg.payload_len = 3 + plen;

    if crate::ipc::endpoint::send(2, pid, msg).is_err() { return -1; }
    let resp = loop {
        match crate::ipc::endpoint::recv(reply_ep, pid) {
            Ok(r) => break r,
            Err(_) => { crate::sched::schedule(); }
        }
    };

    let available = copy_len.min(resp.payload_len);
    buf[..available].copy_from_slice(&resp.payload[..available]);

    // Find the path for fd_out
    let out_path = unsafe {
        let entry = crate::syscall::posix::find_fd_internal(pid, fd_out as usize);
        if entry.is_none() { return -1; }
        let e = &*entry.unwrap();
        core::slice::from_raw_parts(e.path.as_ptr(), e.path_len)
    };

    // VFS IPC write
    let reply_ep2 = crate::ipc::create_endpoint();
    let mut msg2 = crate::ipc::message::Message::new(pid, 3); // WRITE
    msg2.payload[0] = reply_ep2 as u8;
    msg2.payload[1] = (reply_ep2 >> 8) as u8;
    let plen2 = out_path.len().min(31);
    msg2.payload[2] = plen2 as u8;
    for i in 0..plen2 { msg2.payload[3 + i] = out_path[i]; }
    let data_off = 3 + plen2;
    msg2.payload[data_off] = available as u8;
    for i in 0..available { msg2.payload[data_off + 1 + i] = buf[i]; }
    msg2.payload_len = data_off + 1 + available;

    if crate::ipc::endpoint::send(2, pid, msg2).is_err() { return -1; }
    let _ = loop {
        match crate::ipc::endpoint::recv(reply_ep2, pid) {
            Ok(r) => break r,
            Err(_) => { crate::sched::schedule(); }
        }
    };

    available as i32
}

/// Shared memory layout for io_uring rings.
/// The SQ page and CQ page are mapped into user space at fixed offsets.
const SQ_PAGE_VA: usize = 0x0000_003F_FFFF_C000; // fixed user VA for SQ ring
const CQ_PAGE_VA: usize = 0x0000_003F_FFFF_B000; // fixed user VA for CQ ring

/// Create a new io_uring instance. Returns ring_id (fd).
pub fn setup(pid: u32, entries: usize) -> Option<usize> {
    unsafe {
        if RING_COUNT >= MAX_RINGS { return None; }
        let id = RING_COUNT;

        // Look up the process's page table root so we can map shared pages
        let pt_root = {
            let procs = crate::proc::PROCESSES.lock();
            let proc = procs.iter().find(|p| p.pid == pid)?;
            proc.page_table_root
        };

        // Allocate physical pages for SQ and CQ rings
        let sq_phys = crate::mem::buddy::alloc_page()?;
        let cq_phys = crate::mem::buddy::alloc_page()?;

        // Zero the pages
        core::ptr::write_bytes(crate::mem::sv39::pa_to_kva(sq_phys) as *mut u8, 0, 4096);
        core::ptr::write_bytes(crate::mem::sv39::pa_to_kva(cq_phys) as *mut u8, 0, 4096);

        // Map SQ and CQ pages into the process's user address space
        // SQ: read/write, no execute, user-accessible
        crate::proc::elf::map_into_pt(pt_root, SQ_PAGE_VA, sq_phys, true, true, false, true);
        crate::proc::elf::map_into_pt(pt_root, CQ_PAGE_VA, cq_phys, true, true, false, true);

        // Store ring metadata at the start of the SQ shared page
        // Layout: [entries:4][sq_head:4][sq_tail:4][cq_head:4][cq_tail:4]
        let sq_kva = crate::mem::sv39::pa_to_kva(sq_phys);
        let sq_data = sq_kva as *mut u32;
        sq_data.write(entries.min(RING_ENTRIES) as u32); // ring entries

        RINGS[id].pid = pid;
        RINGS[id].sq_entries = entries.min(RING_ENTRIES);
        RINGS[id].cq_entries = entries.min(RING_ENTRIES);
        RINGS[id].active = true;
        RINGS[id].sq_phys = sq_phys;
        RINGS[id].cq_phys = cq_phys;
        RINGS[id].sq_va = SQ_PAGE_VA;
        RINGS[id].cq_va = CQ_PAGE_VA;
        RING_COUNT += 1;
        Some(id)
    }
}

/// Get writeable SQE at tail index. Returns pointer to SQE or null.
pub fn get_sqe(ring_id: usize) -> *mut IoUringSqe {
    unsafe {
        if ring_id >= RING_COUNT { return core::ptr::null_mut(); }
        let ring = &mut RINGS[ring_id];
        if !ring.active { return core::ptr::null_mut(); }
        if ring.sq_tail - ring.sq_head >= ring.sq_entries { return core::ptr::null_mut(); }
        let idx = ring.sq_tail % ring.sq_entries;
        ring.sq_tail += 1;
        &mut ring.sqes[idx] as *mut IoUringSqe
    }
}

/// Submit all queued SQEs. Returns number submitted.
pub fn submit(ring_id: usize) -> usize {
    unsafe {
        if ring_id >= RING_COUNT { return 0; }
        let ring = &mut RINGS[ring_id];
        if !ring.active { return 0; }

        let pid = ring.pid;
        let mut completed: usize = 0;
        while ring.sq_head < ring.sq_tail {
            let sqe = &ring.sqes[ring.sq_head % ring.sq_entries];
            let result = execute_sqe(sqe, pid);

            // Post to CQ
            let cq_idx = ring.cq_head % ring.cq_entries;
            ring.cqes[cq_idx] = IoUringCqe {
                user_data: sqe.user_data,
                res: result,
                flags: 0,
            };
            ring.cq_head += 1;
            ring.sq_head += 1;
            completed += 1;
        }
        completed
    }
}

/// Get the user-space virtual address of the SQ ring for a given ring id.
pub fn get_sq_va(ring_id: usize) -> usize {
    unsafe {
        if ring_id >= RING_COUNT { return 0; }
        RINGS[ring_id].sq_va
    }
}

/// Peek at completion queue entries. Fills user buffer. Returns count.
pub fn peek_cqe(ring_id: usize, buf_ptr: usize, count: usize) -> usize {
    unsafe {
        if ring_id >= RING_COUNT || buf_ptr == 0 { return 0; }
        let ring = &RINGS[ring_id];
        if !ring.active { return 0; }

        let available = ring.cq_head.min(count);
        let buf = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, available * 16);
        for i in 0..available {
            let off = i * 16;
            let cqe = &ring.cqes[i];
            buf[off] = cqe.user_data as u8;
            buf[off+1] = (cqe.user_data>>8) as u8;
            buf[off+2] = (cqe.user_data>>16) as u8;
            buf[off+8] = cqe.res as u8;
            buf[off+9] = (cqe.res>>8) as u8;
            buf[off+10] = (cqe.res>>16) as u8;
            buf[off+11] = (cqe.res>>24) as u8;
        }
        available
    }
}

/// Execute a single SQE by dispatching to the real posix I/O implementation.
/// V35: Passes SQE flags as RWF flags to readv2/writev2 for extended I/O ops.
/// The pid parameter identifies the process that owns the ring.
fn execute_sqe(sqe: &IoUringSqe, _pid: u32) -> i32 {
    match sqe.opcode {
        IORING_OP_NOP => 0,
        IORING_OP_READ => {
            // V35: Use sys_readv2 with flags from SQE for RWF_* support
            let flags = sqe.flags as u32;
            // For RWF_NOWAIT and other flags, use the extended read
            if flags != 0 {
                match crate::syscall::ioflags::sys_readv2(
                    sqe.fd,
                    sqe.addr as *mut u8,
                    sqe.len as usize,
                    sqe.off as i64,
                    flags,
                ) {
                    Ok(n) => n as i32,
                    Err(_) => -1,
                }
            } else {
                // Fast path: no flags, use direct sys_read
                match crate::syscall::posix::sys_read(
                    sqe.fd as usize,
                    sqe.addr as usize,
                    sqe.len as usize,
                ) {
                    Ok(n) => n as i32,
                    Err(_) => -1,
                }
            }
        }
        IORING_OP_WRITE => {
            // V35: Use sys_writev2 with flags from SQE for RWF_* support
            let flags = sqe.flags as u32;
            if flags != 0 {
                match crate::syscall::ioflags::sys_writev2(
                    sqe.fd,
                    sqe.addr as *const u8,
                    sqe.len as usize,
                    sqe.off as i64,
                    flags,
                ) {
                    Ok(n) => n as i32,
                    Err(_) => -1,
                }
            } else {
                // Fast path: no flags, use direct sys_write
                match crate::syscall::posix::sys_write(
                    sqe.fd as usize,
                    sqe.addr as usize,
                    sqe.len as usize,
                ) {
                    Ok(n) => n as i32,
                    Err(_) => -1,
                }
            }
        }
        IORING_OP_OPEN => {
            // sys_open(path_ptr, flags, mode) — open a file
            // SQE: addr=path pointer, len=flags, fd=unused
            match crate::syscall::posix::sys_open(
                sqe.addr as usize,
                sqe.len as usize,
                0, // mode
            ) {
                Ok(fd) => fd as i32,
                Err(_) => -1,
            }
        }
        IORING_OP_CLOSE => {
            // sys_close(fd) — close file descriptor
            match crate::syscall::posix::sys_close(sqe.fd as usize) {
                Ok(_) => 0,
                Err(_) => -1,
            }
        }
        IORING_OP_STAT => {
            // sys_stat(fd, buf_ptr) — get file status
            match crate::syscall::posix::sys_stat(sqe.fd as usize, sqe.addr as usize) {
                Ok(size) => size as i32,
                Err(_) => -1,
            }
        }
        // V35: Zero-copy operations
        IORING_OP_SEND_ZC => {
            // addr = physical page address, fd = destination, len = data length, off = offset in page
            let page = sqe.addr as usize;
            let offset = sqe.off as usize;
            let len = sqe.len as usize;
            send_zc(sqe.fd, page, offset, len)
        }
        IORING_OP_RECV_ZC => {
            // fd = source fd, addr = buf_group ID (lower 16 bits)
            let buf_group = sqe.addr as u16;
            recv_zc(sqe.fd, buf_group)
        }
        IORING_OP_SPLICE => {
            // fd_in = SQE.fd, fd_out = addr (lower bits), len = count
            let fd_in = sqe.fd;
            let fd_out = sqe.addr as u32;
            let len = sqe.len as usize;
            splice(fd_in, fd_out, len)
        }
        IORING_OP_PROVIDE_BUFFERS => {
            // addr = pages array phys, len = buf_size, off = buf_group, fd = ring_id
            // This is typically done via syscall, not through the SQE ring
            // But handle it here for completeness
            -1
        }
        IORING_OP_REMOVE_BUFFERS => {
            // fd = buf_group_id (from ring perspective)
            -1
        }
        _ => -1,
    }
}
