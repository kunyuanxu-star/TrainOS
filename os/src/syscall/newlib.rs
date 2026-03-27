//! Newlib Syscall Interface
//!
//! Provides the syscall layer that newlib/c library calls into
//! This is the bridge between the C library and our kernel

use spin::Mutex;

/// Simple file descriptor table for newlib
const MAX_FD: usize = 16;

#[derive(Copy, Clone)]
enum FileKind {
    None,
    Console,
    Device,
    File,
}

#[derive(Copy, Clone)]
struct FdEntry {
    kind: FileKind,
    name: [u8; 32],
}

static FD_TABLE: Mutex<[FdEntry; MAX_FD]> = Mutex::new({
    let mut arr = [FdEntry { kind: FileKind::None, name: [0; 32] }; MAX_FD];
    // Initialize stdin/stdout/stderr as console
    arr[0] = FdEntry { kind: FileKind::Console, name: [0; 32] };
    arr[1] = FdEntry { kind: FileKind::Console, name: [0; 32] };
    arr[2] = FdEntry { kind: FileKind::Console, name: [0; 32] };
    arr
});

/// Allocate a file descriptor
fn alloc_fd(kind: FileKind) -> Option<usize> {
    let mut table = FD_TABLE.lock();
    for i in 3..MAX_FD {
        if matches!(table[i].kind, FileKind::None) {
            table[i].kind = kind;
            return Some(i);
        }
    }
    None
}

/// Free a file descriptor
fn free_fd(fd: usize) {
    if fd < MAX_FD {
        let mut table = FD_TABLE.lock();
        table[fd].kind = FileKind::None;
    }
}

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
pub extern "C" fn _open(path: *const u8, flags: i32, _mode: i32) -> i32 {
    // Read path string
    let mut path_buf = [0u8; 256];
    let mut i = 0;
    unsafe {
        loop {
            if i >= 255 { break; }
            let c = *path.add(i);
            path_buf[i] = c;
            if c == 0 { break; }
            i += 1;
        }
    }

    let path_str = core::str::from_utf8(&path_buf[..i]).unwrap_or("");

    crate::println!("[newlib] _open called");

    // For now, only allow opening console devices
    if path_str.starts_with("/dev/console") || path_str.starts_with("/dev/tty") {
        if let Some(fd) = alloc_fd(FileKind::Console) {
            return fd as i32;
        }
    }

    // Check if it's a character device request
    if flags & 0o100000 != 0 {
        // O_TMPFILE or similar - not supported
    }

    // For now, fail all other opens
    -1
}

/// Close a file (newlib syscall)
#[no_mangle]
pub extern "C" fn _close(fd: i32) -> i32 {
    if fd < 0 || fd as usize >= MAX_FD {
        return -1;
    }

    // Don't close stdin/stdout/stderr
    if fd < 3 {
        return 0;
    }

    free_fd(fd as usize);
    0
}

/// Read from file (newlib syscall)
/// Returns number of bytes read or -1 on error
#[no_mangle]
pub extern "C" fn _read(fd: i32, buf: *mut u8, count: usize) -> i32 {
    if fd < 0 || fd as usize >= MAX_FD {
        return -1;
    }

    match fd {
        0 => {
            // stdin - read from console (simplified - no actual input yet)
            // In real implementation, this would read from keyboard buffer
            0
        }
        1 | 2 => {
            // stdout/stderr - can't read
            -1
        }
        _ => {
            // For other files, check if it's a device
            let table = FD_TABLE.lock();
            match table[fd as usize].kind {
                FileKind::Console => 0,
                _ => -1,
            }
        }
    }
}

/// Write to file (newlib syscall)
/// Returns number of bytes written or -1 on error
#[no_mangle]
pub extern "C" fn _write(fd: i32, buf: *const u8, count: usize) -> i32 {
    if fd < 0 || fd as usize >= MAX_FD {
        return -1;
    }

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
        _ => {
            // Check if it's a console device
            let table = FD_TABLE.lock();
            match table[fd as usize].kind {
                FileKind::Console => count as i32,
                _ => -1,
            }
        }
    }
}

/// Seek in file (newlib syscall)
#[no_mangle]
pub extern "C" fn _lseek(fd: i32, offset: isize, whence: i32) -> isize {
    // Most devices don't support seeking
    let fd_usize = fd as usize;
    if fd >= 3 && fd_usize < MAX_FD {
        let table = FD_TABLE.lock();
        match table[fd_usize].kind {
            FileKind::File => 0,  // Would seek in file
            _ => -1,
        }
    } else {
        0
    }
}

/// Get file status (newlib syscall)
#[no_mangle]
pub extern "C" fn _fstat(fd: i32, stat_buf: *mut StatBuf) -> i32 {
    if fd < 0 || fd > 2 {
        return -1;
    }

    unsafe {
        (*stat_buf).st_mode = 0x2000;  // S_IFCHR - character device
        (*stat_buf).st_uid = 0;
        (*stat_buf).st_gid = 0;
        (*stat_buf).st_size = 0;
        (*stat_buf).st_blocks = 0;
        (*stat_buf).st_blksize = 512;
    }
    0
}

/// Stat - get file status by path (newlib syscall)
#[no_mangle]
pub extern "C" fn _stat(path: *const u8, stat_buf: *mut StatBuf) -> i32 {
    unsafe {
        (*stat_buf).st_mode = 0x2000;  // S_IFCHR
        (*stat_buf).st_size = 0;
    }
    0
}

/// Link (newlib syscall) - not supported
#[no_mangle]
pub extern "C" fn _link(_oldpath: *const u8, _newpath: *const u8) -> i32 {
    -1
}

/// Unlink (newlib syscall) - not supported
#[no_mangle]
pub extern "C" fn _unlink(_path: *const u8) -> i32 {
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

        let new_heap = HEAP_CURRENT + (increment as usize);

        // Limit heap to 256MB region
        if new_heap > HEAP_END + 0x10000000 {
            crate::println!("[newlib] _sbrk: out of memory");
            return core::ptr::null_mut();
        }

        let old = HEAP_CURRENT;
        HEAP_CURRENT = new_heap;
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
    _sbrk(_size as isize) as *mut u8
}

/// Free (newlib malloc uses sbrk, so free is often a no-op)
#[no_mangle]
pub extern "C" fn _free(_ptr: *mut u8) {
    // No-op for now
}

/// Realloc
#[no_mangle]
pub extern "C" fn _realloc(ptr: *mut u8, _size: usize) -> *mut u8 {
    ptr  // Simple - just return same pointer for now
}

/// File status structure
#[repr(C)]
pub struct StatBuf {
    pub st_mode: u32,
    pub st_ino: u32,
    pub st_dev: u32,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_size: i64,
    pub st_atime: i64,
    pub st_mtime: i64,
    pub st_ctime: i64,
    pub st_blksize: i32,
    pub st_blocks: i32,
}

/// Timeval structure
#[repr(C)]
pub struct TimeVal {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

/// Timespec structure (for clock_gettime)
#[repr(C)]
pub struct Timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

/// TMS structure for times()
#[repr(C)]
pub struct TMS {
    pub tms_utime: i64,
    pub tms_stime: i64,
    pub tms_cutime: i64,
    pub tms_cstime: i64,
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
    if c == '\n' as i32 {
        crate::console::sbi_console_putchar('\r' as usize);
    }
    c
}

/// Getchar for newlib's stdio
#[no_mangle]
pub extern "C" fn _getchar() -> i32 {
    // For now, return no input - would need keyboard driver
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

/// Fcntl - file control
#[no_mangle]
pub extern "C" fn _fcntl(_fd: i32, _cmd: i32, _arg: i32) -> i32 {
    0  // Minimal implementation
}

/// Fork (newlib syscall)
#[no_mangle]
pub extern "C" fn _fork() -> i32 {
    // Process forking not fully implemented
    -1
}

/// Execve (newlib syscall)
#[no_mangle]
pub extern "C" fn _execve(path: *const u8, argv: *const *const u8, _envp: *const *const u8) -> i32 {
    // Read path
    let mut path_buf = [0u8; 256];
    let mut i = 0;
    unsafe {
        loop {
            if i >= 255 { break; }
            let c = *path.add(i);
            path_buf[i] = c;
            if c == 0 { break; }
            i += 1;
        }
    }

    let path_str = core::str::from_utf8(&path_buf[..i]).unwrap_or("");
    crate::println!("[newlib] _execve called");

    // Would load and execute program here
    -1
}

/// Wait (newlib syscall)
#[no_mangle]
pub extern "C" fn _wait(_status: *mut i32) -> i32 {
    -1
}

/// Isatty implementation
#[no_mangle]
pub extern "C" fn isatty(_fd: i32) -> i32 {
    1
}

/// Getpagesize
#[no_mangle]
pub extern "C" fn getpagesize() -> i32 {
    4096
}

/// Sysconf - get configuration value
#[no_mangle]
pub extern "C" fn _sysconf(name: i32) -> i32 {
    match name {
        0 => 4096,   // _SC_PAGESIZE
        1 => 1024,   // _SC_NPROCESSORS_CONF
        2 => 1024,   // _SC_NPROCESSORS_ONLN
        _ => -1,
    }
}

/// Clock_getres (newlib syscall)
#[no_mangle]
pub extern "C" fn _clock_getres(_clock_id: i32, _tp: *mut TimeVal) -> i32 {
    0
}

/// Clock_gettime (newlib syscall)
#[no_mangle]
pub extern "C" fn _clock_gettime(_clock_id: i32, tp: *mut Timespec) -> i32 {
    unsafe {
        (*tp).tv_sec = 0;
        (*tp).tv_nsec = 0;
    }
    0
}

/// Getuid (newlib syscall)
#[no_mangle]
pub extern "C" fn _getuid() -> u32 {
    0
}

/// Geteuid (newlib syscall)
#[no_mangle]
pub extern "C" fn _geteuid() -> u32 {
    0
}

/// Getgid (newlib syscall)
#[no_mangle]
pub extern "C" fn _getgid() -> u32 {
    0
}

/// Getegid (newlib syscall)
#[no_mangle]
pub extern "C" fn _getegid() -> u32 {
    0
}

/// Access (newlib syscall)
#[no_mangle]
pub extern "C" fn _access(_path: *const u8, _mode: i32) -> i32 {
    0  // All files accessible for now
}

/// Pipe (newlib syscall)
#[no_mangle]
pub extern "C" fn _pipe(_fds: *mut i32) -> i32 {
    -1  // Not implemented yet
}

/// Mkdir (newlib syscall)
#[no_mangle]
pub extern "C" fn _mkdir(_path: *const u8, _mode: u32) -> i32 {
    -1  // Not implemented
}

/// Rename (newlib syscall)
#[no_mangle]
pub extern "C" fn _rename(_old: *const u8, _new: *const u8) -> i32 {
    -1  // Not implemented
}

/// Time (newlib syscall)
#[no_mangle]
pub extern "C" fn _time(_t: *mut i64) -> i32 {
    0
}

/// Signal (newlib syscall) - simplified
#[no_mangle]
pub extern "C" fn _signal(_sig: i32, _handler: *const u8) -> i32 {
    0
}

/// Writev (newlib syscall)
#[no_mangle]
pub extern "C" fn _writev(fd: i32, iov: *const IoVec, iovcnt: i32) -> i32 {
    let mut total = 0;
    for i in 0..iovcnt {
        unsafe {
            let iov_ptr = iov.add(i as usize);
            let buf = (*iov_ptr).iov_base as *const u8;
            let len = (*iov_ptr).iov_len;
            let written = _write(fd, buf, len);
            if written < 0 {
                return if total == 0 { -1 } else { total };
            }
            total += written;
        }
    }
    total
}

/// Readv (newlib syscall)
#[no_mangle]
pub extern "C" fn _readv(fd: i32, iov: *const IoVec, iovcnt: i32) -> i32 {
    let mut total = 0;
    for i in 0..iovcnt {
        unsafe {
            let iov_ptr = iov.add(i as usize);
            let buf = (*iov_ptr).iov_base as *mut u8;
            let len = (*iov_ptr).iov_len;
            let nread = _read(fd, buf, len);
            if nread < 0 {
                return if total == 0 { -1 } else { total };
            }
            total += nread;
        }
    }
    total
}

/// IoVec structure for readv/writev
#[repr(C)]
pub struct IoVec {
    pub iov_base: *mut u8,
    pub iov_len: usize,
}

/// Sbrk wrapper that uses the internal HEAP_CURRENT
#[no_mangle]
pub extern "C" fn sbrk(increment: intptr_t) -> *mut u8 {
    _sbrk(increment)
}

/// Intptr type for sbrk
type intptr_t = isize;

/// Mmap (newlib syscall) - simplified
#[no_mangle]
pub extern "C" fn _mmap(_addr: *mut u8, _len: usize, _prot: i32, _flags: i32, _fd: i32, _offset: isize) -> *mut u8 {
    // Memory mapping not fully implemented
    // Fall back to sbrk behavior
    _sbrk(_len as isize)
}

/// Munmap (newlib syscall)
#[no_mangle]
pub extern "C" fn _munmap(_addr: *mut u8, _len: usize) -> i32 {
    0  // No-op for now
}

/// Mprotect (newlib syscall)
#[no_mangle]
pub extern "C" fn _mprotect(_addr: *mut u8, _len: usize, _prot: i32) -> i32 {
    0  // No-op for now
}
