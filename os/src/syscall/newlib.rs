//! Newlib Syscall Interface
//!
//! Provides the syscall layer that newlib/c library calls into
//! This is the bridge between the C library and our kernel

/// Exit a process (newlib syscall)
#[no_mangle]
pub extern "C" fn _exit(_code: i32) -> ! {
    crate::println!("[newlib] _exit called");
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// Open a file (newlib syscall)
/// Returns file descriptor or -1 on error
#[no_mangle]
pub extern "C" fn _open(path: *const u8, _flags: i32, _mode: i32) -> i32 {
    let mut path_buf = [0u8; 256];
    let mut i = 0;
    unsafe {
        loop {
            let c = *path.add(i);
            path_buf[i] = c;
            if c == 0 || i >= 255 {
                break;
            }
            i += 1;
        }
    }
    crate::println!("[newlib] _open called");
    // For now, return error - filesystem not ready
    -1
}

/// Close a file (newlib syscall)
#[no_mangle]
pub extern "C" fn _close(fd: i32) -> i32 {
    if fd < 3 {
        0  // stdin/stdout/stderr are always open
    } else {
        0  // For now
    }
}

/// Read from file (newlib syscall)
/// Returns number of bytes read or -1 on error
#[no_mangle]
pub extern "C" fn _read(fd: i32, buf: *mut u8, count: usize) -> i32 {
    match fd {
        0 => {
            // stdin - for now return no input available
            0
        }
        1 | 2 => {
            // stdout/stderr - can't read
            -1
        }
        _ => {
            // Other files
            -1
        }
    }
}

/// Write to file (newlib syscall)
/// Returns number of bytes written or -1 on error
#[no_mangle]
pub extern "C" fn _write(fd: i32, buf: *const u8, count: usize) -> i32 {
    match fd {
        1 | 2 => {
            // stdout/stderr
            let mut written: isize = 0;
            let mut ptr = buf;
            while written < count as isize {
                let c = unsafe { *ptr };
                crate::console::sbi_console_putchar(c as usize);
                if c == b'\n' {
                    crate::console::sbi_console_putchar(b'\r' as usize);
                }
                ptr = unsafe { ptr.add(1) };
                written += 1;
            }
            written as i32
        }
        _ => -1
    }
}

/// Seek in file (newlib syscall)
#[no_mangle]
pub extern "C" fn _lseek(fd: i32, offset: isize, whence: i32) -> isize {
    // For now, don't support seeking
    0
}

/// Get file status (newlib syscall)
#[no_mangle]
pub extern "C" fn _fstat(fd: i32, stat_buf: *mut StatBuf) -> i32 {
    // Return dummy stat - all files are character devices
    if fd < 0 || fd > 2 {
        return -1;
    }
    unsafe {
        (*stat_buf).st_mode = 0x2000;  // S_IFCHR
        (*stat_buf).st_uid = 0;
        (*stat_buf).st_gid = 0;
        (*stat_buf).st_size = 0;
    }
    0
}

/// Stat - get file status by path (newlib syscall)
#[no_mangle]
pub extern "C" fn _stat(path: *const u8, stat_buf: *mut StatBuf) -> i32 {
    unsafe {
        (*stat_buf).st_mode = 0x2000;
        (*stat_buf).st_size = 0;
    }
    0
}

/// Link (newlib syscall) - not supported
#[no_mangle]
pub extern "C" fn _link(oldpath: *const u8, newpath: *const u8) -> i32 {
    -1
}

/// Unlink (newlib syscall) - not supported
#[no_mangle]
pub extern "C" fn _unlink(path: *const u8) -> i32 {
    -1
}

/// Get time of day (newlib syscall)
#[no_mangle]
pub extern "C" fn _gettimeofday(timeval: *mut TimeVal, _tz: *mut u8) -> i32 {
    unsafe {
        (*timeval).tv_sec = 0;
        (*timeval).tv_usec = 0;
    }
    0
}

/// Times - get process times (newlib syscall)
#[no_mangle]
pub extern "C" fn _times(tbuf: *mut TMS) -> i32 {
    // Return dummy times
    unsafe {
        (*tbuf).tms_utime = 0;
        (*tbuf).tms_stime = 0;
        (*tbuf).tms_cutime = 0;
        (*tbuf).tms_cstime = 0;
    }
    0
}

/// Sbrk - memory allocation (newlib syscall)
/// This is critical for malloc and the heap
static mut HEAP_END: usize = 0x81000000;  // Start of heap
static mut HEAP_CURRENT: usize = 0x81000000;

#[no_mangle]
pub extern "C" fn _sbrk(increment: isize) -> *mut u8 {
    unsafe {
        if increment == 0 {
            return HEAP_CURRENT as *mut u8;
        }
        if HEAP_CURRENT + (increment as usize) > HEAP_END + 0x10000000 {
            // Out of memory - return error
            return core::ptr::null_mut();
        }
        let old = HEAP_CURRENT;
        HEAP_CURRENT += increment as usize;
        old as *mut u8
    }
}

/// Get process ID (newlib syscall)
#[no_mangle]
pub extern "C" fn _getpid() -> i32 {
    1  // Init process
}

/// Kill - send signal (newlib syscall)
#[no_mangle]
pub extern "C" fn _kill(_pid: i32, _sig: i32) -> i32 {
    -1  // Not supported
}

/// Get current working directory (newlib syscall)
#[no_mangle]
pub extern "C" fn _getcwd(buf: *mut u8, size: usize) -> i32 {
    if size < 2 {
        return -1;
    }
    unsafe {
        *buf = b'/';
        *buf.add(1) = 0;
    }
    1
}

/// Chdir (newlib syscall)
#[no_mangle]
pub extern "C" fn _chdir(_path: *const u8) -> i32 {
    0  // Always succeeds for now
}

/// Malloc (newlib uses sbrk, but some implementations have separate malloc)
#[no_mangle]
pub extern "C" fn _malloc(_size: usize) -> *mut u8 {
    // Use sbrk for malloc
    _sbrk(0) as *mut u8
}

/// Free (newlib malloc uses sbrk, so free is often a no-op)
#[no_mangle]
pub extern "C" fn _free(_ptr: *mut u8) {
    // No-op for now
}

/// Realloc
#[no_mangle]
pub extern "C" fn _realloc(ptr: *mut u8, _size: usize) -> *mut u8 {
    // Simple - just return same pointer for now
    ptr
}

/// File status structure
#[repr(C)]
pub struct StatBuf {
    pub st_mode: u32,      // File mode
    pub st_ino: u32,       // File serial number
    pub st_dev: u32,        // Device ID
    pub st_nlink: u32,     // Number of links
    pub st_uid: u32,       // User ID of owner
    pub st_gid: u32,       // Group ID of owner
    pub st_size: i64,       // Size of file
    pub st_atime: i64,      // Access time
    pub st_mtime: i64,      // Modification time
    pub st_ctime: i64,      // Status change time
    pub st_blksize: i32,    // Block size
    pub st_blocks: i32,     // Number of blocks
}

/// Timeval structure
#[repr(C)]
pub struct TimeVal {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

/// TMS structure for times()
#[repr(C)]
pub struct TMS {
    pub tms_utime: i64,     // User time
    pub tms_stime: i64,     // System time
    pub tms_cutime: i64,    // Children's user time
    pub tms_cstime: i64,    // Children's system time
}

/// Environment variable - getenv uses sbrk
#[no_mangle]
pub extern "C" fn _getenv(_name: *const u8) -> *mut u8 {
    core::ptr::null_mut()
}

/// Putchar for newlib's stdio
#[no_mangle]
pub extern "C" fn _putchar(c: i32) -> i32 {
    crate::console::sbi_console_putchar(c as usize);
    c as i32
}

/// Getchar for newlib's stdio
#[no_mangle]
pub extern "C" fn _getchar() -> i32 {
    // For now, return no input
    -1
}

/// Isatty - check if fd is a terminal
#[no_mangle]
pub extern "C" fn _isatty(_fd: i32) -> i32 {
    1  // Assume all fds are terminals for now
}

/// System - execute shell command
#[no_mangle]
pub extern "C" fn _system(_command: *const u8) -> i32 {
    -1  // Not implemented
}
