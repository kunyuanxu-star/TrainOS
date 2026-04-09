//! Procfs - Process Information Virtual Filesystem
//!
//! Provides virtual filesystem for process information

/// Procfs entry types
pub const PROCFS_TYPE_DIR: u8 = 1;
pub const PROCFS_TYPE_FILE: u8 = 2;
pub const PROCFS_TYPE_LINK: u8 = 3;

/// Maximum path length
pub const MAX_PATH: usize = 256;

/// Maximum entries in a directory
pub const MAX_ENTRIES: usize = 64;

/// Directory entry
#[repr(C)]
pub struct ProcfsEntry {
    pub name: [u8; 32],
    pub entry_type: u8,
    pub size: u32,
}

impl ProcfsEntry {
    pub fn new_dir(name: &str) -> Self {
        let mut e = Self {
            name: [0; 32],
            entry_type: PROCFS_TYPE_DIR,
            size: 0,
        };
        e.set_name(name);
        e
    }

    pub fn new_file(name: &str, size: u32) -> Self {
        let mut e = Self {
            name: [0; 32],
            entry_type: PROCFS_TYPE_FILE,
            size,
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

/// Procfs inode numbers
pub mod ino {
    pub const PROC_ROOT: u64 = 1;
    pub const PROC_SELF: u64 = 2;
    pub const PROC_PID: u64 = 100;
    pub const PROC_CMDLINE: u64 = 200;
    pub const PROC_MEMINFO: u64 = 201;
    pub const PROC_CPUINFO: u64 = 202;
    pub const PROC_VERSION: u64 = 203;
}

/// Read process command line
pub fn read_cmdline(_pid: u32, buf: &mut [u8]) -> usize {
    // For now, return empty cmdline
    // In a full implementation, this would read from process structure
    let cmdline = b"trainos\0";
    let len = cmdline.len().min(buf.len());
    buf[..len].copy_from_slice(&cmdline[..len]);
    len
}

/// Read memory info
pub fn read_meminfo(buf: &mut [u8]) -> usize {
    // Simplified meminfo
    let info = b"MemTotal:        2048000 kB\nMemFree:         1024000 kB\n";
    let len = info.len().min(buf.len());
    buf[..len].copy_from_slice(&info[..len]);
    len
}

/// Read CPU info
pub fn read_cpuinfo(buf: &mut [u8]) -> usize {
    let info = b"CPU: RISC-V\nArchitecture: rv64gc\n";
    let len = info.len().min(buf.len());
    buf[..len].copy_from_slice(&info[..len]);
    len
}

/// Read version
pub fn read_version(buf: &mut [u8]) -> usize {
    let info = b"TrainOS 0.1.0\n";
    let len = info.len().min(buf.len());
    buf[..len].copy_from_slice(&info[..len]);
    len
}