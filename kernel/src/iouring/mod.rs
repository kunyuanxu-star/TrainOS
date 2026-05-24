// V22: io_uring-style async I/O subsystem
//
// Architecture:
//   - Per-process submission queue (SQ) and completion queue (CQ)
//   - Shared memory ring buffers mapped into user space
//   - Operations: READ, WRITE, OPEN, CLOSE, STAT, NOP
//   - Zero-copy via shared memory page transfer

use crate::mem::layout::PAGE_SIZE;

const MAX_RINGS: usize = 16;
const RING_ENTRIES: usize = 32;

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

// Opcodes
pub const IORING_OP_NOP: u8 = 0;
pub const IORING_OP_READ: u8 = 1;
pub const IORING_OP_WRITE: u8 = 2;
pub const IORING_OP_OPEN: u8 = 3;
pub const IORING_OP_CLOSE: u8 = 4;
pub const IORING_OP_STAT: u8 = 5;

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
/// The pid parameter identifies the process that owns the ring.
/// All posix functions use current_pid() internally, but we pass pid
/// for consistency and future cross-process I/O support.
fn execute_sqe(sqe: &IoUringSqe, _pid: u32) -> i32 {
    match sqe.opcode {
        IORING_OP_NOP => 0,
        IORING_OP_READ => {
            // sys_read(fd, buf_ptr, count) — read from fd into user buffer at addr
            match crate::syscall::posix::sys_read(
                sqe.fd as usize,
                sqe.addr as usize,
                sqe.len as usize,
            ) {
                Ok(n) => n as i32,
                Err(_) => -1,
            }
        }
        IORING_OP_WRITE => {
            // sys_write(fd, buf_ptr, count) — write from user buffer at addr
            match crate::syscall::posix::sys_write(
                sqe.fd as usize,
                sqe.addr as usize,
                sqe.len as usize,
            ) {
                Ok(n) => n as i32,
                Err(_) => -1,
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
        _ => -1,
    }
}
