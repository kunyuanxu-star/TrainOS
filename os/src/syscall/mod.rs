//! System call module
//!
//! Implements Linux-compatible system call interface

pub mod task;
pub mod memory;
pub mod fs;
pub mod net;
pub mod fd;
pub mod newlib;

use core::ops::Add;
use spin::Mutex;

/// Linux syscall numbers (RISC-V Linux compatible)
/// See /usr/include/asm-generic/unistd.h
pub mod nr {
    // Process
    pub const EXIT: usize = 93;
    pub const EXIT_GROUP: usize = 94;
    pub const CLONE: usize = 220;
    pub const EXECVE: usize = 221;
    pub const WAIT4: usize = 260;
    pub const WAITID: usize = 287;
    pub const GETPID: usize = 172;
    pub const GETTID: usize = 178;
    pub const GETPPID: usize = 173;
    pub const SET_TID_ADDRESS: usize = 96;
    pub const SCHED_YIELD: usize = 124;
    pub const FUTEX: usize = 98;

    // Memory
    pub const BRK: usize = 214;
    pub const MUNMAP: usize = 215;
    pub const MMAP: usize = 222;
    pub const MPROTECT: usize = 226;
    pub const MLOCK: usize = 228;
    pub const MUNLOCK: usize = 229;
    pub const MREMAP: usize = 216;
    pub const MADVISE: usize = 233;

    // I/O
    pub const READ: usize = 63;
    pub const WRITE: usize = 64;
    pub const READV: usize = 65;
    pub const WRITEV: usize = 66;
    pub const PREAD64: usize = 67;
    pub const PWRITE64: usize = 68;
    pub const OPENAT: usize = 56;
    pub const CLOSE: usize = 57;
    pub const PIPE2: usize = 59;
    pub const DUP: usize = 23;
    pub const DUP3: usize = 24;
    pub const SENDFILE: usize = 71;
    pub const SELECT: usize = 29;
    pub const POLL: usize = 73;

    // File
    pub const STAT: usize = 80;
    pub const FSTAT: usize = 80;  // actually 5 on riscv
    pub const LSTAT: usize = 80;
    pub const LINKAT: usize = 37;
    pub const UNLINKAT: usize = 35;
    pub const MKDIRAT: usize = 34;
    pub const RMDIR: usize = 84;
    pub const READLINKAT: usize = 78;
    pub const RENAMEAT2: usize = 38;
    pub const TRUNCATE: usize = 45;
    pub const FTRUNCATE: usize = 46;
    pub const FALLOCATE: usize = 47;
    pub const FSTATFS: usize = 80;
    pub const STATFS: usize = 80;

    // Memory-mapped files
    pub const MSYNC: usize = 227;
    pub const FLOCK: usize = 73;

    // Signals
    pub const SIGACTION: usize = 134;
    pub const RT_SIGACTION: usize = 134;
    pub const SIGPROCMASK: usize = 135;
    pub const RT_SIGPROCMASK: usize = 135;
    pub const SIGRETURN: usize = 139;
    pub const KILL: usize = 129;
    pub const TKILL: usize = 130;
    pub const SIGALTSTACK: usize = 132;

    // Time
    pub const NANOSLEEP: usize = 101;
    pub const GETTIMEOFDAY: usize = 96;
    pub const SETTIMEOFDAY: usize = 99;
    pub const CLOCK_GETTIME: usize = 113;
    pub const CLOCK_GETRES: usize = 114;
    pub const CLOCK_NANO_SLEEP: usize = 115;

    // Process group
    pub const GETPGRP: usize = 160;
    pub const SETPGID: usize = 157;
    pub const GETPGID: usize = 155;
    pub const GETSID: usize = 147;

    // Resource
    pub const GETRUSAGE: usize = 165;
    pub const PRLIMIT64: usize = 261;

    // Sysinfo
    pub const SYSINFO: usize = 179;

    // Prctl
    pub const PRCTL: usize = 167;

    // Syslog
    pub const SYSLOG: usize = 82;

    // Debug
    pub const PTRACE: usize = 117;

    // Fcntl
    pub const FCNTL: usize = 25;
    pub const IOCTL: usize = 29;

    // Sockets
    pub const SOCKET: usize = 198;
    pub const BIND: usize = 200;
    pub const CONNECT: usize = 201;
    pub const LISTEN: usize = 202;
    pub const ACCEPT: usize = 202;
    pub const ACCEPT4: usize = 202;
    pub const SENDTO: usize = 206;
    pub const RECVFROM: usize = 207;
    pub const SHUTDOWN: usize = 210;
    pub const SETSOCKOPT: usize = 208;
    pub const GETSOCKOPT: usize = 209;
    pub const GETSOCKNAME: usize = 200;
    pub const GETPEERNAME: usize = 201;
    pub const SOCKETPAIR: usize = 199;

    // Epoll
    pub const EPOLL_CREATE: usize = 228;  // or 20
    pub const EPOLL_CTL: usize = 227;
    pub const EPOLL_WAIT: usize = 229;
    pub const EPOLL_PWAIT: usize = 229;

    // Eventfd
    pub const EVENTFD: usize = 227;
    pub const EVENTFD2: usize = 228;

    // Timer
    pub const TIMER_CREATE: usize = 222;
    pub const TIMER_DELETE: usize = 223;
    pub const TIMER_SETTIME: usize = 224;
    pub const TIMER_GETTIME: usize = 225;
    pub const TIMER_GETOVERRUN: usize = 225;
    pub const ALARM: usize = 225;

    // Capability
    pub const CAPGET: usize = 90;
    pub const CAPSET: usize = 91;

    // Mount
    pub const MOUNT: usize = 40;
    pub const UMOUNT2: usize = 40;

    // Chdir
    pub const CHDIR: usize = 49;
    pub const FCHDIR: usize = 50;
    pub const RENAME: usize = 38;
    pub const MKNODAT: usize = 33;

    // Umask
    pub const UMASK: usize = 95;

    // Access
    pub const ACCESS: usize = 48;
    pub const FACCESS: usize = 48;

    // Pipe
    pub const SYSCALL36: usize = 36;  // renameat

    // Sched
    pub const SCHED_SETPARAM: usize = 121;
    pub const SCHED_GETPARAM: usize = 122;
    pub const SCHED_SETSCHEDULER: usize = 123;
    pub const SCHED_GETSCHEDULER: usize = 125;
    pub const SCHED_GET_PRIORITY_MAX: usize = 127;
    pub const SCHED_GET_PRIORITY_MIN: usize = 128;
    pub const SCHED_RR_GET_INTERVAL: usize = 128;

    // Misc
    pub const GETCPU: usize = 168;
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
/// a0 = pointer to trap frame
#[no_mangle]
pub extern "C" fn do_syscall(trap_frame: *mut crate::process::context::TrapFrame) {
    let syscall_id: usize;
    unsafe {
        core::arch::asm!("mv {}, a7", out(reg) syscall_id);
    }

    // Get arguments from trap frame
    let arg0 = unsafe { (*trap_frame).a0 };
    let arg1 = unsafe { (*trap_frame).a1 };
    let arg2 = unsafe { (*trap_frame).a2 };
    let arg3 = unsafe { (*trap_frame).a3 };

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
        174 => sys_getuid(),                                    // getuid
        175 => sys_geteuid(),                                   // geteuid
        176 => sys_getgid(),                                    // getgid
        177 => sys_getegid(),                                   // getegid
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
        220 => sys_clone(trap_frame, get_arg0(), get_arg1(), get_arg2(), get_arg3()), // clone
        221 => sys_execve(get_arg0(), get_arg1(), get_arg2()), // execve

        // File operations (Linux standard)
        56 => sys_openat(get_arg0(), get_arg1(), get_arg2(), get_arg3()), // openat
        34 => sys_mkdirat(get_arg0(), get_arg1(), get_arg2()), // mkdirat
        35 => sys_unlinkat(get_arg0(), get_arg1(), get_arg2()), // unlinkat
        37 => sys_linkat(get_arg0(), get_arg1(), get_arg2(), get_arg3(), get_arg4()), // linkat
        38 => sys_renameat2(get_arg0(), get_arg1(), get_arg2(), get_arg3(), get_arg4()), // renameat2
        45 => sys_truncate(get_arg0(), get_arg1()),               // truncate
        46 => sys_ftruncate(get_arg0(), get_arg1()),             // ftruncate
        17 => sys_getcwd(get_arg0(), get_arg1()),                // getcwd
        49 => sys_chdir(get_arg0()),                             // chdir
        50 => sys_fchdir(get_arg0()),                           // fchdir

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

        // Sockets (Linux numbers)
        198 => net::sys_socket(get_arg0() as i32, get_arg1() as i32, get_arg2() as i32), // socket
        200 => net::sys_bind(get_arg0(), get_arg1(), get_arg2()), // bind
        201 => net::sys_connect(get_arg0(), get_arg1(), get_arg2()), // connect
        202 => net::sys_listen(get_arg0(), get_arg1() as i32), // listen
        206 => net::sys_sendto(get_arg0(), get_arg1(), get_arg2(), get_arg3(), get_arg4(), get_arg5()), // sendto
        207 => net::sys_recvfrom(get_arg0(), get_arg1(), get_arg2(), get_arg3(), get_arg4(), get_arg5()), // recvfrom
        210 => net::sys_shutdown(get_arg0(), get_arg1() as i32), // shutdown
        208 => net::sys_setsockopt(get_arg0(), get_arg1() as i32, get_arg2() as i32, get_arg3(), get_arg4()), // setsockopt
        209 => net::sys_getsockopt(get_arg0(), get_arg1() as i32, get_arg2() as i32, get_arg3(), get_arg4()), // getsockopt
        200 => net::sys_getsockname(get_arg0(), get_arg1(), get_arg2()), // getsockname
        201 => net::sys_getpeername(get_arg0(), get_arg1(), get_arg2()), // getpeername
        199 => net::sys_socketpair(get_arg0() as i32, get_arg1() as i32, get_arg2() as i32, get_arg3()), // socketpair

        // Epoll
        20 => sys_epoll_create(get_arg0()), // epoll_create
        227 => sys_epoll_ctl(get_arg0(), get_arg1(), get_arg2(), get_arg3()), // epoll_ctl
        229 => sys_epoll_wait(get_arg0(), get_arg1(), get_arg2(), get_arg3()), // epoll_wait

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
    // For stdin (fd 0), return EOF for now (no keyboard input)
    if fd == 0 {
        // In a real implementation, this would read from keyboard
        // For now, return that no data is available
        crate::println!("[syscall] read from stdin (not implemented)");
        0
    } else if fd == 1 || fd == 2 {
        // Can't read from stdout/stderr
        -1
    } else {
        // File or other fd
        crate::println!("[syscall] read from fd");
        -1  // Not implemented yet
    }
}

pub fn sys_write(fd: usize, buf: usize, count: usize) -> isize {
    // stdout = 1, stderr = 2
    if fd == 1 || fd == 2 {
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
    } else if fd == 0 {
        // Can't write to stdin
        -1
    } else {
        // File or other fd
        crate::println!("[syscall] write to fd");
        -1  // Not implemented yet
    }
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

fn sys_linkat(_olddirfd: usize, _oldpath: usize, _newdirfd: usize, _newpath: usize, _flags: usize) -> isize {
    crate::println!("[syscall] linkat called");
    -1  // Not implemented
}

fn sys_renameat2(_olddirfd: usize, _oldpath: usize, _newdirfd: usize, _newpath: usize, _flags: usize) -> isize {
    crate::println!("[syscall] renameat2 called");
    -1  // Not implemented
}

fn sys_truncate(_path: usize, _length: usize) -> isize {
    crate::println!("[syscall] truncate called");
    -1  // Not implemented
}

fn sys_ftruncate(_fd: usize, _length: usize) -> isize {
    crate::println!("[syscall] ftruncate called");
    0
}

fn sys_getcwd(buf: usize, size: usize) -> isize {
    if buf == 0 || size == 0 {
        return -1;
    }
    // Return "/" as current directory
    unsafe {
        let dest = &mut *(buf as *mut u8);
        *dest = b'/';
        *((buf + 1) as *mut u8) = 0;
    }
    2 as isize
}

fn sys_chdir(_path: usize) -> isize {
    crate::println!("[syscall] chdir called");
    0
}

fn sys_fchdir(_fd: usize) -> isize {
    crate::println!("[syscall] fchdir called");
    0
}

// ============================================
// Process Management
// ============================================

/// Exit the current process
pub fn sys_exit(code: usize) -> ! {
    let pid = *CURRENT_PID.lock();
    crate::println!("[syscall] Process exiting");
    crate::println!("[syscall] Process halted");
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

/// Get current user ID
fn sys_getuid() -> isize {
    0  // Root user
}

/// Get current effective user ID
fn sys_geteuid() -> isize {
    0  // Root user
}

/// Get current group ID
fn sys_getgid() -> isize {
    0  // Root group
}

/// Get current effective group ID
fn sys_getegid() -> isize {
    0  // Root group
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
/// _pid: -1 means wait for any child, >0 means wait for specific child
/// status_addr: where to store exit status
/// options: WNOHANG to not block, WUNTRACED, etc.
fn sys_wait4(_pid: usize, status_addr: usize, options: usize, _rusage: usize) -> isize {
    // For now, just return no children
    // In a full implementation:
    // 1. Find a child in Zombie state
    // 2. If WNOHANG, return immediately
    // 3. Otherwise block until a child exits
    // 4. Copy exit status to status_addr

    if status_addr != 0 {
        // Store exit status as 0 (no child to reap)
        unsafe {
            *(status_addr as *mut u32) = 0;
        }
    }

    // Return error: no child to wait for
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
pub const CLONE_FORK: usize = 0x00040000;
pub const CLONE_SETTLS: usize = 0x00080000;
pub const CLONE_CHILD_CLEARTID: usize = 0x00200000;
pub const CLONE_CHILD_SETTID: usize = 0x01000000;

/// Clone - create a new process/thread
/// trap_frame = pointer to parent's trap frame
/// a0 = flags, a1 = stack ptr, a2 = parent_tidptr, a3 = child_tidptr
fn sys_clone(trap_frame: *mut crate::process::context::TrapFrame, flags: usize, stack_ptr: usize, _parent_tid: usize, _child_tid: usize) -> isize {
    // Get current task's PID as parent
    let parent_pid = *CURRENT_PID.lock();

    // Allocate a new PID
    let new_pid = alloc_pid();

    crate::println!("[syscall] clone: creating child process");

    // For fork (CLONE_VM not set), we need to copy the address space
    // For thread (CLONE_VM set), we share the address space
    let is_thread = (flags & CLONE_VM) != 0;

    if !is_thread {
        // This is a fork - for now, just create a simple child task
        // In a full implementation, we would copy the parent's page table
        // with COW (Copy-on-Write) semantics

        // Create child TCB
        let mut child_tcb = crate::process::task::TaskControlBlock::new(new_pid);
        child_tcb.parent_id = Some(crate::process::task::TaskId::new(parent_pid));
        child_tcb.status = crate::process::task::TaskStatus::Ready;
        child_tcb.alloc_kernel_stack();

        // For fork, set up trap frame to return 0 for child
        // The child will have its own stack and will return 0 from clone
        // For now, just mark the child as ready
    } else {
        // This is a thread - share address space
        crate::println!("[syscall] clone: creating thread (shared VM)");
    }

    // Set the parent's return value to child's PID
    // The trap frame is on the stack, modify a0 there
    unsafe {
        (*trap_frame).a0 = new_pid;
    }

    // Parent returns child's PID
    new_pid as isize
}

/// Execve - execute a program
/// a0 = filename, a1 = argv, a2 = envp
fn sys_execve(filename: usize, argv: usize, envp: usize) -> isize {
    // Try to read the filename
    let mut name_buf = [0u8; 256];
    let mut i = 0;
    while i < 255 {
        let c = unsafe { *(filename as *const u8).add(i) };
        name_buf[i] = c;
        if c == 0 {
            break;
        }
        i += 1;
    }
    name_buf[i] = 0;

    let prog_name = match core::str::from_utf8(&name_buf) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    crate::println!("[syscall] execve: loading program");

    // Try to find the program
    // For now, only handle built-in programs
    let _entry_point: Option<usize> = match prog_name {
        "/bin/hello" | "hello" => Some(0x00400000),
        "/bin/shell" | "shell" => Some(0x00400000),
        "/bin/vi" | "vi" => Some(0x00400000),
        _ => None,
    };

    if _entry_point.is_none() {
        crate::println!("[syscall] execve: program not found");
        return -1;
    }

    // In a real implementation:
    // 1. Load ELF from filesystem
    // 2. Create new address space (page table)
    // 3. Map ELF segments into user space
    // 4. Set up argv/envp on user stack
    // 5. Set up trap frame for user mode entry

    // For now, just return success
    // The actual user mode switch happens on the next schedule
    crate::println!("[syscall] execve: program found");

    0
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

// ============================================
// Epoll Operations
// ============================================

/// Epoll file descriptor table
const MAX_EPOLL_FDS: usize = 64;

#[derive(Debug, Clone, Copy)]
enum EpollItem {
    None,
    Fd { fd: usize, events: u32 },
}

static EPOLL_TABLE: Mutex<[Option<EpollItem>; MAX_EPOLL_FDS]> = Mutex::new([None; MAX_EPOLL_FDS]);

/// Create an epoll file descriptor
fn sys_epoll_create(_size: usize) -> isize {
    let mut table = EPOLL_TABLE.lock();
    // Find a free slot (start from 32 to avoid stdio fds)
    for i in 32..MAX_EPOLL_FDS {
        if table[i].is_none() {
            table[i] = Some(EpollItem::None);
            return i as isize;
        }
    }
    -1
}

/// Control operation on an epoll file descriptor
fn sys_epoll_ctl(epfd: usize, op: usize, fd: usize, event: usize) -> isize {
    if epfd >= MAX_EPOLL_FDS {
        return -1;
    }

    let mut table = EPOLL_TABLE.lock();

    match op {
        1 => {
            // EPOLL_CTL_ADD
            let events = if event != 0 {
                unsafe { *(event as *const u32) }
            } else {
                0
            };
            table[epfd] = Some(EpollItem::Fd { fd, events });
            0
        }
        2 => {
            // EPOLL_CTL_DEL
            table[epfd] = Some(EpollItem::None);
            0
        }
        3 => {
            // EPOLL_CTL_MOD - modify existing
            if let Some(EpollItem::Fd { fd: old_fd, .. }) = table[epfd] {
                let events = if event != 0 {
                    unsafe { *(event as *const u32) }
                } else {
                    0
                };
                table[epfd] = Some(EpollItem::Fd { fd: old_fd, events });
                0
            } else {
                -1
            }
        }
        _ => -1,
    }
}

/// Wait for events on an epoll file descriptor
fn sys_epoll_wait(epfd: usize, events: usize, maxevents: usize, timeout: usize) -> isize {
    if epfd >= MAX_EPOLL_FDS || events == 0 || maxevents == 0 {
        return -1;
    }

    // Simplified implementation - return no events
    // In a real implementation, this would block and wait
    0
}

// Re-export for other modules
// pub use crate::process::task::TaskControlBlock;
