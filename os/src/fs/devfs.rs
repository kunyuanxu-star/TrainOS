//! Device File System (devfs)
//!
//! Provides access to device files

use super::vfs::*;
use spin::Mutex;

/// Device types for devfs
#[derive(Debug, Clone, Copy)]
pub enum DeviceType {
    Null,
    Zero,
    Random,
    Console,
    Directory,
}

impl DeviceType {
    pub fn to_file_type(&self) -> FileType {
        match self {
            DeviceType::Directory => FileType::Directory,
            _ => FileType::CharDevice,
        }
    }
}

/// Device inode
pub struct DeviceInode {
    /// Device type
    pub dev_type: DeviceType,
    /// Inode number
    pub ino: u64,
    /// Device major/minor
    pub major: u32,
    pub minor: u32,
    /// Permissions
    pub perm: FilePerms,
}

impl DeviceInode {
    pub fn new(dev_type: DeviceType, ino: u64) -> Self {
        let (major, minor) = match dev_type {
            DeviceType::Null => (1, 3),    // /dev/null is major 1, minor 3
            DeviceType::Zero => (1, 5),    // /dev/zero is major 1, minor 5
            DeviceType::Random => (1, 8),   // /dev/random is major 1, minor 8
            DeviceType::Console => (4, 0), // /dev/console is major 4, minor 0
            DeviceType::Directory => (0, 0), // Directory has no major/minor
        };
        Self {
            dev_type,
            ino,
            major,
            minor,
            perm: FilePerms::new(0o666),
        }
    }

    /// Read from device
    pub fn read(&self, _offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
        match self.dev_type {
            DeviceType::Null => Ok(0),
            DeviceType::Zero => {
                for byte in buf.iter_mut() {
                    *byte = 0;
                }
                Ok(buf.len())
            }
            DeviceType::Random => {
                // Return pseudo-random data
                for (i, byte) in buf.iter_mut().enumerate() {
                    *byte = ((i as u8) ^ 0xAA) & 0xFF;
                }
                Ok(buf.len())
            }
            DeviceType::Console => {
                // Console input not supported yet
                Ok(0)
            }
            DeviceType::Directory => Err(VfsError::IsDirectory),
        }
    }

    /// Write to device
    pub fn write(&self, _offset: u64, buf: &[u8]) -> Result<usize, VfsError> {
        match self.dev_type {
            DeviceType::Null => Ok(buf.len()),
            DeviceType::Zero => Ok(buf.len()),
            DeviceType::Random => Ok(buf.len()),
            DeviceType::Console => {
                // Write to console via SBI
                for &c in buf {
                    crate::console::sbi_console_putchar(c as usize);
                }
                Ok(buf.len())
            }
            DeviceType::Directory => Err(VfsError::IsDirectory),
        }
    }
}

impl VfsInode for DeviceInode {
    fn attr(&self) -> Result<FileAttr, VfsError> {
        Ok(FileAttr {
            file_type: self.dev_type.to_file_type(),
            mode: 0o644,
            size: 0,
            dev: ((self.major as u64) << 32) | (self.minor as u64),
            ino: self.ino,
            nlink: 1,
            uid: 0,
            gid: 0,
            atime: 0,
            mtime: 0,
            ctime: 0,
        })
    }

    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize, VfsError> {
        self.read(offset, buf)
    }

    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize, VfsError> {
        self.write(offset, buf)
    }

    fn file_type(&self) -> FileType {
        self.dev_type.to_file_type()
    }
}

/// Device file system
pub struct DevFs {
    /// Root inode
    root: Mutex<DeviceInode>,
}

impl DevFs {
    pub fn new() -> Self {
        Self {
            root: Mutex::new(DeviceInode::new(DeviceType::Directory, 1)),
        }
    }
}

impl VfsFilesystem for DevFs {
    fn name(&self) -> &str {
        "devfs"
    }

    fn root_inode(&self) -> &Mutex<dyn VfsInode> {
        // This is a simplified implementation
        // In a real implementation, we would have proper directory handling
        &self.root
    }
}

/// Null device
pub struct NullDevice;

impl NullDevice {
    pub fn new() -> Self {
        Self
    }
}

impl VfsFile for NullDevice {
    fn read(&self, _buf: &mut [u8]) -> Result<usize, VfsError> {
        Ok(0)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, VfsError> {
        Ok(buf.len())
    }

    fn seek(&self, _offset: i64, _whence: SeekWhence) -> Result<u64, VfsError> {
        Ok(0)
    }

    fn close(self) -> Result<(), VfsError> {
        Ok(())
    }
}

/// Zero device
pub struct ZeroDevice;

impl ZeroDevice {
    pub fn new() -> Self {
        Self
    }
}

impl VfsFile for ZeroDevice {
    fn read(&self, buf: &mut [u8]) -> Result<usize, VfsError> {
        for byte in buf.iter_mut() {
            *byte = 0;
        }
        Ok(buf.len())
    }

    fn write(&self, buf: &[u8]) -> Result<usize, VfsError> {
        Ok(buf.len())
    }

    fn seek(&self, _offset: i64, _whence: SeekWhence) -> Result<u64, VfsError> {
        Ok(0)
    }

    fn close(self) -> Result<(), VfsError> {
        Ok(())
    }
}

/// Console device
pub struct ConsoleDevice;

impl ConsoleDevice {
    pub fn new() -> Self {
        Self
    }
}

impl VfsFile for ConsoleDevice {
    fn read(&self, _buf: &mut [u8]) -> Result<usize, VfsError> {
        // No console input yet
        Ok(0)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, VfsError> {
        for &c in buf {
            crate::console::sbi_console_putchar(c as usize);
        }
        Ok(buf.len())
    }

    fn seek(&self, _offset: i64, _whence: SeekWhence) -> Result<u64, VfsError> {
        Ok(0)
    }

    fn close(self) -> Result<(), VfsError> {
        Ok(())
    }
}
