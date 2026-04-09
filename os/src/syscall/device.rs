//! Device access syscalls for driver services
//!
//! Provides controlled access to device MMIO registers for user-space drivers.

/// Device syscall numbers (custom TrainOS)
pub const DEVICE_READ: usize = 1100;
pub const DEVICE_WRITE: usize = 1101;
pub const DEVICE_INTERRUPT_ENABLE: usize = 1102;

/// Maximum devices
const MAX_DEVICES: usize = 4;

/// Device type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeviceType {
    None,
    VirtioBlk,
    VirtioNet,
}

/// Device info
#[derive(Debug, Copy, Clone)]
struct DeviceInfo {
    device_type: DeviceType,
    base_addr: usize,
}

impl DeviceInfo {
    pub const fn new() -> Self {
        Self {
            device_type: DeviceType::None,
            base_addr: 0,
        }
    }
}

/// Device table
static DEVICE_TABLE: spin::Mutex<[DeviceInfo; MAX_DEVICES]> =
    spin::Mutex::new([DeviceInfo::new(); MAX_DEVICES]);

/// Device MMIO regions (QEMU virt - known addresses)
/// In a real system, these would be discovered via PCI
const VIRTIO_BLK_BASE: usize = 0x10000000;
const VIRTIO_NET_BASE: usize = 0x10010000;

/// Initialize device table
pub fn init_devices() {
    let mut table = DEVICE_TABLE.lock();
    // Register VirtIO block at known address (QEMU virt)
    table[0] = DeviceInfo {
        device_type: DeviceType::VirtioBlk,
        base_addr: VIRTIO_BLK_BASE,
    };
    // Register VirtIO net
    table[1] = DeviceInfo {
        device_type: DeviceType::VirtioNet,
        base_addr: VIRTIO_NET_BASE,
    };
}

/// sys_device_read - Read from device MMIO
/// a0 = device_id, a1 = offset, a2 = count (1-4 bytes)
pub fn sys_device_read(device_id: usize, offset: usize, count: usize) -> isize {
    if device_id >= MAX_DEVICES {
        return -1;
    }

    let table = DEVICE_TABLE.lock();
    let device = &table[device_id];

    if device.device_type == DeviceType::None {
        return -1;
    }

    // Validate count (1, 2, or 4 bytes)
    if count != 1 && count != 2 && count != 4 {
        return -1;
    }

    let addr = device.base_addr + offset;

    // Read based on count
    match count {
        1 => unsafe { *(addr as *const u8) as isize },
        2 => unsafe { *(addr as *const u16) as isize },
        4 => unsafe { *(addr as *const u32) as isize },
        _ => -1,
    }
}

/// sys_device_write - Write to device MMIO
/// a0 = device_id, a1 = offset, a2 = value, a3 = count (1-4 bytes)
pub fn sys_device_write(device_id: usize, offset: usize, value: usize, count: usize) -> isize {
    if device_id >= MAX_DEVICES {
        return -1;
    }

    let table = DEVICE_TABLE.lock();
    let device = &table[device_id];

    if device.device_type == DeviceType::None {
        return -1;
    }

    // Validate count
    if count != 1 && count != 2 && count != 4 {
        return -1;
    }

    let addr = device.base_addr + offset;

    match count {
        1 => unsafe { *(addr as *mut u8) = value as u8 },
        2 => unsafe { *(addr as *mut u16) = value as u16 },
        4 => unsafe { *(addr as *mut u32) = value as u32 },
        _ => return -1,
    }

    0
}

/// sys_device_interrupt_enable - Enable interrupt delivery to calling process
/// a0 = device_id, a1 = interrupt_id (ignored for now)
/// Returns 0 on success, -1 on error
pub fn sys_device_interrupt_enable(device_id: usize, _interrupt_id: usize) -> isize {
    if device_id >= MAX_DEVICES {
        return -1;
    }

    let table = DEVICE_TABLE.lock();
    let device = &table[device_id];

    if device.device_type == DeviceType::None {
        return -1;
    }

    // For now, just return success
    // Real implementation would register the process for interrupt notifications
    0
}