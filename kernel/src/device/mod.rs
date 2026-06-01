// Device Driver Framework — standardized driver ABI for user-space drivers
//
// Drivers run as user-space services. The kernel provides:
//   1. Driver registration (name, type, probe function endpoint)
//   2. Device discovery notification
//   3. Standardized MMIO access (map, read32, write32)
//   4. Interrupt delivery (future)
//
// Driver types:
//   DRV_BLOCK   = 1  — block device driver
//   DRV_NET     = 2  — network device driver
//   DRV_CHAR    = 3  — character device driver
//   DRV_PCI     = 4  — PCI device driver
//   DRV_DISPLAY = 5  — display driver

pub mod merge;
pub mod sched;
pub mod iommu;
pub mod framebuffer;
pub mod graphics;
pub mod window;
pub mod input;
pub mod widgets;
pub mod gui;

pub const DRV_BLOCK: u32 = 1;
pub const DRV_NET: u32 = 2;
pub const DRV_CHAR: u32 = 3;
pub const DRV_PCI: u32 = 4;
pub const DRV_DISPLAY: u32 = 5;
pub const DRV_IOMMU: u32 = 6;

use crate::mem::cache_ops;

// ── V36c: DMA buffer cache management ─────────────────────────────────────
//
// Before initiating a DMA transfer, the kernel must ensure cache coherency
// between the CPU and the device:
//
//   - DMA READ  (device reads from memory): CPU data in cache must be
//     written back via `cache_clean_range` before the device reads.
//
//   - DMA WRITE (device writes to memory): CPU cache lines for the buffer
//     must be invalidated via `cache_flush_range` (write-back + invalidate)
//     before the device writes, so the CPU does not read stale data after
//     the transfer completes.

/// Prepare a memory buffer for a DMA **read** (device → memory).
///
/// Invalidates cache lines covering `[buf, buf+len)` so that the CPU
/// sees fresh data written by the device.  The caller must guarantee
/// no dirty data is present in the range.
pub fn dma_prepare_read(buf: usize, len: usize) {
    cache_ops::cache_flush_range(buf, len);
}

/// Prepare a memory buffer for a DMA **write** (memory → device).
///
/// Writes back (cleans) cache lines covering `[buf, buf+len)` so that
/// the device sees the latest data written by the CPU.
pub fn dma_prepare_write(buf: usize, len: usize) {
    cache_ops::cache_clean_range(buf, len);
}

// ── Driver framework ──────────────────────────────────────────────────────

const MAX_DRIVERS: usize = 16;

struct Driver {
    name: [u8; 32],
    name_len: usize,
    drv_type: u32,
    pid: u32,          // process that registered this driver
    ep: usize,         // endpoint for probing
    registered: bool,
}

static mut DRIVERS: [Driver; MAX_DRIVERS] = [
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
    Driver { name: [0; 32], name_len: 0, drv_type: 0, pid: 0, ep: 0, registered: false },
];

/// Register a driver. Returns driver ID.
pub fn register(name: &[u8], drv_type: u32, pid: u32, ep: usize) -> Option<usize> {
    unsafe {
        for i in 0..MAX_DRIVERS {
            if !DRIVERS[i].registered {
                DRIVERS[i].registered = true;
                DRIVERS[i].drv_type = drv_type;
                DRIVERS[i].pid = pid;
                DRIVERS[i].ep = ep;
                let nlen = name.len().min(31);
                DRIVERS[i].name_len = nlen;
                for j in 0..nlen { DRIVERS[i].name[j] = name[j]; }
                return Some(i);
            }
        }
    }
    None
}

/// Unregister a driver.
pub fn unregister(drv_id: usize) -> bool {
    unsafe {
        if drv_id < MAX_DRIVERS && DRIVERS[drv_id].registered {
            DRIVERS[drv_id].registered = false;
            return true;
        }
    }
    false
}

/// List drivers. Fills buf with driver info.
/// Format per driver: [type:4][pid:4][name_len:1][name:name_len]
pub fn list(buf: &mut [u8]) -> usize {
    let mut pos: usize = 0;
    unsafe {
        for i in 0..MAX_DRIVERS {
            if !DRIVERS[i].registered { continue; }
            let need = 9 + DRIVERS[i].name_len;
            if pos + need > buf.len() { break; }

            // type (4 bytes)
            let t = DRIVERS[i].drv_type;
            buf[pos] = t as u8; buf[pos+1] = (t>>8) as u8; buf[pos+2] = (t>>16) as u8; buf[pos+3] = (t>>24) as u8;

            // pid (4 bytes)
            let p = DRIVERS[i].pid;
            buf[pos+4] = p as u8; buf[pos+5] = (p>>8) as u8; buf[pos+6] = (p>>16) as u8; buf[pos+7] = (p>>24) as u8;

            // name_len
            buf[pos+8] = DRIVERS[i].name_len as u8;

            // name
            for j in 0..DRIVERS[i].name_len {
                buf[pos+9+j] = DRIVERS[i].name[j];
            }

            pos += need;
        }
    }
    pos
}

/// Probe a device with a registered driver.
pub fn probe_pci(vendor: u16, device: u16) -> Option<(usize, u32)> {
    // For now, return the first registered PCI driver
    unsafe {
        for i in 0..MAX_DRIVERS {
            if DRIVERS[i].registered && DRIVERS[i].drv_type == DRV_PCI {
                return Some((DRIVERS[i].ep, DRIVERS[i].pid));
            }
        }
    }
    None
}

// ── V22.5 Multi-Queue Block Layer (blk-mq) ──────────────────────────────────

/// Maximum entries per per-CPU block queue.
pub const BLK_MQ_ENTRIES: usize = 32;

/// Maximum number of per-CPU queues (one per hardware thread).
pub const BLK_MQ_MAX_QUEUES: usize = 8;

/// A single entry in a per-CPU block queue.
#[derive(Copy, Clone)]
pub struct BlkMqEntry {
    pub sector: u64,
    pub count: u32,
    pub buf: u64,
    pub write: bool,
    pub used: bool,
}

impl BlkMqEntry {
    pub const fn empty() -> Self {
        BlkMqEntry { sector: 0, count: 0, buf: 0, write: false, used: false }
    }
}

/// A per-CPU queue of pending block I/O requests.
pub struct BlkMqQueue {
    pub entries: [BlkMqEntry; BLK_MQ_ENTRIES],
    pub head: usize,
    pub tail: usize,
}

impl BlkMqQueue {
    pub const fn empty() -> Self {
        BlkMqQueue {
            entries: [BlkMqEntry::empty(); BLK_MQ_ENTRIES],
            head: 0,
            tail: 0,
        }
    }
}

/// Global array of per-CPU block queues.
pub static mut BLK_QUEUES: [BlkMqQueue; BLK_MQ_MAX_QUEUES] = [
    BlkMqQueue::empty(),
    BlkMqQueue::empty(),
    BlkMqQueue::empty(),
    BlkMqQueue::empty(),
    BlkMqQueue::empty(),
    BlkMqQueue::empty(),
    BlkMqQueue::empty(),
    BlkMqQueue::empty(),
];

/// Submit a block I/O request to the specified per-CPU queue.
///
/// Returns the queue index on success, or `None` if the queue is full
/// or `cpu` is out of range.
pub fn blk_submit(cpu: usize, sector: u64, count: u32, buf: u64, write: bool) -> Option<usize> {
    unsafe {
        if cpu >= BLK_MQ_MAX_QUEUES {
            return None;
        }
        let queue = &mut BLK_QUEUES[cpu];
        if queue.tail - queue.head >= BLK_MQ_ENTRIES {
            return None; // queue is full
        }
        let idx = queue.tail % BLK_MQ_ENTRIES;
        queue.entries[idx] = BlkMqEntry { sector, count, buf, write, used: true };
        queue.tail += 1;
        Some(cpu)
    }
}

/// Drain (dequeue) all pending requests from the specified per-CPU queue.
///
/// Calls the provided closure for each drained entry.
/// Returns the number of requests processed.
pub fn blk_drain<F: FnMut(&BlkMqEntry)>(cpu: usize, mut callback: F) -> usize {
    unsafe {
        if cpu >= BLK_MQ_MAX_QUEUES {
            return 0;
        }
        let queue = &mut BLK_QUEUES[cpu];
        let mut drained = 0;
        while queue.head < queue.tail {
            let idx = queue.head % BLK_MQ_ENTRIES;
            if queue.entries[idx].used {
                callback(&queue.entries[idx]);
                queue.entries[idx].used = false;
                drained += 1;
            }
            queue.head += 1;
        }
        queue.head = 0;
        queue.tail = 0;
        drained
    }
}
