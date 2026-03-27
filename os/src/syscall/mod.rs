//! System call module
//!
//! Implements Linux-compatible system call interface

pub mod task;
pub mod memory;
pub mod fs;

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
        57 => sys_close(get_arg0()),                            // close
        59 => sys_pipe2(get_arg0(), get_arg1()),                // pipe2

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
    if flags & 0x00010000 == 0 {
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

// Re-export for other modules
pub use task::TaskControlBlock;
