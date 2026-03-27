//! RAM File System (ramfs)
//!
//! A simple in-memory file system for TrainOS
//! Provides a basic filesystem implementation for storing files in RAM
//! Note: This implementation uses fixed-size arrays for no_std compatibility

use spin::Mutex;

/// Maximum file size (64KB)
const MAX_FILE_SIZE: usize = 64 * 1024;
/// Maximum files in ramfs
const MAX_FILES: usize = 32;
/// Maximum total data size (256KB)
const MAX_TOTAL_DATA: usize = 256 * 1024;

/// In-memory file entry (fixed-size for no_std)
struct RamFile {
    name: [u8; 32],
    name_len: u8,
    data: [u8; 4096],  // 4KB per file max
    data_len: usize,
    size: usize,
    mode: u16,
    uid: u32,
    gid: u32,
    active: bool,
}

/// RAM File System
pub struct RamFs {
    files: Mutex<[RamFile; MAX_FILES]>,
    next_ino: Mutex<u64>,
}

impl RamFs {
    /// Create a new RAM filesystem
    pub const fn new() -> Self {
        // Create initialized file array with all files inactive
        const INIT_FILE: RamFile = RamFile {
            name: [0u8; 32],
            name_len: 0,
            data: [0u8; 4096],
            data_len: 0,
            size: 0,
            mode: 0,
            uid: 0,
            gid: 0,
            active: false,
        };
        Self {
            files: Mutex::new([INIT_FILE; MAX_FILES]),
            next_ino: Mutex::new(1),
        }
    }

    /// Allocate a new inode number
    fn alloc_ino(&self) -> u64 {
        let mut ino = self.next_ino.lock();
        let result = *ino;
        *ino += 1;
        result
    }

    /// Find a file by name
    fn find_file(&self, name: &str) -> Option<usize> {
        let files = self.files.lock();
        for (i, file) in files.iter().enumerate() {
            if !file.active {
                continue;
            }
            let file_name = core::str::from_utf8(&file.name[..file.name_len as usize]).unwrap_or("");
            if file_name == name {
                return Some(i);
            }
        }
        None
    }

    /// Create a file
    pub fn create(&self, name: &str, mode: u16) -> Result<u64, RamfsError> {
        if name.len() > 31 {
            return Err(RamfsError::NameTooLong);
        }

        let mut files = self.files.lock();

        // Find empty slot
        let slot = files.iter_mut().position(|f| !f.active);

        if slot.is_none() {
            return Err(RamfsError::NoSpace);
        }

        // Check if file already exists
        for file in files.iter() {
            if !file.active {
                continue;
            }
            let file_name = core::str::from_utf8(&file.name[..file.name_len as usize]).unwrap_or("");
            if file_name == name {
                return Err(RamfsError::AlreadyExists);
            }
        }

        let ino = {
            let mut ino_lock = self.next_ino.lock();
            let result = *ino_lock;
            *ino_lock += 1;
            result
        };

        // Initialize the file slot
        let slot = slot.unwrap();
        files[slot].name = [0u8; 32];
        files[slot].name[..name.len()].copy_from_slice(name.as_bytes());
        files[slot].name_len = name.len() as u8;
        files[slot].data = [0u8; 4096];
        files[slot].data_len = 0;
        files[slot].size = 0;
        files[slot].mode = mode;
        files[slot].uid = 0;
        files[slot].gid = 0;
        files[slot].active = true;

        Ok(ino)
    }

    /// Open a file
    pub fn open(&self, name: &str) -> Result<usize, RamfsError> {
        self.find_file(name)
            .ok_or(RamfsError::NotFound)
    }

    /// Read from a file
    pub fn read(&self, name: &str, buf: &mut [u8], offset: usize) -> Result<usize, RamfsError> {
        let files = self.files.lock();

        for file in files.iter() {
            if !file.active {
                continue;
            }
            let file_name = core::str::from_utf8(&file.name[..file.name_len as usize]).unwrap_or("");
            if file_name == name {
                if offset >= file.size {
                    return Ok(0);
                }

                let remaining = file.size - offset;
                let to_read = remaining.min(buf.len()).min(file.data_len);

                buf[..to_read].copy_from_slice(&file.data[offset..offset + to_read]);
                return Ok(to_read);
            }
        }

        Err(RamfsError::NotFound)
    }

    /// Write to a file
    pub fn write(&self, name: &str, buf: &[u8], offset: usize) -> Result<usize, RamfsError> {
        let mut files = self.files.lock();

        for file in files.iter_mut() {
            if !file.active {
                continue;
            }
            let file_name = core::str::from_utf8(&file.name[..file.name_len as usize]).unwrap_or("");
            if file_name == name {
                let new_size = offset + buf.len();

                if new_size > MAX_FILE_SIZE || new_size > 4096 {
                    return Err(RamfsError::FileTooLarge);
                }

                // Extend data if needed
                if new_size > file.data_len {
                    file.data_len = new_size;
                }

                // Copy data (clamp to buffer size)
                let copy_len = buf.len().min(4096 - offset);
                file.data[offset..offset + copy_len].copy_from_slice(&buf[..copy_len]);
                file.size = new_size.max(file.size);

                return Ok(copy_len);
            }
        }

        Err(RamfsError::NotFound)
    }

    /// Delete a file
    pub fn unlink(&self, name: &str) -> Result<(), RamfsError> {
        let mut files = self.files.lock();

        for file in files.iter_mut() {
            if !file.active {
                continue;
            }
            let file_name = core::str::from_utf8(&file.name[..file.name_len as usize]).unwrap_or("");
            if file_name == name {
                file.active = false;
                file.name = [0u8; 32];
                file.name_len = 0;
                file.data = [0u8; 4096];
                file.data_len = 0;
                file.size = 0;
                return Ok(());
            }
        }

        Err(RamfsError::NotFound)
    }

    /// Get file size
    pub fn size(&self, name: &str) -> Result<usize, RamfsError> {
        let files = self.files.lock();

        for file in files.iter() {
            if !file.active {
                continue;
            }
            let file_name = core::str::from_utf8(&file.name[..file.name_len as usize]).unwrap_or("");
            if file_name == name {
                return Ok(file.size);
            }
        }

        Err(RamfsError::NotFound)
    }

    /// List all files (returns comma-separated names)
    pub fn list(&self) -> [u8; 512] {
        let mut result = [0u8; 512];
        let files = self.files.lock();
        let mut pos = 0;

        for file in files.iter() {
            if !file.active {
                continue;
            }
            for &c in &file.name[..file.name_len as usize] {
                if pos < 511 {
                    result[pos] = c;
                    pos += 1;
                }
            }
            if pos < 511 {
                result[pos] = b',';
                pos += 1;
            }
        }

        result
    }

    /// Get number of files
    pub fn file_count(&self) -> usize {
        self.files.lock().iter().filter(|f| f.active).count()
    }

    /// Check if file exists
    pub fn exists(&self, name: &str) -> bool {
        self.find_file(name).is_some()
    }
}

/// RAM filesystem error
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RamfsError {
    NotFound,
    AlreadyExists,
    NameTooLong,
    NoSpace,
    FileTooLarge,
    IsDirectory,
    NotDirectory,
}

impl RamfsError {
    pub fn to_errno(&self) -> i32 {
        match self {
            RamfsError::NotFound => -2,      // ENOENT
            RamfsError::AlreadyExists => -17, // EEXIST
            RamfsError::NameTooLong => -36,  // ENAMETOOLONG
            RamfsError::NoSpace => -28,       // ENOSPC
            RamfsError::FileTooLarge => -27,  // EFBIG
            RamfsError::IsDirectory => -21,   // EISDIR
            RamfsError::NotDirectory => -20,  // ENOTDIR
        }
    }
}

/// Global RAM filesystem instance - lazy initialized
static RAM_FS: RamFs = RamFs::new();

/// Get the global RAM filesystem
pub fn get_ramfs() -> &'static RamFs {
    &RAM_FS
}

/// Initialize RAM filesystem
pub fn init() {
    crate::println!("[ramfs] Initializing RAM filesystem...");
    crate::println!("[ramfs] Files: 32, Size per file: 4KB");

    // Create some default device files
    let _ = RAM_FS.create("/dev/null", 0o666);
    let _ = RAM_FS.create("/dev/zero", 0o666);
    let _ = RAM_FS.create("/dev/console", 0o666);

    crate::println!("[ramfs] OK");
}
