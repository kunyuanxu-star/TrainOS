//! Sysfs - System Information Virtual Filesystem
//!
//! Provides virtual filesystem for kernel objects

/// Sysfs entry types
pub const SYSFS_TYPE_DIR: u8 = 1;
pub const SYSFS_TYPE_FILE: u8 = 2;
pub const SYSFS_TYPE_LINK: u8 = 3;

/// Maximum path length
pub const MAX_PATH: usize = 256;

/// Directory entry
#[repr(C)]
pub struct SysfsEntry {
    pub name: [u8; 32],
    pub entry_type: u8,
}

impl SysfsEntry {
    pub fn new_dir(name: &str) -> Self {
        let mut e = Self {
            name: [0; 32],
            entry_type: SYSFS_TYPE_DIR,
        };
        e.set_name(name);
        e
    }

    pub fn new_file(name: &str) -> Self {
        let mut e = Self {
            name: [0; 32],
            entry_type: SYSFS_TYPE_FILE,
        };
        e.set_name(name);
        e
    }

    fn set_name(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(31);
        self.name[..len].copy_from_slice(&bytes[..len]);
    }
}

/// Sysfs inode numbers
pub mod ino {
    pub const SYS_ROOT: u64 = 1;
    pub const SYS_CLASS: u64 = 2;
    pub const SYS_CLASS_NET: u64 = 10;
    pub const SYS_DEVICE: u64 = 20;
}

/// Network interface info
#[repr(C)]
pub struct NetDeviceInfo {
    pub name: [u8; 16],
    pub mtu: u32,
    pub flags: u32,
    pub mac: [u8; 6],
}

impl NetDeviceInfo {
    pub fn new(name: &str) -> Self {
        let mut dev = Self {
            name: [0; 16],
            mtu: 1500,
            flags: 0x89, // IFF_UP | IFF_RUNNING | IFF_BROADCAST | IFF_MULTICAST
            mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
        };
        let name_bytes = name.as_bytes();
        let len = name_bytes.len().min(15);
        dev.name[..len].copy_from_slice(&name_bytes[..len]);
        dev
    }
}

/// Read network interface address
pub fn read_net_address(buf: &mut [u8], _iface_name: &str) -> usize {
    // Return MAC address
    let mac = b"52:54:00:12:34:56\n";
    let len = mac.len().min(buf.len());
    buf[..len].copy_from_slice(&mac[..len]);
    len
}

/// Read network interface mtu
pub fn read_net_mtu(buf: &mut [u8], _iface_name: &str) -> usize {
    let mtu = b"1500\n";
    let len = mtu.len().min(buf.len());
    buf[..len].copy_from_slice(&mtu[..len]);
    len
}

/// Read network interface flags
pub fn read_net_flags(buf: &mut [u8], _iface_name: &str) -> usize {
    let flags = b"0x89\n";
    let len = flags.len().min(buf.len());
    buf[..len].copy_from_slice(&flags[..len]);
    len
}