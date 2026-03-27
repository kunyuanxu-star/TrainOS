//! Virtual File System (VFS) Layer
//!
//! Provides a unified interface for different file systems

use spin::Mutex;

/// Maximum file name length
pub const MAX_FNAME_LEN: usize = 255;
/// Maximum path components
pub const MAX_PATH_DEPTH: usize = 32;

/// File types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileType {
    Unknown,
    RegularFile,
    Directory,
    CharDevice,
    BlockDevice,
    Pipe,
    Socket,
    Symlink,
}

impl FileType {
    pub fn from_u16(mode: u16) -> Self {
        match mode & 0o170000 {
            0o100000 => FileType::RegularFile,
            0o040000 => FileType::Directory,
            0o020000 => FileType::CharDevice,
            0o060000 => FileType::BlockDevice,
            0o010000 => FileType::Pipe,
            0o140000 => FileType::Socket,
            0o120000 => FileType::Symlink,
            _ => FileType::Unknown,
        }
    }
}

/// File mode (permissions)
#[derive(Debug, Clone, Copy)]
pub struct FileMode(pub u16);

impl FileMode {
    pub const READ: u16 = 0o4;
    pub const WRITE: u16 = 0o2;
    pub const EXEC: u16 = 0o1;

    pub fn new(mode: u16) -> Self {
        Self(mode)
    }

    pub fn can_read(&self) -> bool {
        (self.0 & Self::READ) != 0
    }

    pub fn can_write(&self) -> bool {
        (self.0 & Self::WRITE) != 0
    }

    pub fn can_exec(&self) -> bool {
        (self.0 & Self::EXEC) != 0
    }
}

/// File permissions
#[derive(Debug, Clone, Copy)]
pub struct FilePerms {
    pub mode: FileMode,
}

impl FilePerms {
    pub fn new(mode: u16) -> Self {
        Self {
            mode: FileMode::new(mode as u16),
        }
    }
}

/// File attributes
#[derive(Debug, Clone, Copy)]
pub struct FileAttr {
    /// File type
    pub file_type: FileType,
    /// Mode (permissions)
    pub mode: u16,
    /// Size in bytes
    pub size: u64,
    /// Device ID
    pub dev: u64,
    /// Inode number
    pub ino: u64,
    /// Link count
    pub nlink: u32,
    /// User ID
    pub uid: u32,
    /// Group ID
    pub gid: u32,
    /// Access time
    pub atime: u64,
    /// Modify time
    pub mtime: u64,
    /// Change time
    pub ctime: u64,
}

impl FileAttr {
    pub fn new() -> Self {
        Self {
            file_type: FileType::Unknown,
            mode: 0,
            size: 0,
            dev: 0,
            ino: 0,
            nlink: 0,
            uid: 0,
            gid: 0,
            atime: 0,
            mtime: 0,
            ctime: 0,
        }
    }
}

/// Inode trait - must be implemented by all file system inodes
pub trait VfsInode: Send + Sync {
    /// Get file attributes
    fn attr(&self) -> Result<FileAttr, VfsError>;

    /// Read from file
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<usize, VfsError>;

    /// Write to file
    fn write_at(&mut self, offset: u64, buf: &[u8]) -> Result<usize, VfsError>;

    /// Open the inode
    fn open(&mut self, _mode: FileMode) -> Result<(), VfsError> {
        Ok(())
    }

    /// Close the inode
    fn close(&mut self) -> Result<(), VfsError> {
        Ok(())
    }

    /// Get the file type
    fn file_type(&self) -> FileType;

    /// Check if this is a directory
    fn is_dir(&self) -> bool {
        self.file_type() == FileType::Directory
    }

    /// Get number of links
    fn nlink(&self) -> u32 {
        1
    }
}

/// File trait - represents an open file
pub trait VfsFile: Send + Sync {
    /// Read from file
    fn read(&self, buf: &mut [u8]) -> Result<usize, VfsError>;

    /// Write to file
    fn write(&self, buf: &[u8]) -> Result<usize, VfsError>;

    /// Seek in file
    fn seek(&self, offset: i64, whence: SeekWhence) -> Result<u64, VfsError>;

    /// Close the file
    fn close(self) -> Result<(), VfsError>;
}

/// Seek direction
#[derive(Debug, Clone, Copy)]
pub enum SeekWhence {
    /// Seek from start
    Set,
    /// Seek from current position
    Cur,
    /// Seek from end
    End,
}

/// Seek whence values (compatible with Linux)
pub const SEEK_SET: i64 = 0;
pub const SEEK_CUR: i64 = 1;
pub const SEEK_END: i64 = 2;

/// Directory entry
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Name
    pub name: [u8; MAX_FNAME_LEN],
    /// Name length
    pub name_len: usize,
    /// Inode number
    pub ino: u64,
    /// File type
    pub file_type: FileType,
}

impl DirEntry {
    pub fn new(name: &str, ino: u64, file_type: FileType) -> Self {
        let mut entry = Self {
            name: [0; MAX_FNAME_LEN],
            name_len: name.len(),
            ino,
            file_type,
        };
        entry.name[..name.len()].copy_from_slice(name.as_bytes());
        entry
    }
}

/// Directory iterator
pub trait VfsDirIter: Send + Sync {
    /// Move to next entry
    fn next_entry(&mut self) -> Option<Result<DirEntry, VfsError>>;

    /// Get current position
    fn position(&self) -> u64;
}

/// VFS Errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfsError {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    IsDirectory,
    NotDirectory,
    FileExists,
    ExceedsMaxSize,
    InvalidPath,
    DeviceError,
    IoError,
    NotSupported,
    BufferFull,
    ChildStillActive,
}

impl VfsError {
    pub fn to_errno(&self) -> isize {
        match self {
            VfsError::NotFound => -2,
            VfsError::PermissionDenied => -13,
            VfsError::AlreadyExists => -17,
            VfsError::IsDirectory => -21,
            VfsError::NotDirectory => -20,
            VfsError::FileExists => -17,
            VfsError::ExceedsMaxSize => -27,
            VfsError::InvalidPath => -36,
            VfsError::DeviceError => -19,
            VfsError::IoError => -5,
            VfsError::NotSupported => -95,
            VfsError::BufferFull => -28,
            VfsError::ChildStillActive => -81,
        }
    }
}

/// Filesystem trait - represents a mounted file system
pub trait VfsFilesystem: Send + Sync {
    /// Get filesystem name
    fn name(&self) -> &str;

    /// Get root inode
    fn root_inode(&self) -> &Mutex<dyn VfsInode>;

    /// Sync filesystem to disk
    fn sync(&self) -> Result<(), VfsError> {
        Ok(())
    }
}

/// Global VFS instance
static VFS: Mutex<Option<VfsInstance>> = Mutex::new(None);

/// VFS instance
pub struct VfsInstance {
    /// Next inode number
    next_ino: u64,
}

impl VfsInstance {
    pub fn new() -> Self {
        Self {
            next_ino: 1,
        }
    }

    /// Allocate a new inode number
    pub fn alloc_ino(&mut self) -> u64 {
        let ino = self.next_ino;
        self.next_ino += 1;
        ino
    }
}

impl Default for VfsInstance {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize VFS
pub fn init() {
    crate::println!("[vfs] Initializing VFS...");
    let mut vfs = VFS.lock();
    *vfs = Some(VfsInstance::new());
    crate::println!("[vfs] VFS initialized");
}

/// Get VFS instance
pub fn get_vfs() -> &'static Mutex<Option<VfsInstance>> {
    &VFS
}
