//! File Descriptor Table
//!
//! Manages open file descriptors for processes

use spin::Mutex;

/// Maximum number of file descriptors per process
pub const MAX_FD_PER_PROCESS: usize = 1024;

/// Maximum number of open files in system
pub const MAX_OPEN_FILES: usize = 4096;

/// File descriptor entry
#[derive(Debug, Clone, Copy)]
pub enum FileKind {
    /// Standard input/output/error (console)
    Console,
    /// Regular file (in-memory)
    MemoryFile,
    /// Pipe
    Pipe,
    /// Socket
    Socket,
    /// Device
    Device,
}

/// File descriptor flags
#[derive(Debug, Clone, Copy)]
pub struct FdFlags {
    pub non_blocking: bool,
    pub cloexec: bool,
}

/// Represents an open file description
#[derive(Debug, Clone, Copy)]
pub struct FileDesc {
    /// File kind
    pub kind: FileKind,
    /// Flags
    pub flags: FdFlags,
    /// Current offset (for seekable files)
    pub offset: usize,
    /// Reference to underlying file data (index)
    pub data_idx: usize,
}

impl Default for FdFlags {
    fn default() -> Self {
        Self {
            non_blocking: false,
            cloexec: false,
        }
    }
}

impl FileDesc {
    pub fn new(kind: FileKind) -> Self {
        Self {
            kind,
            flags: FdFlags::default(),
            offset: 0,
            data_idx: 0,
        }
    }
}

/// File descriptor table type
type FdTableType = Option<[Option<FileDesc>; MAX_FD_PER_PROCESS]>;

/// Global file descriptor table for init process
static INIT_FD_TABLE: Mutex<FdTableType> = Mutex::new(None);

/// Initialize the fd table for init process
pub fn init() {
    let mut table = INIT_FD_TABLE.lock();
    if table.is_none() {
        let mut entries = [None; MAX_FD_PER_PROCESS];
        // Reserve fd 0, 1, 2 for stdin/stdout/stderr
        entries[0] = Some(FileDesc::new(FileKind::Console));
        entries[1] = Some(FileDesc::new(FileKind::Console));
        entries[2] = Some(FileDesc::new(FileKind::Console));
        *table = Some(entries);
    }
    crate::println!("[syscall] fd table initialized");
}

/// Allocate a new file descriptor
pub fn alloc_fd(kind: FileKind) -> Option<usize> {
    let mut table = INIT_FD_TABLE.lock();
    if let Some(ref mut entries) = *table {
        // First try to find an empty slot
        for i in 3..MAX_FD_PER_PROCESS {
            if entries[i].is_none() {
                entries[i] = Some(FileDesc::new(kind));
                return Some(i);
            }
        }
    }
    None
}

/// Free a file descriptor
pub fn free_fd(fd: usize) -> bool {
    if fd < 3 {
        // Don't free stdin/stdout/stderr
        return false;
    }
    let mut table = INIT_FD_TABLE.lock();
    if let Some(ref mut entries) = *table {
        if fd < MAX_FD_PER_PROCESS && entries[fd].is_some() {
            entries[fd] = None;
            return true;
        }
    }
    false
}

/// Get file descriptor entry
pub fn get_fd(fd: usize) -> Option<FileDesc> {
    let table = INIT_FD_TABLE.lock();
    if let Some(ref entries) = *table {
        if fd < MAX_FD_PER_PROCESS {
            if let Some(fdesc) = entries[fd] {
                return Some(fdesc);
            }
        }
    }
    None
}

/// Get mutable fd entry
pub fn get_fd_mut(fd: usize) -> Option<&'static mut FileDesc> {
    // This is unsafe because we're returning a mutable reference
    // Only use when you know only one thread will access it
    let mut table = INIT_FD_TABLE.lock();
    if let Some(ref mut entries) = *table {
        if fd < MAX_FD_PER_PROCESS {
            if entries[fd].is_some() {
                // SAFETY: We have exclusive access via the mutex
                // and we've verified the fd is valid
                let opt_ptr = unsafe { entries.as_mut_ptr().add(fd) } as *mut Option<FileDesc>;
                let file_desc_ptr = unsafe { &mut *opt_ptr };
                if let Some(ref mut fd) = file_desc_ptr {
                    return Some(fd);
                }
            }
        }
    }
    None
}

/// Check if fd is valid and open
pub fn is_fd_open(fd: usize) -> bool {
    get_fd(fd).is_some()
}
