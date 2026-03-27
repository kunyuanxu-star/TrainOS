//! PCI Bus Driver
//!
//! Provides PCI device discovery and configuration

use spin::Mutex;

/// PCI configuration space
pub const PCI_CONFIG_ADDRESS: usize = 0xCF8;
pub const PCI_CONFIG_DATA: usize = 0xCFC;

/// PCI vendor IDs
pub const PCI_VENDOR_ID: u16 = 0x1AF4;  // Red Hat / VirtIO

/// PCI device class codes
pub const PCI_CLASS_STORAGE: u8 = 0x01;
pub const PCI_CLASS_NETWORK: u8 = 0x02;
pub const PCI_CLASS_INPUT: u8 = 0x09;

/// PCI configuration header
#[derive(Debug, Clone, Copy)]
pub struct PciDevice {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class_code: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub revision: u8,
}

/// Read PCI configuration word
pub fn pci_config_read(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
    let address = (1u32 << 31)
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);

    unsafe {
        (PCI_CONFIG_ADDRESS as *mut u32).write_volatile(address);
        (PCI_CONFIG_DATA as *const u32).read_volatile()
    }
}

/// Write PCI configuration word
pub fn pci_config_write(bus: u8, dev: u8, func: u8, offset: u8, val: u32) {
    let address = (1u32 << 31)
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC);

    unsafe {
        (PCI_CONFIG_ADDRESS as *mut u32).write_volatile(address);
        (PCI_CONFIG_DATA as *mut u32).write_volatile(val);
    }
}

/// Read PCI device ID and vendor ID
pub fn pci_get_vendor(bus: u8, dev: u8, func: u8) -> u16 {
    pci_config_read(bus, dev, func, 0) as u16
}

/// Check if a PCI device exists
pub fn pci_device_exists(bus: u8, dev: u8, func: u8) -> bool {
    pci_get_vendor(bus, dev, func) != 0xFFFF
}

/// Get PCI header type
pub fn pci_get_header_type(bus: u8, dev: u8, func: u8) -> u8 {
    (pci_config_read(bus, dev, func, 0x0E) >> 16) as u8
}

/// Get PCI BAR address
pub fn pci_get_bar(bus: u8, dev: u8, func: u8, bar: usize) -> u32 {
    pci_config_read(bus, dev, func, 0x10 + (bar as u8) * 4)
}

/// Scan for VirtIO devices
pub fn scan_virtio_devices() -> [Option<PciDevice>; 8] {
    let mut devices: [Option<PciDevice>; 8] = [None; 8];
    let mut count = 0;

    // Scan PCI bus 0
    for dev in 0..32 {
        for func in 0..8 {
            if !pci_device_exists(0, dev, func) {
                if func == 0 {
                    break;
                }
                continue;
            }

            let vendor = pci_get_vendor(0, dev, func);
            if vendor == PCI_VENDOR_ID {
                let device_id = (pci_config_read(0, dev, func, 0) >> 16) as u16;
                let class = pci_config_read(0, dev, func, 0x08);
                let class_code = (class >> 24) as u8;
                let subclass = (class >> 16) as u8;
                let prog_if = (class >> 8) as u8;
                let revision = (class) as u8;

                if count < 8 {
                    devices[count] = Some(PciDevice {
                        bus: 0,
                        device: dev,
                        function: func,
                        vendor_id: vendor,
                        device_id,
                        class_code,
                        subclass,
                        prog_if,
                        revision,
                    });
                    count += 1;
                }
            }

            // Only check func 0 for non-multi-function devices
            if func == 0 {
                let header = pci_get_header_type(0, dev, func);
                if header & 0x80 == 0 {
                    break;
                }
            }
        }
    }

    devices
}

/// VirtIO device type from device ID
pub fn virtio_device_type(device_id: u16) -> super::virtio::VirtioDeviceType {
    match device_id & 0xFF {
        1 => super::virtio::VirtioDeviceType::Network,
        2 => super::virtio::VirtioDeviceType::Block,
        3 => super::virtio::VirtioDeviceType::Console,
        4 => super::virtio::VirtioDeviceType::Entropy,
        5 => super::virtio::VirtioDeviceType::MemoryBalloon,
        16 => super::virtio::VirtioDeviceType::Gpu,
        18 => super::virtio::VirtioDeviceType::Input,
        20 => super::virtio::VirtioDeviceType::Crypto,
        24 => super::virtio::VirtioDeviceType::Socket,
        _ => super::virtio::VirtioDeviceType::Invalid,
    }
}

/// PCI interrupt pin
pub fn pci_get_interrupt_pin(bus: u8, dev: u8, func: u8) -> u8 {
    (pci_config_read(bus, dev, func, 0x3C) >> 8) as u8
}
