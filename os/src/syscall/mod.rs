//! System call module
//!
//! Implements Linux-compatible system call interface

pub mod task;
pub mod memory;
pub mod fs;

use core::ops::Add;
use spin::Mutex;

/// Linux syscall numbers (RISC-V Linux compatible)
/// See /usr/include/asm-generic/unistd.h
pub mod nr {
    pub const EXIT: usize = 93;
    pub const EXIT_GROUP: usize = 94;
    pub const READ: usize = 63;
    pub const WRITE: usize = 64;
    pub const OPENAT: usize = 56;
    pub const CLOSE: usize = 57;
    pub const PIPE2: usize = 59;
    pub const GETPID: usize = 172;
    pub const GETTID: usize = 178;
    pub const GETPPID: usize = 173;
    pub const BRK: usize = 214;
    pub const MUNMAP: usize = 215;
    pub const MMAP: usize = 222;
    pub const MPROTECT: usize = 226;
    pub const CLONE: usize = 220;
    pub const EXECVE: usize = 221;
    pub const WAIT4: usize = 260;
    pub const WAITID: usize = 287;
    pub const SCHED_YIELD: usize = 124;
    pub const NANOSLEEP: usize = 101;
    pub const FUTEX: usize = 98;
    pub const SET_TID_ADDRESS: usize = 96;
    pub const MLOCK: usize = 228;
    pub const MUNLOCK: usize = 229;
    pub const DUP: usize = 23;
    pub const DUP3: usize = 24;
    pub const READLINKAT: usize = 78;
    pub const UNLINKAT: usize = 35;
    pub const MKDIRAT: usize = 34;
    pub const SYSINFO: usize = 179;
}

/// Current process ID
static CURRENT_PID: Mutex<usize> = Mutex::new(1);

/// Get and increment next PID
pub fn alloc_pid() -> usize {
    let mut pid = CURRENT_PID.lock();
    let id = *pid;
    *pid += 1;
    id
}

/// Read register a0
fn get_arg0() -> usize {
    let mut val: usize;
    unsafe {
        core::arch::asm!("mv {}, a0", out(reg) val);
    }
    val
}

/// Read register a1
fn get_arg1() -> usize {
    let mut val: usize;
    unsafe {
        core::arch::asm!("mv {}, a1", out(reg) val);
    }
    val
}

/// Read register a2
fn get_arg2() -> usize {
    let mut val: usize;
    unsafe {
        core::arch::asm!("mv {}, a2", out(reg) val);
    }
    val
}

/// Read register a3
fn get_arg3() -> usize {
    let mut val: usize;
    unsafe {
        core::arch::asm!("mv {}, a3", out(reg) val);
    }
    val
}

/// Read register a4
fn get_arg4() -> usize {
    let mut val: usize;
    unsafe {
        core::arch::asm!("mv {}, a4", out(reg) val);
    }
    val
}

/// Read register a5
fn get_arg5() -> usize {
    let mut val: usize;
    unsafe {
        core::arch::asm!("mv {}, a5", out(reg) val);
    }
    val
}

/// Set return value in a0
fn set_ret(val: usize) {
    unsafe {
        core::arch::asm!("mv a0, {}", in(reg) val);
    }
}

/// Handle a system call
#[no_mangle]
pub extern "C" fn do_syscall() {
    let syscall_id: usize;
    unsafe {
        core::arch::asm!("mv {}, a7", out(reg) syscall_id);
    }

    let result = match syscall_id {
        // File operations (Linux numbers)
        63 => sys_read(get_arg0(), get_arg1(), get_arg2()),     // read
        64 => sys_write(get_arg0(), get_arg1(), get_arg2()),    // write
        23 => sys_dup(get_arg0()),                              // dup
        24 => sys_dup3(get_arg0(), get_arg1(), get_arg2()),    // dup3
        57 => sys_close(get_arg0()),                            // close
        59 => sys_pipe2(get_arg0(), get_arg1()),                // pipe2
        66 => sys_readv(get_arg0(), get_arg1(), get_arg2()),     // readv
        67 => sys_writev(get_arg0(), get_arg1(), get_arg2()),   // writev

        // Process management
        172 => sys_getpid(),                                    // getpid
        173 => sys_getppid(),                                   // getppid
        178 => sys_gettid(),                                    // gettid
        96 => sys_set_tid_address(get_arg0()),                  // set_tid_address
        93 => sys_exit(get_arg0()),                             // exit
        94 => sys_exit_group(get_arg0()),                       // exit_group
        260 => sys_wait4(get_arg0(), get_arg1(), get_arg2(), get_arg3()), // wait4
        287 => sys_waitid(get_arg0(), get_arg1(), get_arg2(), get_arg3(), get_arg4()), // waitid

        // Scheduling
        101 => sys_nanosleep(get_arg0(), get_arg1()),           // nanosleep
        124 => sys_sched_yield(),                              // sched_yield
        98 => sys_futex(get_arg0(), get_arg1(), get_arg2(), get_arg3(), get_arg4(), get_arg5()), // futex

        // Memory management
        222 => sys_mmap(get_arg0(), get_arg1(), get_arg2(), get_arg3(), get_arg4(), get_arg5()), // mmap
        215 => sys_munmap(get_arg0(), get_arg1()),             // munmap
        226 => sys_mprotect(get_arg0(), get_arg1(), get_arg2()), // mprotect
        214 => sys_brk(get_arg0()),                           // brk
        228 => sys_mlock(get_arg0(), get_arg1()),              // mlock
        229 => sys_munlock(get_arg0(), get_arg1()),            // munlock

        // Info
        179 => sys_sysinfo(),                                  // sysinfo

        // Process creation
        220 => sys_clone(get_arg0(), get_arg1(), get_arg2(), get_arg3()), // clone
        221 => sys_execve(get_arg0(), get_arg1(), get_arg2()), // execve

        // File operations (TrainOS custom)
        1000 => sys_openat(get_arg0(), get_arg1(), get_arg2(), get_arg3()), // openat
        1001 => sys_mkdirat(get_arg0(), get_arg1(), get_arg2()), // mkdirat
        1002 => sys_unlinkat(get_arg0(), get_arg1(), get_arg2()), // unlinkat

        // Signal handling
        129 => sys_sigaction(get_arg0(), get_arg1(), get_arg2()), // rt_sigaction
        130 => sys_sigaction(get_arg0(), get_arg1(), get_arg2()), // sigaction
        62 => sys_kill(get_arg0(), get_arg1()),                 // kill

        // I/O
        29 => sys_select(get_arg0(), get_arg1(), get_arg2(), get_arg3(), get_arg4()), // select
        73 => sys_poll(get_arg0(), get_arg1(), get_arg2() as isize),     // poll
        25 => sys_sendfile(get_arg0(), get_arg1(), get_arg2(), get_arg3()), // sendfile

        // File control
        72 => sys_fcntl(get_arg0(), get_arg1(), get_arg2()),     // fcntl
        29 => sys_ioctl(get_arg0(), get_arg1(), get_arg2()),     // ioctl

        // Time
        96 => sys_gettimeofday(get_arg0(), get_arg1()),          // gettimeofday
        201 => sys_settimeofday(get_arg0(), get_arg1()),        // settimeofday
        113 => sys_clock_gettime(get_arg0(), get_arg1()),         // clock_gettime

        // Process group
        132 => sys_getpgrp(),                                  // getpgrp
        154 => sys_setpgid(get_arg0(), get_arg1()),              // setpgid

        // Resource usage
        165 => sys_getrusage(get_arg0(), get_arg1()),           // getrusage

        // Capability
        90 => sys_capget(get_arg0(), get_arg1()),               // capget
        91 => sys_capset(get_arg0(), get_arg1()),               // capset

        // Debug
        117 => sys_ptrace(get_arg0(), get_arg1(), get_arg2(), get_arg3()), // ptrace

        _ => {
            crate::println!("[syscall] Unknown syscall: unknown");
            -1
        }
    };

    set_ret(result as usize);

    // Advance program counter
    advance_sepc();
}

/// Advance sepc by 4 bytes (skip the ecall instruction)
fn advance_sepc() {
    #[allow(deprecated)]
    let mut sepc = riscv::register::sepc::read();
    sepc += 4;
    #[allow(deprecated)]
    riscv::register::sepc::write(sepc);
}

// ============================================
// File Operations
// ============================================

fn sys_read(fd: usize, buf: usize, count: usize) -> isize {
    if fd != 0 && fd != 1 && fd != 2 {
        return -1;
    }
    // For stdin/stdout/stderr, just return 0 (no input available)
    0
}

pub fn sys_write(fd: usize, buf: usize, count: usize) -> isize {
    if fd != 1 && fd != 2 {
        return -1;
    }
    // Write string to console
    let mut written = 0;
    let mut ptr = buf;
    while written < count {
        let c = unsafe { *(ptr as *const u8) };
        crate::console::sbi_console_putchar(c as usize);
        if c == b'\n' {
            crate::console::sbi_console_putchar(b'\r' as usize);
        }
        ptr += 1;
        written += 1;
    }
    written as isize
}

fn sys_dup(fd: usize) -> isize {
    // Simple dup - just return the same fd for now
    if fd <= 2 {
        fd as isize
    } else {
        -1
    }
}

fn sys_close(fd: usize) -> isize {
    // File descriptors 0-2 are reserved for stdin/stdout/stderr
    if fd <= 2 {
        0
    } else {
        -1
    }
}

fn sys_pipe2(addr: usize, _flags: usize) -> isize {
    // Return two file descriptors in the address
    // For now, just simulate with stdin/stdout
    unsafe {
        *(addr as *mut usize) = 0;  // read end
        *((addr + 8) as *mut usize) = 1;  // write end
    }
    0
}

fn sys_openat(_dirfd: usize, _pathname: usize, _flags: usize, _mode: usize) -> isize {
    crate::println!("[syscall] openat called");
    3  // First user file descriptor
}

fn sys_mkdirat(_dirfd: usize, _pathname: usize, _mode: usize) -> isize {
    crate::println!("[syscall] mkdirat called");
    0
}

fn sys_unlinkat(_dirfd: usize, _pathname: usize, _flags: usize) -> isize {
    crate::println!("[syscall] unlinkat called");
    0
}

// ============================================
// Process Management
// ============================================

/// Exit the current process
pub fn sys_exit(code: usize) -> ! {
    crate::println!("[syscall] Process exiting");
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// Exit group - all threads in a process exit
fn sys_exit_group(code: usize) -> ! {
    sys_exit(code)
}

/// Get current process ID
pub fn sys_getpid() -> isize {
    *CURRENT_PID.lock() as isize
}

/// Get parent process ID
fn sys_getppid() -> isize {
    0  // Parent is init
}

/// Get thread ID (same as PID in our implementation)
fn sys_gettid() -> isize {
    *CURRENT_PID.lock() as isize
}

/// Set tid address (for clone)
fn sys_set_tid_address(_addr: usize) -> isize {
    *CURRENT_PID.lock() as isize
}

/// Wait for child process
fn sys_wait4(_pid: usize, _status_addr: usize, _options: usize, _rusage: usize) -> isize {
    // No children yet, return error
    -1
}

/// Wait for specific child process
fn sys_waitid(_which: usize, _pid: usize, _info_addr: usize, _options: usize, _rusage: usize) -> isize {
    -1
}

// ============================================
// Scheduling
// ============================================

fn sys_nanosleep(_req: usize, _rem: usize) -> isize {
    // Simple implementation - just return success
    0
}

/// Yield the CPU to scheduler
pub fn sys_sched_yield() -> isize {
    // For now, just return success
    // In a real implementation, this would reschedule
    0
}

fn sys_futex(_addr: usize, _op: usize, _val: usize, _timeout: usize, _uaddr2: usize, _val3: usize) -> isize {
    // Futex not fully implemented yet
    0
}

// ============================================
// Memory Management
// ============================================

/// Memory map
/// a0 = addr, a1 = length, a2 = prot, a3 = flags, a4 = fd, a5 = offset
fn sys_mmap(addr: usize, len: usize, prot: usize, flags: usize, fd: usize, offset: usize) -> isize {
    crate::syscall::memory::sys_mmap(addr, len, prot, flags, fd, offset)
}

/// Memory unmap
fn sys_munmap(addr: usize, len: usize) -> isize {
    crate::syscall::memory::sys_munmap(addr, len)
}

/// Set memory protection
fn sys_mprotect(addr: usize, len: usize, prot: usize) -> isize {
    crate::syscall::memory::sys_mprotect(addr, len, prot)
}

/// Change data segment size (heap)
fn sys_brk(addr: usize) -> isize {
    crate::syscall::memory::sys_brk(addr)
}

/// Lock memory
fn sys_mlock(_addr: usize, _len: usize) -> isize {
    0  // Not implemented
}

/// Unlock memory
fn sys_munlock(_addr: usize, _len: usize) -> isize {
    0  // Not implemented
}

// ============================================
// Info
// ============================================

/// Get system information
fn sys_sysinfo() -> isize {
    // Return basic sysinfo structure
    // struct sysinfo { long uptime; ... }
    // For now, just return a dummy value
    0
}

// ============================================
// Process Creation (Clone/Exec)
// ============================================

/// Clone flags
pub const CLONE_VM: usize = 0x00000100;
pub const CLONE_FS: usize = 0x00000200;
pub const CLONE_FILES: usize = 0x00000400;
pub const CLONE_SIGHAND: usize = 0x00008000;
pub const CLONE_THREAD: usize = 0x00010000;
pub const CLONE_VFORK: usize = 0x00004000;

/// Clone - create a new process/thread
/// a0 = flags, a1 = stack, a2 = parent_tidptr, a3 = child_tidptr
fn sys_clone(flags: usize, stack: usize, parent_tid: usize, child_tid: usize) -> isize {
    crate::println!("[syscall] clone called");

    // For COW fork, we would:
    // 1. Allocate a new PID
    // 2. Share the page table (COW)
    // 3. Set up child stack

    // For now, just return the new PID
    let new_pid = alloc_pid();

    // If this is a fork (CLONE_VM not set), we would copy the page table
    if flags & CLONE_VM == 0 {
        // CLONE_VM not set - this is a fork
        crate::println!("[syscall] fork: new pid");
    }

    new_pid as isize
}

/// Execve - execute a program
/// a0 = filename, a1 = argv, a2 = envp
fn sys_execve(_filename: usize, _argv: usize, _envp: usize) -> isize {
    crate::println!("[syscall] execve called");
    -1  // Not implemented yet
}

// ============================================
// Signal Handling
// ============================================

/// Signal numbers
pub mod signal {
    pub const SIGINT: usize = 2;    // Interrupt
    pub const SIGKILL: usize = 9;    // Kill
    pub const SIGSEGV: usize = 11;   // Segmentation fault
    pub const SIGTERM: usize = 15;   // Terminate
    pub const SIGCHLD: usize = 17;   // Child exited
}

/// Sigaction - signal handler
#[repr(C)]
pub struct Sigaction {
    pub handler: usize,      // Signal handler function
    pub flags: usize,        // Flags
    pub mask: usize,         // Signal mask
}

/// Signal handler function type
type SigHandler = extern "C" fn(signal: usize);

/// Set a signal handler
fn sys_sigaction(sig: usize, act: usize, oldact: usize) -> isize {
    crate::println!("[syscall] sigaction called");
    // Not fully implemented yet
    0
}

/// Send a signal to a process
fn sys_kill(pid: usize, sig: usize) -> isize {
    crate::println!("[syscall] kill called");
    0
}

/// Create a signalfd
fn sys_signalfd(fd: usize, _mask: usize, _flags: usize) -> isize {
    fd as isize
}

// ============================================
// I/O Operations
// ============================================

/// Readv - read from multiple buffers
fn sys_readv(fd: usize, iov: usize, iovcnt: usize) -> isize {
    crate::println!("[syscall] readv called");
    -1
}

/// Writev - write to multiple buffers
fn sys_writev(fd: usize, iov: usize, iovcnt: usize) -> isize {
    if fd != 1 && fd != 2 {
        return -1;
    }
    // Simplified: just write the first buffer
    let mut total = 0;
    for i in 0..iovcnt.min(16) {
        let ptr = unsafe { *(iov.add(i) as *const usize) };
        let len = unsafe { *((iov.add(i) + 8) as *const usize) };
        let mut p = ptr;
        for _ in 0..len {
            let c = unsafe { *(p as *const u8) };
            crate::console::sbi_console_putchar(c as usize);
            if c == b'\n' {
                crate::console::sbi_console_putchar(b'\r' as usize);
            }
            p += 1;
        }
        total += len;
    }
    total as isize
}

/// Sendfile - transfer data between file descriptors
fn sys_sendfile(out_fd: usize, in_fd: usize, _offset: usize, count: usize) -> isize {
    if out_fd != 1 && out_fd != 2 {
        return -1;
    }
    // Simplified: just read from in_fd and write to out_fd
    // In a real implementation, we would do actual file I/O
    let buf = 0x10000 as *mut u8; // dummy buffer
    let mut written = 0;
    for _ in 0..count.min(4096) {
        unsafe {
            let c = *buf.add(written);
            crate::console::sbi_console_putchar(c as usize);
        }
        written += 1;
    }
    written as isize
}

/// Poll - wait for events on file descriptors
fn sys_poll(fds: usize, nfds: usize, timeout: isize) -> isize {
    crate::println!("[syscall] poll called");
    // Simplified: return 0 (no events)
    0
}

/// Select - synchronous I/O multiplexing
fn sys_select(nfds: usize, readfds: usize, writefds: usize, exceptfds: usize, timeout: usize) -> isize {
    crate::println!("[syscall] select called");
    0
}

// ============================================
// File Descriptor Operations
// ============================================

/// Create a file descriptor with specific flags
fn sys_dup3(oldfd: usize, newfd: usize, flags: usize) -> isize {
    crate::println!("[syscall] dup3 called");
    if oldfd <= 2 {
        oldfd as isize
    } else {
        -1
    }
}

/// fcntl - file control
fn sys_fcntl(fd: usize, cmd: usize, arg: usize) -> isize {
    match cmd {
        0 => fd as isize,  // F_DUPFD
        1 => {
            // F_SETFD
            0
        }
        2 => {
            // F_GETFD
            0
        }
        3 => {
            // F_SETFL
            0
        }
        4 => {
            // F_GETFL
            0
        }
        _ => -1,
    }
}

/// ioctl - device control
fn sys_ioctl(fd: usize, request: usize, arg: usize) -> isize {
    crate::println!("[syscall] ioctl called");
    0
}

// ============================================
// Time Operations
// ============================================

/// Get current time
fn sys_gettimeofday(tv: usize, tz: usize) -> isize {
    crate::println!("[syscall] gettimeofday called");
    // Return dummy values
    if tv != 0 {
        unsafe {
            *(tv as *mut u64) = 0;      // seconds
            *((tv + 8) as *mut u64) = 0; // microseconds
        }
    }
    0
}

/// Set the time
fn sys_settimeofday(_tv: usize, _tz: usize) -> isize {
    -1
}

/// Clock_gettime
fn sys_clock_gettime(clockid: usize, tp: usize) -> isize {
    crate::println!("[syscall] clock_gettime called");
    if tp != 0 {
        unsafe {
            *((tp) as *mut u64) = 0;      // seconds
            *((tp + 8) as *mut u64) = 0; // nanoseconds
        }
    }
    0
}

// ============================================
// Process Information
// ============================================

/// Get process group ID
fn sys_getpgrp() -> isize {
    0  // PGID is 0 for init
}

/// Set process group ID
fn sys_setpgid(pid: usize, pgid: usize) -> isize {
    crate::println!("[syscall] setpgid called");
    0
}

/// Getrusage - get resource usage
fn sys_getrusage(who: usize, usage: usize) -> isize {
    crate::println!("[syscall] getrusage called");
    // Return zeros
    if usage != 0 {
        let ptr = usage as *mut u64;
        for i in 0..16 {
            unsafe { ptr.write(0); }
        }
    }
    0
}

/// Uptime - get system uptime
fn sys_uptime() -> isize {
    0
}

// ============================================
// Capability Operations
// ============================================

/// Capability check (simplified - always return 0)
fn sys_capget(_hdr: usize, _data: usize) -> isize {
    0
}

fn sys_capset(_hdr: usize, _data: usize) -> isize {
    -1
}

// ============================================
// Debug / Tracing
// ============================================

/// ptrace - process trace
fn sys_ptrace(request: usize, pid: usize, addr: usize, data: usize) -> isize {
    crate::println!("[syscall] ptrace called");
    -1
}

// Re-export for other modules
pub use task::TaskControlBlock;
