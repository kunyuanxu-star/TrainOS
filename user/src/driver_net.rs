//! VirtIO Network Device Driver (User Space)
//!
//! Provides network device support via VirtIO

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

/// VirtIO net device
pub struct VirtioNetDevice {
    device_id: usize,
}

impl VirtioNetDevice {
    /// Create a new virtio net device
    pub fn new(device_id: usize) -> Self {
        Self { device_id }
    }

    fn read_reg(&self, offset: usize) -> u32 {
        read32(self.device_id, offset)
    }

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

        // Negotiate features - no special features needed for basic operation
        self.write_reg(VIRTIO_PCI_GUEST_FEATURES, features & 0x10000000);

        // Set features OK
        self.write_reg(VIRTIO_PCI_STATUS,
            (VIRTIO_CONFIG_S_ACKNOWLEDGE | VIRTIO_CONFIG_S_DRIVER | VIRTIO_CONFIG_S_FEATURES_OK) as u32);

        // Check features accepted
        let status = self.read_reg(VIRTIO_PCI_STATUS);
        if status & VIRTIO_CONFIG_S_FEATURES_OK as u32 == 0 {
            return Err("Features not accepted");
        }

        // Set driver OK
        self.write_reg(VIRTIO_PCI_STATUS,
            (VIRTIO_CONFIG_S_ACKNOWLEDGE | VIRTIO_CONFIG_S_DRIVER | VIRTIO_CONFIG_S_FEATURES_OK | VIRTIO_CONFIG_S_DRIVER_OK) as u32);

        Ok(())
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

    /// Receive a frame (stub - returns empty frame for now)
    pub fn recv_frame(&mut self, _buffer: &mut [u8]) -> Result<usize, &'static str> {
        // Check if we have a frame waiting
        if !self.interrupt_pending() {
            return Err("No frame available");
        }
        self.ack_interrupt();

        // For now, return no data - actual DMA-based receive
        // would use the virtqueue mechanism
        Ok(0)
    }

    /// Send a frame (stub)
    pub fn send_frame(&mut self, _data: &[u8]) -> Result<(), &'static str> {
        // For now, this is a stub
        // Actual DMA-based send would use the virtqueue mechanism
        Ok(())
    }
}