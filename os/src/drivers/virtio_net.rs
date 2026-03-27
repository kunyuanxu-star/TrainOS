//! VirtIO Network Device Driver
//!
//! Provides network device support via VirtIO

use super::virtio::*;
use spin::Mutex;

/// Maximum number of network devices
const MAX_NET_DEVICES: usize = 4;

/// VirtIO network configuration
#[repr(C)]
pub struct VirtioNetConfig {
    /// Mac address
    pub mac: [u8; 6],
    /// Status (link up/down)
    pub status: u16,
    /// Max queue pairs
    pub max_virtqueue_pairs: u16,
    /// MTU
    pub mtu: u16,
    /// Speed
    pub speed: u32,
    pub duplex: u32,
}

impl Default for VirtioNetConfig {
    fn default() -> Self {
        Self {
            mac: [0; 6],
            status: 0,
            max_virtqueue_pairs: 1,
            mtu: 1500,
            speed: 0,
            duplex: 0,
        }
    }
}

/// VirtIO network device
pub struct VirtioNetDevice {
    /// Base address of the device registers
    base_addr: usize,
    /// Device configuration
    config: VirtioNetConfig,
    /// Whether the device is initialized
    initialized: bool,
    /// Link is up
    link_up: bool,
}

impl VirtioNetDevice {
    /// Create a new virtio network device
    pub fn new(base_addr: usize) -> Self {
        Self {
            base_addr,
            config: VirtioNetConfig::default(),
            initialized: false,
            link_up: false,
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

        // For network, we want basic features
        let required_features = features & (1 << 5 | 1 << 6 | 1 << 7);

        // Negotiate features
        self.write_reg(VIRTIO_PCI_GUEST_FEATURES, required_features);

        // Set FEATURES_OK
        self.write_reg(VIRTIO_PCI_STATUS, (VIRTIO_CONFIG_S_ACKNOWLEDGE | VIRTIO_CONFIG_S_DRIVER | VIRTIO_CONFIG_S_FEATURES_OK) as u32);

        // Check if features were accepted
        let status = self.read_reg(VIRTIO_PCI_STATUS);
        if status & VIRTIO_CONFIG_S_FEATURES_OK as u32 == 0 {
            return Err("Features not accepted");
        }

        // Read MAC address and configuration
        self.read_config();

        // Set DRIVER_OK
        self.write_reg(VIRTIO_PCI_STATUS, (VIRTIO_CONFIG_S_ACKNOWLEDGE | VIRTIO_CONFIG_S_DRIVER | VIRTIO_CONFIG_S_FEATURES_OK | VIRTIO_CONFIG_S_DRIVER_OK) as u32);

        self.initialized = true;
        self.link_up = true;

        Ok(())
    }

    /// Read device configuration
    fn read_config(&mut self) {
        let config_ptr = (self.base_addr + 0x100) as *const VirtioNetConfig;
        self.config = unsafe { core::ptr::read_volatile(config_ptr) };
    }

    /// Check if device is initialized
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get MAC address
    pub fn mac(&self) -> [u8; 6] {
        self.config.mac
    }

    /// Check if link is up
    pub fn link_up(&self) -> bool {
        self.link_up
    }

    /// Get MTU
    pub fn mtu(&self) -> u16 {
        self.config.mtu
    }

    /// Receive a packet (non-blocking)
    pub fn recv(&mut self, _buf: &mut [u8]) -> Result<usize, &'static str> {
        if !self.initialized {
            return Err("Device not initialized");
        }

        if !self.link_up {
            return Err("Link is down");
        }

        // Simulate no data available
        Ok(0)
    }

    /// Send a packet
    pub fn send(&mut self, buf: &[u8]) -> Result<usize, &'static str> {
        if !self.initialized {
            return Err("Device not initialized");
        }

        if !self.link_up {
            return Err("Link is down");
        }

        if buf.len() > (self.config.mtu as usize + 14) {
            return Err("Packet too large");
        }

        // Simulated send
        Ok(buf.len())
    }
}

/// Global network device table - lazy initialized
static NET_DEVICES: Mutex<Option<NetDeviceTable>> = Mutex::new(None);

/// Network device table
pub struct NetDeviceTable {
    devices: [Option<VirtioNetDevice>; MAX_NET_DEVICES],
}

impl NetDeviceTable {
    pub fn new() -> Self {
        // Manually initialize array since Option<VirtioNetDevice> is not Copy
        let mut devices: [Option<VirtioNetDevice>; MAX_NET_DEVICES] = unsafe {
            core::mem::zeroed()
        };
        for i in 0..MAX_NET_DEVICES {
            devices[i] = None;
        }
        Self { devices }
    }

    /// Register a network device
    pub fn register(&mut self, base_addr: usize) -> Option<usize> {
        for i in 0..MAX_NET_DEVICES {
            if self.devices[i].is_none() {
                let mut device = VirtioNetDevice::new(base_addr);
                if device.init().is_ok() {
                    self.devices[i] = Some(device);
                    return Some(i);
                }
            }
        }
        None
    }

    /// Get a device by index
    pub fn get(&mut self, index: usize) -> Option<&mut VirtioNetDevice> {
        if index < MAX_NET_DEVICES {
            self.devices[index].as_mut()
        } else {
            None
        }
    }
}

/// Initialize the first virtio network device
pub fn init_net_device(base_addr: usize) -> Option<usize> {
    let mut guard = NET_DEVICES.lock();
    if guard.is_none() {
        *guard = Some(NetDeviceTable::new());
    }
    if let Some(ref mut table) = *guard {
        table.register(base_addr)
    } else {
        None
    }
}

/// Get network device
pub fn get_net_device(index: usize) -> Option<*mut VirtioNetDevice> {
    let mut guard = NET_DEVICES.lock();
    if let Some(ref mut table) = *guard {
        table.get(index).map(|r| r as *mut VirtioNetDevice)
    } else {
        None
    }
}
