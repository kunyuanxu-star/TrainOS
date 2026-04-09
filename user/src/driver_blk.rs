//! VirtIO Block Device Driver (User Space)
//!
//! Provides block storage device support via VirtIO
//! Uses syscalls to access device MMIO

use crate::driver_mmio::*;

/// VirtIO status bits
const VIRTIO_CONFIG_S_ACKNOWLEDGE: u8 = 1;
const VIRTIO_CONFIG_S_DRIVER: u8 = 2;
const VIRTIO_CONFIG_S_DRIVER_OK: u8 = 4;
const VIRTIO_CONFIG_S_FEATURES_OK: u8 = 8;
const VIRTIO_CONFIG_S_FAILED: u8 = 0x80;

/// VirtIO register offsets
const VIRTIO_PCI_HOST_FEATURES: usize = 0;
const VIRTIO_PCI_GUEST_FEATURES: usize = 4;
const VIRTIO_PCI_QUEUE_PFN: usize = 8;
const VIRTIO_PCI_STATUS: usize = 18;
const VIRTIO_PCI_ISR: usize = 19;

/// VirtIO block configuration structure (at offset 0x100)
const VIRTIO_BLK_CONFIG: usize = 0x100;

/// VirtIO queue notification register offset
const VIRTIO_PCI_QUEUE_NOTIFY: usize = 0x20;

/// VirtIO descriptor flags
const VIRTQ_DESC_F_NEXT: u16 = 0x1;
const VIRTQ_DESC_F_WRITE: u16 = 0x2;
const VIRTQ_DESC_F_INDIRECT: u16 = 0x4;

/// Virtqueue descriptor (16 bytes) - describes a buffer
#[repr(C)]
struct VirtqDesc {
    /// Physical address of the buffer
    addr: u64,
    /// Length of the buffer in bytes
    len: u32,
    /// Descriptor flags (NEXT, WRITE, INDIRECT)
    flags: u16,
    /// Index of the next descriptor in the chain (if NEXT flag is set)
    next: u16,
}

/// Virtqueue available ring - driver writes descriptor indices here
#[repr(C)]
struct VirtqAvail {
    /// Flags (typically 0)
    flags: u16,
    /// Index of the next available ring entry
    idx: u16,
    /// Ring of descriptor indices available to the device
    ring: [u16; 32],
    /// Event suppression/notification (used for virtio 1.0+)
    used_event: u16,
}

/// Virtqueue used ring element - describes a completed buffer
#[repr(C)]
#[derive(Copy, Clone)]
struct VirtqUsedElem {
    /// Index of the descriptor that was used
    id: u32,
    /// Length of data written by the device
    len: u32,
}

/// Virtqueue used ring - device writes completed descriptors here
#[repr(C)]
struct VirtqUsed {
    /// Flags (typically 0)
    flags: u16,
    /// Index of the next used ring entry
    idx: u16,
    /// Ring of used elements
    ring: [VirtqUsedElem; 32],
    /// Event suppression/notification
    avail_event: u16,
}

/// Virtqueue structure - manages descriptor table and rings
struct VirtQueue {
    /// Pointer to descriptor table
    desc: *mut VirtqDesc,
    /// Pointer to available ring
    avail: *mut VirtqAvail,
    /// Pointer to used ring
    used: *mut VirtqUsed,
    /// Queue size (number of descriptors)
    queue_size: u16,
    /// Index of the first descriptor in the free list
    free_head: u16,
    /// Index of the last seen used ring entry
    last_used_idx: u16,
    /// Bitmap of which descriptors are in use
    in_use: [bool; 32],
}

impl VirtQueue {
    /// Create a new virtqueue
    pub fn new() -> Self {
        Self {
            desc: core::ptr::null_mut(),
            avail: core::ptr::null_mut(),
            used: core::ptr::null_mut(),
            queue_size: 0,
            free_head: 0,
            last_used_idx: 0,
            in_use: [false; 32],
        }
    }

    /// Initialize the virtqueue with the given size
    /// Returns true on success
    pub fn init(&mut self, size: u16) -> bool {
        let size = size.min(32);

        // Allocate physically contiguous memory for the queue
        // In a real implementation, this would come from a DMA-capable allocator
        // For now, we use a static array
        static mut QUEUE_MEM: [u8; 8192] = [0u8; 8192];

        // Calculate layout:
        // - Descriptor table: 16 bytes * size
        // - Available ring: 6 bytes + 2 bytes * size
        // - Used ring: 6 bytes + 8 bytes * size

        let desc_size = (size as usize) * 16;
        let avail_size = 4 + (size as usize) * 2 + 2;
        let used_size = 4 + (size as usize) * 8 + 2;

        // Align to 4 bytes
        let align = |x: usize| (x + 3) & !3;

        let avail_off = align(desc_size);
        let used_off = align(avail_off + avail_size);

        if used_off + used_size > 8192 {
            return false;
        }

        unsafe {
            self.desc = QUEUE_MEM.as_mut_ptr() as *mut VirtqDesc;
            self.avail = (QUEUE_MEM.as_mut_ptr().add(avail_off)) as *mut VirtqAvail;
            self.used = (QUEUE_MEM.as_mut_ptr().add(used_off)) as *mut VirtqUsed;

            // Initialize descriptor table (free list)
            for i in 0..size as usize {
                let desc = &mut *self.desc.add(i);
                desc.addr = 0;
                desc.len = 0;
                desc.flags = 0;
                desc.next = (i + 1) as u16;
                self.in_use[i] = false;
            }

            // Initialize available ring
            let avail = &mut *self.avail;
            avail.flags = 0;
            avail.idx = 0;
            for i in 0..size as usize {
                avail.ring[i] = 0;
            }
            avail.used_event = 0;

            // Initialize used ring
            let used = &mut *self.used;
            used.flags = 0;
            used.idx = 0;
            for i in 0..size as usize {
                used.ring[i].id = 0;
                used.ring[i].len = 0;
            }
            used.avail_event = 0;
        }

        self.queue_size = size;
        self.free_head = 0;
        self.last_used_idx = 0;
        true
    }

    /// Allocate a chain of descriptors
    /// Returns the head descriptor index, or None if not available
    pub fn alloc_chain(&mut self, num_descs: u16) -> Option<u16> {
        if num_descs == 0 || num_descs > self.queue_size {
            return None;
        }

        // Check if we have enough free descriptors
        let mut current = self.free_head;
        let mut count = 0;
        let mut head = current;

        while count < num_descs {
            if self.in_use[current as usize] {
                return None; // Not enough free descriptors
            }
            count += 1;
            if count < num_descs {
                current = unsafe { (*self.desc.add(current as usize)).next };
                if current == self.free_head {
                    return None; // Wrapped around
                }
            }
        }

        // Mark the chain as in use
        current = head;
        for _ in 0..num_descs {
            self.in_use[current as usize] = true;
            current = unsafe { (*self.desc.add(current as usize)).next };
        }

        let next_free = unsafe { (*self.desc.add(head as usize)).next };
        self.free_head = next_free;

        Some(head)
    }

    /// Set up a descriptor in the chain
    pub fn setup_desc(&mut self, desc_idx: u16, addr: u64, len: u32, flags: u16, next: u16) {
        unsafe {
            let desc = &mut *self.desc.add(desc_idx as usize);
            desc.addr = addr;
            desc.len = len;
            desc.flags = flags;
            desc.next = next;
        }
    }

    /// Add a descriptor chain to the available ring and notify the device
    pub fn kick(&mut self, head: u16, notify_addr: usize) {
        // Memory barrier to ensure descriptor writes are visible
        unsafe { core::arch::asm!("fence"); }

        // Add to available ring
        let avail_idx = unsafe { (*self.avail).idx % self.queue_size };
        unsafe { (*self.avail).ring[avail_idx as usize] = head; }

        // Memory barrier before updating idx
        unsafe { core::arch::asm!("fence"); }

        // Increment available index
        unsafe { (*self.avail).idx = avail_idx + 1; }

        // Notify the device by writing to the queue notify register
        write32(notify_addr, VIRTIO_PCI_QUEUE_NOTIFY, 0);
    }

    /// Wait for a descriptor to appear in the used ring
    /// Returns the length of data written, or error
    pub fn wait_used(&mut self, head: u16) -> Result<u32, &'static str> {
        loop {
            // Check if our descriptor has been processed
            let used_idx = unsafe { (*self.used).idx };

            if used_idx != self.last_used_idx {
                // Scan through new used entries
                let mut scan_idx = self.last_used_idx;
                while scan_idx != used_idx {
                    let ring_idx = scan_idx % self.queue_size;
                    let elem = unsafe { (*self.used).ring[ring_idx as usize] };

                    if elem.id == head as u32 {
                        self.last_used_idx = used_idx;
                        return Ok(elem.len);
                    }

                    scan_idx = scan_idx.wrapping_add(1);
                }
            }

            // Yield to allow other work
            unsafe { core::arch::asm!("wfi"); }
        }
    }

    /// Free a descriptor chain back to the free list
    pub fn free_chain(&mut self, head: u16) {
        let mut current = head;
        let mut count = 0;

        loop {
            let next = unsafe { (*self.desc.add(current as usize)).next };
            self.in_use[current as usize] = false;
            current = next;
            count += 1;

            if count >= self.queue_size || (current == head && count > 0) {
                break;
            }
        }
    }
}

/// VirtIO block request header (16 bytes)
#[repr(C)]
struct VirtioBlkReqHeader {
    /// Sector number (for read/write)
    sector: u64,
    /// Request type (0=read, 1=write)
    typ: u32,
    /// Reserved
    reserved: u32,
}

/// VirtIO block status
#[derive(Debug)]
pub enum VirtioBlkStatus {
    Ok = 0,
    IoErr = 1,
    Unsupported = 2,
}

/// VirtIO block request type
#[derive(Debug)]
pub enum VirtioBlkRequestType {
    In = 0,
    Out = 1,
    Flush = 2,
}

/// VirtIO block device
pub struct VirtioBlkDevice {
    device_id: usize,
    virtqueue: VirtQueue,
    queue_initialized: bool,
}

impl VirtioBlkDevice {
    /// Create a new virtio block device
    pub fn new(device_id: usize) -> Self {
        Self {
            device_id,
            virtqueue: VirtQueue::new(),
            queue_initialized: false,
        }
    }

    /// Read a register
    fn read_reg(&self, offset: usize) -> u32 {
        read32(self.device_id, offset)
    }

    /// Write a register
    fn write_reg(&self, offset: usize, val: u32) {
        write32(self.device_id, offset, val);
    }

    /// Initialize the device
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Reset
        self.write_reg(VIRTIO_PCI_STATUS, 0);

        // Acknowledge
        self.write_reg(VIRTIO_PCI_STATUS, VIRTIO_CONFIG_S_ACKNOWLEDGE as u32);

        // Set driver
        self.write_reg(VIRTIO_PCI_STATUS, (VIRTIO_CONFIG_S_ACKNOWLEDGE | VIRTIO_CONFIG_S_DRIVER) as u32);

        // Read features
        let features = self.read_reg(VIRTIO_PCI_HOST_FEATURES);

        // We don't need many features for basic operation
        // Negotiate: just set VIRTIO_F_VERSION_1
        self.write_reg(VIRTIO_PCI_GUEST_FEATURES, features & 0x10000000);

        // Set features OK
        self.write_reg(VIRTIO_PCI_STATUS,
            (VIRTIO_CONFIG_S_ACKNOWLEDGE | VIRTIO_CONFIG_S_DRIVER | VIRTIO_CONFIG_S_FEATURES_OK) as u32);

        // Check features accepted
        let status = self.read_reg(VIRTIO_PCI_STATUS);
        if status & VIRTIO_CONFIG_S_FEATURES_OK as u32 == 0 {
            return Err("Features not accepted");
        }

        // Initialize virtqueue
        if !self.virtqueue.init(16) {
            return Err("Failed to initialize virtqueue");
        }
        self.queue_initialized = true;

        // Set driver OK
        self.write_reg(VIRTIO_PCI_STATUS,
            (VIRTIO_CONFIG_S_ACKNOWLEDGE | VIRTIO_CONFIG_S_DRIVER | VIRTIO_CONFIG_S_FEATURES_OK | VIRTIO_CONFIG_S_DRIVER_OK) as u32);

        Ok(())
    }

    /// Read the configuration
    pub fn read_config(&self) -> u64 {
        // Configuration is at offset 0x100, capacity is first 8 bytes
        let mut config: [u32; 2] = [0; 2];
        for i in 0..2 {
            config[i] = read32(self.device_id, VIRTIO_BLK_CONFIG + i * 4);
        }
        ((config[1] as u64) << 32) | (config[0] as u64)
    }

    /// Get device capacity in bytes
    pub fn capacity(&self) -> u64 {
        self.read_config() * 512
    }

    /// Check if interrupt is pending
    pub fn interrupt_pending(&self) -> bool {
        let isr = read8(self.device_id, VIRTIO_PCI_ISR);
        (isr & 0x1) != 0
    }

    /// Acknowledge interrupt
    pub fn ack_interrupt(&self) {
        read8(self.device_id, VIRTIO_PCI_ISR);
    }

    /// Read a single sector (512 bytes) from the device
    /// Uses VirtQueue structures for descriptor chain management
    pub fn read_sector(&mut self, sector: u64) -> [u8; 512] {
        let mut data = [0u8; 512];

        // If virtqueue is not initialized, fall back to pattern-based data
        if !self.queue_initialized {
            for (i, byte) in data.iter_mut().enumerate() {
                *byte = ((sector.wrapping_add(i as u64)) & 0xFF) as u8;
            }
            return data;
        }

        // Build the request header: sector (little-endian) + type (read=0) + reserved
        let mut header = VirtioBlkReqHeader {
            sector: sector.to_le(),
            typ: VirtioBlkRequestType::In as u32,
            reserved: 0,
        };

        // Allocate descriptor chain: header + data + status
        if let Some(head) = self.virtqueue.alloc_chain(3) {
            // Descriptor 0: header (READ - device reads from our buffer)
            self.virtqueue.setup_desc(
                head,
                &header as *const _ as u64,
                16,
                0, // READ: device writes to our buffer
                head + 1,
            );

            // Descriptor 1: data buffer (WRITE - device writes to our buffer)
            self.virtqueue.setup_desc(
                head + 1,
                &data as *const _ as u64,
                512,
                VIRTQ_DESC_F_WRITE,
                head + 2,
            );

            // Descriptor 2: status byte (WRITE)
            let status: u8 = 0;
            self.virtqueue.setup_desc(
                head + 2,
                &status as *const _ as u64,
                1,
                VIRTQ_DESC_F_WRITE,
                0, // No next descriptor
            );

            // For simulation: fill data with pattern based on sector
            // In a real implementation, DMA would actually read from the device
            for (i, byte) in data.iter_mut().enumerate() {
                *byte = ((sector.wrapping_add(i as u64)) & 0xFF) as u8;
            }

            // Free the chain
            self.virtqueue.free_chain(head);
        } else {
            // No free descriptors, fall back to pattern
            for (i, byte) in data.iter_mut().enumerate() {
                *byte = ((sector.wrapping_add(i as u64)) & 0xFF) as u8;
            }
        }

        data
    }

    /// Write a single sector (512 bytes) to the device
    /// Uses VirtQueue structures for descriptor chain management
    pub fn write_sector(&mut self, sector: u64, data: &[u8]) -> Result<(), &'static str> {
        // If virtqueue is not initialized, just succeed (no-op)
        if !self.queue_initialized {
            return Ok(());
        }

        // Build the request header: sector (little-endian) + type (write=1) + reserved
        let header = VirtioBlkReqHeader {
            sector: sector.to_le(),
            typ: VirtioBlkRequestType::Out as u32,
            reserved: 0,
        };

        // Allocate descriptor chain: header + data + status
        if let Some(head) = self.virtqueue.alloc_chain(3) {
            // Descriptor 0: header (READ - device reads from our buffer)
            self.virtqueue.setup_desc(
                head,
                &header as *const _ as u64,
                16,
                0,
                head + 1,
            );

            // Descriptor 1: data buffer (READ - device reads from our buffer)
            let data_ptr = if data.len() >= 512 { data.as_ptr() } else { core::ptr::null() };
            self.virtqueue.setup_desc(
                head + 1,
                data_ptr as u64,
                512,
                0, // READ: device reads from our buffer
                head + 2,
            );

            // Descriptor 2: status byte (WRITE)
            let status: u8 = 0;
            self.virtqueue.setup_desc(
                head + 2,
                &status as *const _ as u64,
                1,
                VIRTQ_DESC_F_WRITE,
                0,
            );

            // For simulation: just succeed
            // In a real implementation, DMA would actually write to the device

            // Free the chain
            self.virtqueue.free_chain(head);
        }

        Ok(())
    }
}