//! VirtIO Device Driver Infrastructure
//!
//! VirtIO is a para-virtualization standard for I/O devices
//! See: https://docs.oasis-open.org/virtio/virtio/v1.1/virtio-v1.1.pdf

use spin::Mutex;

/// VirtIO device types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VirtioDeviceType {
    Invalid = 0,
    Network = 1,
    Block = 2,
    Console = 3,
    Entropy = 4,       // RNG
    MemoryBalloon = 5,
    IoThread = 6,
    Gpu = 16,
    Input = 18,
    Crypto = 20,
    Socket = 24,
}

/// VirtIO PCI vendor ID (all virtio devices use this)
pub const VIRTIO_VENDOR_ID: u16 = 0x1AF4;

/// VirtIO PCI device IDs (device type encoded in lower bits)
pub const VIRTIO_PCI_DEVICE_ID_BASE: u16 = 0x1000;

/// VirtIO header offsets (legacy)
pub const VIRTIO_PCI_HOST_FEATURES: usize = 0;
pub const VIRTIO_PCI_GUEST_FEATURES: usize = 4;
pub const VIRTIO_PCI_QUEUE_PFN: usize = 8;
pub const VIRTIO_PCI_QUEUE_NUM: usize = 12;
pub const VIRTIO_PCI_QUEUE_SEL: usize = 14;
pub const VIRTIO_PCI_QUEUE_NOTIFY: usize = 16;
pub const VIRTIO_PCI_STATUS: usize = 18;
pub const VIRTIO_PCI_ISR: usize = 19;

/// Modern virtio-pci capability types
#[derive(Debug, Clone, Copy)]
pub enum VirtioPciCapType {
    Common = 1,
    Notify = 2,
    Isr = 3,
    DeviceSpecific = 4,
    PciConfig = 5,
}

/// VirtIO status bits
pub const VIRTIO_CONFIG_S_ACKNOWLEDGE: u8 = 1;
pub const VIRTIO_CONFIG_S_DRIVER: u8 = 2;
pub const VIRTIO_CONFIG_S_DRIVER_OK: u8 = 4;
pub const VIRTIO_CONFIG_S_FEATURES_OK: u8 = 8;
pub const VIRTIO_CONFIG_S_FAILED: u8 = 0x80;

/// VirtIO config offset (modern)
pub const VIRTIO_PCI_CAP_DEVICE_CFG: usize = 4;

/// VirtIO device features
pub const VIRTIO_F_RING_EVENT_IDX: u32 = 0x40000000;
pub const VIRTIO_F_RING_INDIRECT_DESC: u32 = 0x40000000;
pub const VIRTIO_F_VERSION_1: u32 = 0x10000000;
pub const VIRTIO_F_ACCESS_PLATFORM: u32 = 0x20000000;

/// Virtqueue alignment (typical)
pub const VIRTIO_VRING_ALIGN: usize = 4096;

/// VirtIO interrupt status bits
pub const VIRTIO_PCI_ISR_INTR: u8 = 0x1;
pub const VIRTIO_PCI_ISR_CONFIG: u8 = 0x2;

/// Global virtio status
static VIRTIO_INITIALIZED: Mutex<bool> = Mutex::new(false);

/// Check if virtio is initialized
pub fn virtio_initialized() -> bool {
    *VIRTIO_INITIALIZED.lock()
}

/// Mark virtio as initialized
pub fn set_virtio_initialized() {
    *VIRTIO_INITIALIZED.lock() = true;
}
