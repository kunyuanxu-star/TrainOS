//! Syscall file system operations
//!
//! Implements file-related syscalls

use spin::Mutex;

/// File descriptor table entry
#[derive(Debug, Clone, Copy)]
pub struct FileDescriptor {
    /// File type
    pub type_: FileType,
    /// Flags
    pub flags: usize,
    /// Position
    pub position: usize,
    /// Inode number (for files)
    pub inode: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileType {
    Device,
    Pipe,
    Socket,
    File,
    Directory,
}

/// Maximum number of file descriptors per process
const MAX_FD: usize = 1024;

/// Global file descriptor table (simplified - per-process in real OS)
static FD_TABLE: Mutex<[Option<FileDescriptor>; MAX_FD]> = Mutex::new([None; MAX_FD]);

/// Open flags (simplified)
pub const O_RDONLY: usize = 0;
pub const O_WRONLY: usize = 1;
pub const O_RDWR: usize = 2;
pub const O_CREAT: usize = 0x40;
pub const O_EXCL: usize = 0x80;
pub const O_NOCTTY: usize = 0x100;
pub const O_TRUNC: usize = 0x200;
pub const O_APPEND: usize = 0x400;
pub const O_NONBLOCK: usize = 0x800;

/// File mode
pub const S_IFMT: usize = 0o170000;
pub const S_IFREG: usize = 0o100000;
pub const S_IFDIR: usize = 0o040000;
pub const S_IFCHR: usize = 0o020000;
pub const S_IFBLK: usize = 0o060000;
pub const S_IFIFO: usize = 0o010000;
pub const S_IFSOCK: usize = 0o140000;

/// Open syscall
pub fn sys_openat(_dirfd: usize, _pathname: usize, _flags: usize, _mode: usize) -> isize {
    // In a real implementation:
    // 1. Parse the pathname
    // 2. Look up the inode
    // 3. Create a file descriptor
    // 4. Return fd number

    // For now, return a dummy fd
    crate::println!("[openat] Called");
    3  // First user file descriptor after stdin/stdout/stderr
}

/// Close file descriptor
pub fn sys_close(fd: usize) -> isize {
    if fd < 3 {
        // Don't close stdin/stdout/stderr
        return 0;
    }

    let mut table = FD_TABLE.lock();
    if fd < MAX_FD {
        table[fd] = None;
    }
    0
}

/// Allocate a file descriptor
pub fn alloc_fd() -> Option<usize> {
    let mut table = FD_TABLE.lock();
    for i in 3..MAX_FD {
        if table[i].is_none() {
            table[i] = Some(FileDescriptor {
                type_: FileType::File,
                flags: 0,
                position: 0,
                inode: 0,
            });
            return Some(i);
        }
    }
    None
}

/// Mkdirat syscall
pub fn sys_mkdirat(_dirfd: usize, _pathname: usize, _mode: usize) -> isize {
    crate::println!("[mkdirat] Called");
    0
}

/// Unlinkat syscall
pub fn sys_unlinkat(_dirfd: usize, _pathname: usize, _flags: usize) -> isize {
    crate::println!("[unlinkat] Called");
    0
}

/// Readlinkat syscall
pub fn sys_readlinkat(_dirfd: usize, _path: usize, _buf: usize, _bufsiz: usize) -> isize {
    crate::println!("[readlinkat] Called");
    0
}
