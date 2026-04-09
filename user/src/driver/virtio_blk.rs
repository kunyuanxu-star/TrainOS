//! VirtIO Block Device Driver (User Space)
//!
//! Provides block storage device support via VirtIO
//! Uses syscalls to access device MMIO

use super::mmio::*;

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
}

impl VirtioBlkDevice {
    /// Create a new virtio block device
    pub fn new(device_id: usize) -> Self {
        Self { device_id }
    }

    /// Read a register
    fn read_reg(&self, offset: usize) -> u32 {
        mmio::read32(self.device_id, offset)
    }

    /// Write a register
    fn write_reg(&self, offset: usize, val: u32) {
        mmio::write32(self.device_id, offset, val);
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
            config[i] = mmio::read32(self.device_id, VIRTIO_BLK_CONFIG + i * 4);
        }
        ((config[1] as u64) << 32) | (config[0] as u64)
    }

    /// Get device capacity in bytes
    pub fn capacity(&self) -> u64 {
        self.read_config() * 512
    }

    /// Check if interrupt is pending
    pub fn interrupt_pending(&self) -> bool {
        let isr = mmio::read8(self.device_id, VIRTIO_PCI_ISR);
        (isr & 0x1) != 0
    }

    /// Acknowledge interrupt
    pub fn ack_interrupt(&self) {
        mmio::read8(self.device_id, VIRTIO_PCI_ISR);
    }
}