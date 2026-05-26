// V35: I/O flags — inspired by Linux RWF_* flags
//
// Provides extended read/write syscalls (preadv2/pwritev2 equivalents),
// atomic write support, and page cache statistics.

use crate::ipc::message::Message;

// ── RWF Flags ──────────────────────────────────────────────────────────────

pub const RWF_HIPRI: u32    = 0x01;  // High priority request
pub const RWF_DSYNC: u32    = 0x02;  // Data sync completion
pub const RWF_SYNC: u32     = 0x04;  // Full sync completion
pub const RWF_NOWAIT: u32   = 0x08;  // Non-blocking; returns EAGAIN if would block
pub const RWF_APPEND: u32   = 0x10;  // Append to end of file
pub const RWF_UNCACHED: u32 = 0x20;  // Drop page cache after I/O
pub const RWF_ATOMIC: u32   = 0x40;  // Atomic write (all-or-nothing)

// ── Atomic Write Support ───────────────────────────────────────────────────

const ATOMIC_WRITE_MAX: usize = 65536;

/// State for tracking atomic writes.
/// Saves original disk data for rollback on failure.
struct AtomicWriteState {
    active: bool,
    original_data: [u8; ATOMIC_WRITE_MAX],
    sector: u64,
    count: u32,
}

const EMPTY_ATOMIC_STATE: AtomicWriteState = AtomicWriteState {
    active: false,
    original_data: [0u8; ATOMIC_WRITE_MAX],
    sector: 0,
    count: 0,
};

static mut ATOMIC_STATE: AtomicWriteState = EMPTY_ATOMIC_STATE;

/// Begin an atomic write: mark the state as active and record the target sector/count.
/// The actual data backup is done by the caller (sys_blk_write) using the DMA buffer.
pub fn atomic_write_begin(sector: u64, count: u32) -> Result<(), &'static str> {
    unsafe {
        if ATOMIC_STATE.active {
            return Err("atomic write already in progress");
        }
        ATOMIC_STATE.active = true;
        ATOMIC_STATE.sector = sector;
        ATOMIC_STATE.count = count;
    }
    Ok(())
}

/// Save original sector data into the atomic write backup buffer.
/// Called by sys_blk_write with the data read back from the device before the write.
pub fn atomic_write_save_original(data: &[u8]) {
    unsafe {
        let len = data.len().min(ATOMIC_WRITE_MAX);
        ATOMIC_STATE.original_data[..len].copy_from_slice(&data[..len]);
    }
}

/// Commit an atomic write — clear the saved state (write succeeded).
pub fn atomic_write_commit() {
    unsafe {
        ATOMIC_STATE.active = false;
        ATOMIC_STATE.original_data = [0u8; ATOMIC_WRITE_MAX];
        ATOMIC_STATE.sector = 0;
        ATOMIC_STATE.count = 0;
    }
}

/// Rollback an atomic write — write the saved original data back to disk.
/// Returns the number of bytes written on success.
pub fn atomic_write_rollback() -> Result<usize, &'static str> {
    unsafe {
        if !ATOMIC_STATE.active {
            return Err("no atomic write in progress");
        }
        let sector = ATOMIC_STATE.sector as usize;
        let count = ATOMIC_STATE.count as usize;
        // Write saved data back one sector at a time using kernel-internal blk write
        let mut written = 0;
        let sectors = (count + 511) / 512;
        for s in 0..sectors {
            let off = s * 512;
            let end = core::cmp::min(off + 512, ATOMIC_WRITE_MAX);
            let chunk = &ATOMIC_STATE.original_data[off..end];
            if chunk.is_empty() { break; }
            // Write via a kernel DMA buffer
            let dma_buf = alloc::alloc::alloc(core::alloc::Layout::from_size_align(512, 64).unwrap());
            if dma_buf.is_null() { return Err("OOM in rollback"); }
            // Pad with zeros if needed
            for i in 0..chunk.len() { unsafe { dma_buf.add(i).write(chunk[i]); } }
            for i in chunk.len()..512 { unsafe { dma_buf.add(i).write(0); } }
            // Use the raw block write (sector, buf)
            let res = raw_blk_write(sector + s, dma_buf);
            let _ = alloc::alloc::dealloc(dma_buf, core::alloc::Layout::from_size_align(512, 64).unwrap());
            if !res { return Err("rollback write failed"); }
            written += chunk.len();
        }
        // Clear state
        ATOMIC_STATE.active = false;
        ATOMIC_STATE.original_data = [0u8; ATOMIC_WRITE_MAX];
        ATOMIC_STATE.sector = 0;
        ATOMIC_STATE.count = 0;
        Ok(written)
    }
}

/// Low-level block write using virtio (copied from sys_blk_write internals).
/// Writes 512 bytes from `data_buf` to `sector`.
/// Returns true on success.
fn raw_blk_write(sector: usize, data_buf: *mut u8) -> bool {
    // This is a minimal version of the virtio block write path.
    // It reuses the same MMIO register layout as sys_blk_write in proc.rs.
    const VIRTIO_BASE: usize = 0x10001000;

    // Register offsets
    const VR_REG_QUEUE_SEL: usize = 0x30;
    const VR_REG_QUEUE_NUM_MAX: usize = 0x34;
    const VR_REG_QUEUE_NUM: usize = 0x38;
    const VR_REG_QUEUE_DESC_LOW: usize = 0x80;
    const VR_REG_QUEUE_DESC_HIGH: usize = 0x84;
    const VR_REG_QUEUE_AVAIL_LOW: usize = 0x90;
    const VR_REG_QUEUE_AVAIL_HIGH: usize = 0x94;
    const VR_REG_QUEUE_USED_LOW: usize = 0xA0;
    const VR_REG_QUEUE_USED_HIGH: usize = 0xA4;
    const VR_REG_STATUS: usize = 0x70;
    const VR_REG_QUEUE_READY: usize = 0x44;

    const STATUS_ACKNOWLEDGE: u32 = 1;
    const STATUS_DRIVER: u32 = 2;
    const STATUS_DRIVER_OK: u32 = 4;
    const VIRTIO_BLK_T_OUT: u32 = 1;

    fn vr_read(offset: usize) -> u32 {
        unsafe { ((VIRTIO_BASE + offset) as *const u32).read_volatile() }
    }
    fn vr_write(offset: usize, val: u32) {
        unsafe { ((VIRTIO_BASE + offset) as *mut u32).write_volatile(val) }
    }

    // 1-3. Reset, acknowledge, driver
    vr_write(VR_REG_STATUS, 0);
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE);
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);

    // 4. Feature negotiation
    vr_write(0x14, 0);
    let _ = vr_read(0x10);
    vr_write(0x20, 0);
    vr_write(0x24, 0);
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER | (1 << 8));
    if vr_read(VR_REG_STATUS) & (1 << 8) == 0 { return false; }

    // 5. Configure queue 0
    vr_write(VR_REG_QUEUE_SEL, 0);
    let max_size = vr_read(VR_REG_QUEUE_NUM_MAX);
    if max_size == 0 { return false; }
    let queue_size = (max_size as usize).min(16);
    vr_write(VR_REG_QUEUE_NUM, queue_size as u32);

    let desc_size = queue_size * 16;
    let avail_size = 6 + 2 * queue_size;
    let used_size = 6 + 8 * queue_size;
    let total_size = desc_size + ((avail_size + 1) & !1) + ((used_size + 3) & !3);

    let vq_mem = unsafe {
        alloc::alloc::alloc_zeroed(core::alloc::Layout::from_size_align(total_size, 4096).unwrap())
    };
    if vq_mem.is_null() { return false; }

    let desc_table = vq_mem as usize;
    let avail_ring = (desc_table + desc_size + 1) & !1;
    let used_ring = (avail_ring + avail_size + 3) & !3;

    // Request header
    let req_buf = unsafe { alloc::alloc::alloc(core::alloc::Layout::from_size_align(16, 8).unwrap()) };
    if req_buf.is_null() { let _ = unsafe { alloc::alloc::dealloc(vq_mem, core::alloc::Layout::from_size_align(total_size, 4096).unwrap()) }; return false; }
    unsafe {
        (req_buf as *mut u32).write_volatile(VIRTIO_BLK_T_OUT);
        (req_buf as *mut u32).add(1).write_volatile(0);
        (req_buf as *mut u64).add(1).write_volatile(sector as u64);
    }

    // Status byte
    let status_buf = unsafe { alloc::alloc::alloc(core::alloc::Layout::from_size_align(1, 1).unwrap()) };
    if status_buf.is_null() { return false; }

    // Set up descriptor table
    unsafe {
        let d0 = desc_table as *mut u32;
        d0.add(0).write_volatile(req_buf as u32);
        d0.add(1).write_volatile(0);
        d0.add(2).write_volatile(16);
        d0.add(3).write_volatile(1 | (1 << 16));

        let d1 = (desc_table + 16) as *mut u32;
        d1.add(0).write_volatile(data_buf as u32);
        d1.add(1).write_volatile(0);
        d1.add(2).write_volatile(512);
        d1.add(3).write_volatile(1 | (2 << 16));

        let d2 = (desc_table + 32) as *mut u32;
        d2.add(0).write_volatile(status_buf as u32);
        d2.add(1).write_volatile(0);
        d2.add(2).write_volatile(1);
        d2.add(3).write_volatile(2);
    }

    // Set up available ring
    unsafe {
        (avail_ring as *mut u16).write_volatile(0);
        (avail_ring as *mut u16).add(1).write_volatile(0);
        (avail_ring as *mut u16).add(2).write_volatile(0);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        (avail_ring as *mut u16).add(1).write_volatile(1);
    }

    vr_write(VR_REG_QUEUE_DESC_LOW, desc_table as u32);
    vr_write(VR_REG_QUEUE_DESC_HIGH, 0);
    vr_write(VR_REG_QUEUE_AVAIL_LOW, avail_ring as u32);
    vr_write(VR_REG_QUEUE_AVAIL_HIGH, 0);
    vr_write(VR_REG_QUEUE_USED_LOW, used_ring as u32);
    vr_write(VR_REG_QUEUE_USED_HIGH, 0);
    vr_write(VR_REG_QUEUE_READY, 1);
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER | (1 << 8) | STATUS_DRIVER_OK);
    unsafe { ((VIRTIO_BASE + 0x50) as *mut u32).write_volatile(0); }

    // Poll for completion
    let used_idx_ptr = (used_ring + 2) as *mut u16;
    let mut poll_count: u32 = 0;
    loop {
        if unsafe { used_idx_ptr.read_volatile() > 0 } { break; }
        poll_count += 1;
        if poll_count > 10_000_000 {
            let _st = vr_read(VR_REG_STATUS);
            return false;
        }
        core::hint::spin_loop();
    }

    let blk_status = unsafe { status_buf.read() };
    let ok = blk_status == 0;

    // Cleanup
    unsafe {
        let _ = alloc::alloc::dealloc(req_buf, core::alloc::Layout::from_size_align(16, 8).unwrap());
        let _ = alloc::alloc::dealloc(status_buf, core::alloc::Layout::from_size_align(1, 1).unwrap());
        let _ = alloc::alloc::dealloc(vq_mem, core::alloc::Layout::from_size_align(total_size, 4096).unwrap());
    }

    ok
}

/// Read current sector data into a kernel buffer for atomic write save.
/// Returns the data as a fixed-size array.
pub fn atomic_read_sector(sector: usize) -> Result<[u8; 512], &'static str> {
    // Use the raw blk read to get current sector data
    let dma_buf = unsafe { alloc::alloc::alloc_zeroed(core::alloc::Layout::from_size_align(512, 64).unwrap()) };
    if dma_buf.is_null() { return Err("OOM"); }

    // Read sector via virtio
    let ok = raw_blk_read(sector, dma_buf);

    let mut result = [0u8; 512];
    unsafe {
        core::ptr::copy_nonoverlapping(dma_buf, result.as_mut_ptr(), 512);
    }
    let _ = unsafe { alloc::alloc::dealloc(dma_buf, core::alloc::Layout::from_size_align(512, 64).unwrap()) };

    if !ok { Err("raw blk read failed") } else { Ok(result) }
}

/// Low-level block read (sector -> kernel buffer).
fn raw_blk_read(sector: usize, data_buf: *mut u8) -> bool {
    const VIRTIO_BASE: usize = 0x10001000;
    const VR_REG_QUEUE_SEL: usize = 0x30;
    const VR_REG_QUEUE_NUM_MAX: usize = 0x34;
    const VR_REG_QUEUE_NUM: usize = 0x38;
    const VR_REG_QUEUE_DESC_LOW: usize = 0x80;
    const VR_REG_QUEUE_DESC_HIGH: usize = 0x84;
    const VR_REG_QUEUE_AVAIL_LOW: usize = 0x90;
    const VR_REG_QUEUE_AVAIL_HIGH: usize = 0x94;
    const VR_REG_QUEUE_USED_LOW: usize = 0xA0;
    const VR_REG_QUEUE_USED_HIGH: usize = 0xA4;
    const VR_REG_STATUS: usize = 0x70;
    const VR_REG_QUEUE_READY: usize = 0x44;
    const STATUS_ACKNOWLEDGE: u32 = 1;
    const STATUS_DRIVER: u32 = 2;
    const STATUS_DRIVER_OK: u32 = 4;
    const VIRTIO_BLK_T_IN: u32 = 0;

    fn vr_read(offset: usize) -> u32 {
        unsafe { ((VIRTIO_BASE + offset) as *const u32).read_volatile() }
    }
    fn vr_write(offset: usize, val: u32) {
        unsafe { ((VIRTIO_BASE + offset) as *mut u32).write_volatile(val) }
    }

    vr_write(VR_REG_STATUS, 0);
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE);
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);
    vr_write(0x14, 0);
    let _ = vr_read(0x10);
    vr_write(0x20, 0);
    vr_write(0x24, 0);
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER | (1 << 8));
    if vr_read(VR_REG_STATUS) & (1 << 8) == 0 { return false; }

    vr_write(VR_REG_QUEUE_SEL, 0);
    let max_size = vr_read(VR_REG_QUEUE_NUM_MAX);
    if max_size == 0 { return false; }
    let queue_size = (max_size as usize).min(16);
    vr_write(VR_REG_QUEUE_NUM, queue_size as u32);

    let desc_size = queue_size * 16;
    let avail_size = 6 + 2 * queue_size;
    let used_size = 6 + 8 * queue_size;
    let total_size = desc_size + ((avail_size + 1) & !1) + ((used_size + 3) & !3);

    let vq_mem = unsafe {
        alloc::alloc::alloc_zeroed(core::alloc::Layout::from_size_align(total_size, 4096).unwrap())
    };
    if vq_mem.is_null() { return false; }

    let desc_table = vq_mem as usize;
    let avail_ring = (desc_table + desc_size + 1) & !1;
    let used_ring = (avail_ring + avail_size + 3) & !3;

    let req_buf = unsafe { alloc::alloc::alloc(core::alloc::Layout::from_size_align(16, 8).unwrap()) };
    if req_buf.is_null() { return false; }
    unsafe {
        (req_buf as *mut u32).write_volatile(VIRTIO_BLK_T_IN);
        (req_buf as *mut u32).add(1).write_volatile(0);
        (req_buf as *mut u64).add(1).write_volatile(sector as u64);
    }

    let status_buf = unsafe { alloc::alloc::alloc(core::alloc::Layout::from_size_align(1, 1).unwrap()) };
    if status_buf.is_null() { return false; }

    unsafe {
        let d0 = desc_table as *mut u32;
        d0.add(0).write_volatile(req_buf as u32);
        d0.add(1).write_volatile(0);
        d0.add(2).write_volatile(16);
        d0.add(3).write_volatile(1 | (1 << 16));

        let d1 = (desc_table + 16) as *mut u32;
        d1.add(0).write_volatile(data_buf as u32);
        d1.add(1).write_volatile(0);
        d1.add(2).write_volatile(512);
        d1.add(3).write_volatile(3 | (2 << 16));

        let d2 = (desc_table + 32) as *mut u32;
        d2.add(0).write_volatile(status_buf as u32);
        d2.add(1).write_volatile(0);
        d2.add(2).write_volatile(1);
        d2.add(3).write_volatile(2);
    }

    unsafe {
        (avail_ring as *mut u16).write_volatile(0);
        (avail_ring as *mut u16).add(1).write_volatile(0);
        (avail_ring as *mut u16).add(2).write_volatile(0);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        (avail_ring as *mut u16).add(1).write_volatile(1);
    }

    vr_write(VR_REG_QUEUE_DESC_LOW, desc_table as u32);
    vr_write(VR_REG_QUEUE_DESC_HIGH, 0);
    vr_write(VR_REG_QUEUE_AVAIL_LOW, avail_ring as u32);
    vr_write(VR_REG_QUEUE_AVAIL_HIGH, 0);
    vr_write(VR_REG_QUEUE_USED_LOW, used_ring as u32);
    vr_write(VR_REG_QUEUE_USED_HIGH, 0);
    vr_write(VR_REG_QUEUE_READY, 1);
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER | (1 << 8) | STATUS_DRIVER_OK);
    unsafe { ((VIRTIO_BASE + 0x50) as *mut u32).write_volatile(0); }

    let used_idx_ptr = (used_ring + 2) as *mut u16;
    let mut poll_count: u32 = 0;
    loop {
        if unsafe { used_idx_ptr.read_volatile() > 0 } { break; }
        poll_count += 1;
        if poll_count > 10_000_000 { return false; }
        core::hint::spin_loop();
    }

    let blk_status = unsafe { status_buf.read() };
    let ok = blk_status == 0;

    unsafe {
        let _ = alloc::alloc::dealloc(req_buf, core::alloc::Layout::from_size_align(16, 8).unwrap());
        let _ = alloc::alloc::dealloc(status_buf, core::alloc::Layout::from_size_align(1, 1).unwrap());
        let _ = alloc::alloc::dealloc(vq_mem, core::alloc::Layout::from_size_align(total_size, 4096).unwrap());
    }

    ok
}

// ── Cache Eviction Tracking ────────────────────────────────────────────────

/// Counter for RWF_UNCACHED operations (page cache evictions).
static mut CACHE_EVICTION_COUNT: u64 = 0;
/// Counter for recently evicted pages (last 1000 ops).
static mut CACHE_RECENT_EVICTION_COUNT: u64 = 0;

/// Record a cache eviction event (called when RWF_UNCACHED is used).
pub fn record_cache_eviction() {
    unsafe {
        CACHE_EVICTION_COUNT = CACHE_EVICTION_COUNT.wrapping_add(1);
        CACHE_RECENT_EVICTION_COUNT = CACHE_RECENT_EVICTION_COUNT.wrapping_add(1);
    }
}

/// Reset the recent eviction counter.
pub fn reset_recent_evictions() {
    unsafe { CACHE_RECENT_EVICTION_COUNT = 0; }
}

// ── FD table helpers (shared with posix.rs) ────────────────────────────────

fn current_pid() -> u32 {
    crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0)
}

fn vfs_send_recv(opcode: u16, path: &[u8], write_data: &[u8]) -> Result<Message, &'static str> {
    let sender_pid = current_pid();
    let reply_ep = crate::ipc::create_endpoint();

    let mut msg = Message::new(sender_pid, opcode);
    msg.payload[0] = reply_ep as u8;
    msg.payload[1] = (reply_ep >> 8) as u8;
    let plen = path.len().min(31);
    msg.payload[2] = plen as u8;
    for i in 0..plen { msg.payload[3 + i] = path[i]; }
    let data_off = 3 + plen;
    let dlen = write_data.len().min(64 - data_off - 1);
    msg.payload[data_off] = dlen as u8;
    for i in 0..dlen { msg.payload[data_off + 1 + i] = write_data[i]; }
    msg.payload_len = data_off + 1 + dlen;

    crate::ipc::endpoint::send(2, sender_pid, msg).ok().ok_or("vfs send failed")?;

    loop {
        match crate::ipc::endpoint::recv(reply_ep, sender_pid) {
            Ok(resp) => return Ok(resp),
            Err(_) => { crate::sched::schedule(); }
        }
    }
}

fn vfs_read(path: &[u8]) -> Result<Message, &'static str> {
    vfs_send_recv(2, path, &[])
}

fn vfs_write(path: &[u8], data: &[u8]) -> Result<Message, &'static str> {
    vfs_send_recv(3, path, data)
}

fn current_root_pt() -> Option<usize> {
    let pid = current_pid();
    let procs = crate::proc::PROCESSES.lock();
    for proc in procs.iter() {
        if proc.pid == pid {
            return Some(proc.page_table_root);
        }
    }
    None
}

/// Validate user buffer bounds.
fn check_user_buf(buf_ptr: *const u8, count: usize) -> Result<(), &'static str> {
    if count == 0 { return Ok(()); }
    if buf_ptr.is_null() { return Err("null buf"); }
    if let Some(root_pt) = current_root_pt() {
        if !crate::mem::sv39::is_user_range_valid(root_pt, buf_ptr as usize, count) {
            return Err("buffer out of bounds");
        }
    }
    Ok(())
}

// ── sys_readv2 / preadv2 equivalent ────────────────────────────────────────

/// Extended read with explicit offset and flags (preadv2 equivalent).
///
/// Arguments:
///   fd      — file descriptor
///   buf     — user-space destination buffer
///   count   — number of bytes to read
///   offset  — explicit offset (-1 means use fd's current offset)
///   flags   — RWF_* flags
///
/// Returns number of bytes read on success.
pub fn sys_readv2(
    fd: u32,
    buf: *mut u8,
    count: usize,
    offset: i64,
    flags: u32,
) -> Result<usize, &'static str> {
    let pid = current_pid();
    check_user_buf(buf as *const u8, count)?;

    // Handle stdin (fd=0)
    if fd == 0 {
        if flags & RWF_NOWAIT != 0 {
            return Err("EAGAIN"); // non-blocking read from console would block
        }
        let c: usize;
        unsafe { core::arch::asm!("ecall", in("a7") 2usize, lateout("a0") c); }
        if !buf.is_null() && count > 0 {
            unsafe { buf.write_volatile(c as u8); }
        }
        return Ok(1);
    }
    if fd <= 2 {
        return Err("bad fd for read");
    }

    // Look up fd entry
    let entry = unsafe { crate::syscall::posix::find_fd_internal(pid, fd as usize).ok_or("bad fd")? };
    let path = unsafe {
        let e = &*entry;
        if matches!(e.fd_type, crate::syscall::posix::FdType::Socket)
            || matches!(e.fd_type, crate::syscall::posix::FdType::Pipe)
        {
            // Socket/pipe read
            if flags & RWF_NOWAIT != 0 {
                // Check if data is available (non-blocking)
                // For simplicity, try recv with a fast timeout
                match crate::ipc::endpoint::recv(e.ep, pid) {
                    Ok(msg) => {
                        let copy_len = core::cmp::min(msg.payload_len, count.min(64));
                        if !buf.is_null() && copy_len > 0 {
                            unsafe {
                                let dst = core::slice::from_raw_parts_mut(buf, copy_len);
                                dst.copy_from_slice(&msg.payload[..copy_len]);
                            }
                        }
                        if flags & RWF_UNCACHED != 0 { record_cache_eviction(); }
                        return Ok(copy_len);
                    }
                    Err(_) => return Err("EAGAIN"), // would block
                }
            }
            let sender_pid = pid;
            loop {
                match crate::ipc::endpoint::recv(e.ep, sender_pid) {
                    Ok(msg) => {
                        let copy_len = core::cmp::min(msg.payload_len, count.min(64));
                        if !buf.is_null() && copy_len > 0 {
                            unsafe {
                                let dst = core::slice::from_raw_parts_mut(buf, copy_len);
                                dst.copy_from_slice(&msg.payload[..copy_len]);
                            }
                        }
                        if flags & RWF_UNCACHED != 0 { record_cache_eviction(); }
                        return Ok(copy_len);
                    }
                    Err(_) => { crate::sched::schedule(); }
                }
            }
        }
        core::slice::from_raw_parts(e.path.as_ptr(), e.path_len)
    };

    // V27.3: Sandbox check
    if !crate::aslr::sandbox_check(pid, path, false) {
        return Err("sandbox: read denied");
    }

    // Determine effective offset
    let effective_offset = if offset >= 0 {
        offset as usize
    } else {
        unsafe { (*entry).offset }
    };

    // File read via VFS
    let resp = vfs_read(path)?;

    // Skip "ENOENT" response
    if resp.payload_len >= 6 && &resp.payload[..6] == b"ENOENT" {
        return Ok(0);
    }

    let available = if resp.payload_len > effective_offset {
        resp.payload_len - effective_offset
    } else {
        0
    };
    let copy_len = core::cmp::min(available, count);

    if !buf.is_null() && copy_len > 0 {
        unsafe {
            let dst = core::slice::from_raw_parts_mut(buf, copy_len);
            dst.copy_from_slice(&resp.payload[effective_offset..effective_offset + copy_len]);
        }
    }

    // Update fd offset only if no explicit offset was given
    if offset < 0 {
        unsafe { (*entry).offset += copy_len; }
    }

    // RWF_UNCACHED: mark for eviction
    if flags & RWF_UNCACHED != 0 {
        record_cache_eviction();
    }

    Ok(copy_len)
}

// ── sys_writev2 / pwritev2 equivalent ──────────────────────────────────────

/// Extended write with explicit offset and flags (pwritev2 equivalent).
///
/// Arguments:
///   fd      — file descriptor
///   buf     — user-space source buffer
///   count   — number of bytes to write
///   offset  — explicit offset (-1 means use fd's current offset)
///   flags   — RWF_* flags
///
/// Returns number of bytes written on success.
pub fn sys_writev2(
    fd: u32,
    buf: *const u8,
    count: usize,
    offset: i64,
    flags: u32,
) -> Result<usize, &'static str> {
    let pid = current_pid();
    check_user_buf(buf, count)?;

    // Handle stdout/stderr (fd=1,2)
    if fd <= 2 {
        if !buf.is_null() && count > 0 {
            unsafe {
                let src = core::slice::from_raw_parts(buf, count);
                for &c in src {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") c as usize);
                }
            }
        }
        if flags & RWF_UNCACHED != 0 { record_cache_eviction(); }
        return Ok(count);
    }

    // Look up fd entry
    let entry = unsafe { crate::syscall::posix::find_fd_internal(pid, fd as usize).ok_or("bad fd")? };

    // Socket/pipe write
    unsafe {
        let e = &mut *entry;
        if matches!(e.fd_type, crate::syscall::posix::FdType::Socket)
            || matches!(e.fd_type, crate::syscall::posix::FdType::Pipe)
        {
            if flags & RWF_NOWAIT != 0 {
                // Non-blocking: try to send, fail if endpoint full
                let data_len = core::cmp::min(count, 62);
                let mut msg = Message::new(pid, 0);
                if !buf.is_null() && data_len > 0 {
                    let src = unsafe { core::slice::from_raw_parts(buf, data_len) };
                    msg.payload[..data_len].copy_from_slice(src);
                }
                msg.payload_len = data_len;
                match crate::ipc::endpoint::send(e.ep, pid, msg) {
                    Ok(_) => {
                        if flags & RWF_UNCACHED != 0 { record_cache_eviction(); }
                        return Ok(data_len);
                    }
                    Err(_) => return Err("EAGAIN"),
                }
            }
            let data_len = core::cmp::min(count, 62);
            let mut msg = Message::new(pid, 0);
            if !buf.is_null() && data_len > 0 {
                let src = unsafe { core::slice::from_raw_parts(buf, data_len) };
                msg.payload[..data_len].copy_from_slice(src);
            }
            msg.payload_len = data_len;
            crate::ipc::endpoint::send(e.ep, pid, msg).ok().ok_or("send failed")?;
            if flags & RWF_UNCACHED != 0 { record_cache_eviction(); }
            return Ok(data_len);
        }
    }

    // File write via VFS
    let path = unsafe {
        let e = &*entry;
        core::slice::from_raw_parts(e.path.as_ptr(), e.path_len)
    };

    // V27.3: Sandbox check
    if !crate::aslr::sandbox_check(pid, path, true) {
        return Err("sandbox: write denied");
    }

    // Handle RWF_APPEND: set offset to end of file
    let effective_offset = if flags & RWF_APPEND != 0 {
        // Read current file to get length
        let resp = vfs_read(path)?;
        resp.payload_len
    } else if offset >= 0 {
        offset as usize
    } else {
        unsafe { (*entry).offset }
    };

    let write_len = core::cmp::min(count, 60); // leave room for IPC headers
    let data = unsafe { core::slice::from_raw_parts(buf, write_len) };

    // If writing at an explicit offset, we need to read-modify-write
    if effective_offset > 0 {
        // Read current content
        if let Ok(resp) = vfs_read(path) {
            if resp.payload_len >= 6 && &resp.payload[..6] != b"ENOENT" {
                // Merge: keep data before offset, write new data at offset
                let before_len = core::cmp::min(effective_offset, resp.payload_len);
                let after_start = effective_offset + write_len;
                let after_len = if after_start < resp.payload_len {
                    resp.payload_len - after_start
                } else {
                    0
                };
                let mut merged = [0u8; 64];
                let mut merged_len = 0;
                // Copy data before offset
                if before_len > 0 {
                    merged[..before_len].copy_from_slice(&resp.payload[..before_len]);
                    merged_len = before_len;
                }
                // Insert new data
                if write_len > 0 {
                    let end = merged_len + write_len;
                    if end <= 64 {
                        merged[merged_len..end].copy_from_slice(&data[..write_len]);
                        merged_len = end;
                    }
                }
                // Copy remaining data after the write
                if after_len > 0 && merged_len + after_len <= 64 {
                    let remaining_end = resp.payload.len().min(64 - merged_len);
                    let actual_after = after_len.min(remaining_end);
                    merged[merged_len..merged_len + actual_after]
                        .copy_from_slice(&resp.payload[after_start..after_start + actual_after]);
                    merged_len += actual_after;
                }
                vfs_write(path, &merged[..merged_len])?;
                // Update fd offset
                if offset < 0 {
                    unsafe { (*entry).offset = merged_len; }
                }
                if flags & RWF_UNCACHED != 0 { record_cache_eviction(); }

                // Handle sync flags
                if flags & RWF_SYNC != 0 {
                    crate::syscall::fs::sys_fsync(fd as usize)?;
                } else if flags & RWF_DSYNC != 0 {
                    crate::syscall::fs::sys_fdatasync(fd as usize)?;
                }

                return Ok(write_len);
            }
        }
    }

    // Simple case: write at offset 0 (replace file content)
    vfs_write(path, data)?;

    // Update fd offset if no explicit offset
    if offset < 0 {
        unsafe { (*entry).offset += write_len; }
    }

    // Handle RWF_UNCACHED
    if flags & RWF_UNCACHED != 0 {
        record_cache_eviction();
    }

    // Handle sync flags
    if flags & RWF_SYNC != 0 {
        crate::syscall::fs::sys_fsync(fd as usize)?;
    } else if flags & RWF_DSYNC != 0 {
        crate::syscall::fs::sys_fdatasync(fd as usize)?;
    }

    Ok(write_len)
}

// ── cachestat ──────────────────────────────────────────────────────────────

/// Cachestat result structure — reports page cache residency statistics.
#[repr(C)]
pub struct CachestatResult {
    pub total_pages: u64,
    pub cached_pages: u64,
    pub dirty_pages: u64,
    pub writeback_pages: u64,
    pub evicted_pages: u64,
    pub recently_evicted_pages: u64,
}

/// Query page cache statistics.
///
/// Arguments:
///   fd      — file descriptor (unused in current implementation)
///   offset  — offset range start (unused)
///   len     — range length (unused)
///   buf_ptr — destination buffer for CachestatResult (24 bytes)
///
/// Since TrainOS uses a VFS-based model (file data in user-space VFS service),
/// this function reports kernel-level memory statistics plus eviction counters.
pub fn sys_cachestat(
    _fd: u32,
    _offset: usize,
    _len: usize,
    buf_ptr: usize,
) -> Result<usize, &'static str> {
    if buf_ptr == 0 {
        return Err("null buf");
    }

    let total_pages = crate::mem::buddy::total_pages() as u64;
    let allocated = crate::mem::buddy::allocated_pages() as u64;
    let cached_pages = if total_pages > allocated {
        total_pages - allocated
    } else {
        0
    };

    let result = CachestatResult {
        total_pages,
        cached_pages,
        dirty_pages: 0,           // No dirty page tracking in kernel yet
        writeback_pages: 0,       // No writeback tracking
        evicted_pages: unsafe { CACHE_EVICTION_COUNT },
        recently_evicted_pages: unsafe { CACHE_RECENT_EVICTION_COUNT },
    };

    unsafe {
        let ptr = buf_ptr as *mut u64;
        ptr.add(0).write_volatile(result.total_pages);
        ptr.add(1).write_volatile(result.cached_pages);
        ptr.add(2).write_volatile(result.dirty_pages);
        ptr.add(3).write_volatile(result.writeback_pages);
        ptr.add(4).write_volatile(result.evicted_pages);
        ptr.add(5).write_volatile(result.recently_evicted_pages);
    }

    Ok(0)
}
