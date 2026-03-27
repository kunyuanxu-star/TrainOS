//! VirtIO Block Device Driver
//!
//! Provides block storage device support via VirtIO

use super::virtio::*;
use spin::Mutex;

/// Maximum number of block devices
const MAX_BLK_DEVICES: usize = 4;

/// VirtIO block configuration
#[repr(C)]
pub struct VirtioBlkConfig {
    /// Capacity in 512-byte sectors
    pub capacity: u64,
    /// Size of the physical block
    pub size_max: u32,
    /// Size of the optimal block
    pub seg_max: u32,
    /// Geometry
    pub geometry: VirtioBlkGeometry,
    /// Block size
    pub blk_size: u32,
    pub padding: [u8; 12],
}

/// Disk geometry
#[repr(C)]
pub struct VirtioBlkGeometry {
    pub cylinders: u16,
    pub heads: u8,
    pub sectors: u8,
}

/// VirtIO block status
#[derive(Debug, Clone, Copy)]
pub enum VirtioBlkStatus {
    Ok = 0,
    IoErr = 1,
    Unsupported = 2,
}

/// VirtIO block request type
#[derive(Debug, Clone, Copy)]
pub enum VirtioBlkRequestType {
    In = 0,
    Out = 1,
    Flush = 2,
    Discard = 3,
    WriteZeros = 4,
}

/// VirtIO block device
pub struct VirtioBlkDevice {
    /// Base address of the device registers
    base_addr: usize,
    /// Device configuration
    config: VirtioBlkConfig,
    /// Whether the device is initialized
    initialized: bool,
}

impl VirtioBlkDevice {
    /// Create a new virtio block device
    pub fn new(base_addr: usize) -> Self {
        Self {
            base_addr,
            config: VirtioBlkConfig {
                capacity: 0,
                size_max: 0,
                seg_max: 0,
                geometry: VirtioBlkGeometry {
                    cylinders: 0,
                    heads: 0,
                    sectors: 0,
                },
                blk_size: 512,
                padding: [0; 12],
            },
            initialized: false,
        }
    }

    /// Read a device register
    fn read_reg(&self, offset: usize) -> u32 {
        unsafe { (self.base_addr as *const u32).read_volatile() }
    }

    /// Write a device register
    fn write_reg(&self, offset: usize, val: u32) {
        unsafe { (self.base_addr as *mut u32).write_volatile(val) }
    }

    /// Initialize the device
    pub fn init(&mut self) -> Result<(), &'static str> {
        // Reset the device
        self.write_reg(VIRTIO_PCI_STATUS, 0);

        // Set ACKNOWLEDGE
        self.write_reg(VIRTIO_PCI_STATUS, VIRTIO_CONFIG_S_ACKNOWLEDGE as u32);

        // Set DRIVER
        self.write_reg(VIRTIO_PCI_STATUS, (VIRTIO_CONFIG_S_ACKNOWLEDGE | VIRTIO_CONFIG_S_DRIVER) as u32);

        // Check if we have the features we need
        let features = self.read_reg(VIRTIO_PCI_HOST_FEATURES);

        // For block devices, we don't need many features
        let required_features = 0u32;

        // Negotiate features
        self.write_reg(VIRTIO_PCI_GUEST_FEATURES, features & required_features);

        // Set FEATURES_OK
        self.write_reg(VIRTIO_PCI_STATUS, (VIRTIO_CONFIG_S_ACKNOWLEDGE | VIRTIO_CONFIG_S_DRIVER | VIRTIO_CONFIG_S_FEATURES_OK) as u32);

        // Check if features were accepted
        let status = self.read_reg(VIRTIO_PCI_STATUS);
        if status & VIRTIO_CONFIG_S_FEATURES_OK as u32 == 0 {
            return Err("Features not accepted");
        }

        // Read configuration
        self.read_config();

        // Set DRIVER_OK
        self.write_reg(VIRTIO_PCI_STATUS, (VIRTIO_CONFIG_S_ACKNOWLEDGE | VIRTIO_CONFIG_S_DRIVER | VIRTIO_CONFIG_S_FEATURES_OK | VIRTIO_CONFIG_S_DRIVER_OK) as u32);

        self.initialized = true;
        Ok(())
    }

    /// Read device configuration
    fn read_config(&mut self) {
        let config_ptr = (self.base_addr + 0x100) as *const VirtioBlkConfig;
        self.config = unsafe { core::ptr::read_volatile(config_ptr) };
    }

    /// Check if device is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get device capacity in bytes
    pub fn capacity(&self) -> u64 {
        self.config.capacity * 512
    }

    /// Read sectors from the device
    pub fn read_sectors(&mut self, sector: u64, count: usize, buf: &mut [u8]) -> Result<usize, &'static str> {
        if !self.initialized {
            return Err("Device not initialized");
        }

        if buf.len() < count * 512 {
            return Err("Buffer too small");
        }

        // Simulate successful read
        for i in 0..(count * 512).min(buf.len()) {
            buf[i] = 0;
        }

        Ok(count * 512)
    }

    /// Write sectors to the device
    pub fn write_sectors(&mut self, _sector: u64, count: usize, buf: &[u8]) -> Result<usize, &'static str> {
        if !self.initialized {
            return Err("Device not initialized");
        }

        if buf.len() < count * 512 {
            return Err("Buffer too small");
        }

        // Simulated write
        Ok(count * 512)
    }
}

/// Global block device table - lazy initialized
static BLK_DEVICES: Mutex<Option<BlkDeviceTable>> = Mutex::new(None);

/// Block device table
pub struct BlkDeviceTable {
    devices: [Option<VirtioBlkDevice>; MAX_BLK_DEVICES],
}

impl BlkDeviceTable {
    pub fn new() -> Self {
        // Manually initialize array since Option<VirtioBlkDevice> is not Copy
        let mut devices: [Option<VirtioBlkDevice>; MAX_BLK_DEVICES] = unsafe {
            core::mem::zeroed()
        };
        for i in 0..MAX_BLK_DEVICES {
            devices[i] = None;
        }
        Self { devices }
    }

    /// Register a block device
    pub fn register(&mut self, base_addr: usize) -> Option<usize> {
        for i in 0..MAX_BLK_DEVICES {
            if self.devices[i].is_none() {
                let mut device = VirtioBlkDevice::new(base_addr);
                if device.init().is_ok() {
                    self.devices[i] = Some(device);
                    return Some(i);
                }
            }
        }
        None
    }

    /// Get a device by index
    pub fn get(&mut self, index: usize) -> Option<&mut VirtioBlkDevice> {
        if index < MAX_BLK_DEVICES {
            self.devices[index].as_mut()
        } else {
            None
        }
    }
}

/// Initialize the first virtio block device
pub fn init_blk_device(base_addr: usize) -> Option<usize> {
    let mut guard = BLK_DEVICES.lock();
    if guard.is_none() {
        *guard = Some(BlkDeviceTable::new());
    }
    if let Some(ref mut table) = *guard {
        table.register(base_addr)
    } else {
        None
    }
}

/// Get block device
pub fn get_blk_device(index: usize) -> Option<*mut VirtioBlkDevice> {
    let mut guard = BLK_DEVICES.lock();
    if let Some(ref mut table) = *guard {
        table.get(index).map(|r| r as *mut VirtioBlkDevice)
    } else {
        None
    }
}
