//! Hello world user program for trainOS
//!
//! This program demonstrates basic syscall usage and serves as
//! a template for more complex user applications.

#![no_std]
#![no_main]

// Syscall numbers (Linux-compatible RISC-V)
const SYS_EXIT: usize = 93;
const SYS_GETPID: usize = 172;
const SYS_GETTID: usize = 178;
const SYS_GETPPID: usize = 173;
const SYS_GETUID: usize = 174;
const SYS_GETGID: usize = 176;
const SYS_READ: usize = 63;
const SYS_WRITE: usize = 64;
const SYS_BRK: usize = 214;
const SYS_MMAP: usize = 222;
const SYS_GETTIMEOFDAY: usize = 96;
const SYS_CLOCK_GETTIME: usize = 113;
const SYS_SYSINFO: usize = 179;
const SYS_SCHED_YIELD: usize = 124;

// File descriptor constants
const STDIN: usize = 0;
const STDOUT: usize = 1;
const STDERR: usize = 2;

// Syscall functions
#[inline(always)]
fn syscall1(id: usize, arg0: usize) -> usize {
    let ret;
    unsafe {
        core::arch::asm!(
            "mv a7, {0}",
            "mv a0, {1}",
            "ecall",
            in(reg) id,
            in(reg) arg0,
            lateout("a0") ret
        );
    }
    ret
}

#[inline(always)]
fn syscall2(id: usize, arg0: usize, arg1: usize) -> usize {
    let ret;
    unsafe {
        core::arch::asm!(
            "mv a7, {0}",
            "mv a0, {1}",
            "mv a1, {2}",
            "ecall",
            in(reg) id,
            in(reg) arg0,
            in(reg) arg1,
            lateout("a0") ret
        );
    }
    ret
}

#[inline(always)]
fn syscall3(id: usize, arg0: usize, arg1: usize, arg2: usize) -> usize {
    let ret;
    unsafe {
        core::arch::asm!(
            "mv a7, {0}",
            "mv a0, {1}",
            "mv a1, {2}",
            "mv a2, {3}",
            "ecall",
            in(reg) id,
            in(reg) arg0,
            in(reg) arg1,
            in(reg) arg2,
            lateout("a0") ret
        );
    }
    ret
}

// Write to stdout
fn write(fd: usize, buf: *const u8, count: usize) -> usize {
    syscall3(SYS_WRITE, fd, buf as usize, count)
}

// Read from stdin
fn read(fd: usize, buf: *mut u8, count: usize) -> usize {
    syscall3(SYS_READ, fd, buf as usize, count)
}

// Exit
fn exit(code: usize) -> ! {
    syscall1(SYS_EXIT, code);
    loop {}
}

// Get PID
fn getpid() -> usize {
    syscall1(SYS_GETPID, 0)
}

// Get TID
fn gettid() -> usize {
    syscall1(SYS_GETTID, 0)
}

// Get PPID
fn getppid() -> usize {
    syscall1(SYS_GETPPID, 0)
}

// Get UID
fn getuid() -> usize {
    syscall1(SYS_GETUID, 0)
}

// Get GID
fn getgid() -> usize {
    syscall1(SYS_GETGID, 0)
}

// Yield
fn sched_yield() -> usize {
    syscall1(SYS_SCHED_YIELD, 0)
}

// Time structures
#[repr(C)]
struct TimeVal {
    tv_sec: i64,
    tv_usec: i64,
}

#[repr(C)]
struct Timespec {
    tv_sec: i64,
    tv_nsec: i64,
}

#[repr(C)]
struct SysInfo {
    uptime: i64,
    loads: [i64; 3],
    totalram: u64,
    freeram: u64,
    sharedram: u64,
    bufferram: u64,
    totalswap: u64,
    freeswap: u64,
    procs: u16,
    pad: u16,
    totalhigh: u64,
    freehigh: u64,
    mem_unit: u32,
    _pad2: [u8; 8],
}

// Get time of day
fn gettimeofday(tv: *mut TimeVal) -> usize {
    syscall2(SYS_GETTIMEOFDAY, tv as usize, 0)
}

// Get clock time
fn clock_gettime(clockid: usize, tp: *mut Timespec) -> usize {
    syscall2(SYS_CLOCK_GETTIME, clockid, tp as usize)
}

// Get sysinfo
fn sysinfo(info: *mut SysInfo) -> usize {
    syscall1(SYS_SYSINFO, info as usize)
}

// Write string to fd
fn write_str(fd: usize, s: &[u8]) {
    write(fd, s.as_ptr(), s.len());
}

// Print a number
fn print_num(n: usize) {
    if n == 0 {
        putc(b'0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut len = 0;
    let mut x = n;
    while x > 0 {
        buf[len] = b'0' + (x % 10) as u8;
        len += 1;
        x /= 10;
    }
    for i in (0..len).rev() {
        putc(buf[i]);
    }
}

// Putchar via write syscall
fn putc(c: u8) {
    let _ = write(STDOUT, &c, 1);
}

// String length
fn strlen(s: *const u8) -> usize {
    let mut len = 0;
    unsafe {
        while *s.add(len) != 0 {
            len += 1;
        }
    }
    len
}

// Main
#[no_mangle]
extern "C" fn _start() {
    // Debug: try to print 'H' before infinite loop
    unsafe {
        core::arch::asm!(
            "li a7, 1",
            "li a0, 72",
            "ecall",
            "li a7, 1",
            "li a0, 73",
            "ecall"
        );
    }

    // Now infinite loop
    loop {}
    write_str(STDOUT, b"================================\n");

    // Get and display PID info
    let pid = getpid();
    let tid = gettid();
    let ppid = getppid();
    let uid = getuid();
    let gid = getgid();

    write_str(STDOUT, b"PID: ");
    print_num(pid);
    write_str(STDOUT, b" TID: ");
    print_num(tid);
    write_str(STDOUT, b" PPID: ");
    print_num(ppid);
    write_str(STDOUT, b"\nUID: ");
    print_num(uid);
    write_str(STDOUT, b" GID: ");
    print_num(gid);
    write_str(STDOUT, b"\n");

    // Test sched_yield
    let _ = sched_yield();
    write_str(STDOUT, b"sched_yield() called successfully\n");

    // Display sysinfo
    let mut info: SysInfo = unsafe { core::mem::zeroed() };
    let ret = sysinfo(&mut info as *mut SysInfo);
    if ret == 0 {
        write_str(STDOUT, b"SysInfo:\n");
        write_str(STDOUT, b"  Uptime: ");
        print_num(info.uptime as usize);
        write_str(STDOUT, b" seconds\n");
        write_str(STDOUT, b"  Procs: ");
        print_num(info.procs as usize);
        write_str(STDOUT, b"\n");
        write_str(STDOUT, b"  TotalRAM: ");
        print_num(info.totalram as usize);
        write_str(STDOUT, b" bytes\n");
    }

    // Display time
    let mut tv: TimeVal = unsafe { core::mem::zeroed() };
    let ret = gettimeofday(&mut tv as *mut TimeVal);
    if ret == 0 {
        write_str(STDOUT, b"Time: ");
        print_num(tv.tv_sec as usize);
        putc(b'.');
        print_num(tv.tv_usec as usize);
        putc(b'\n');
    }

    // Test clock_gettime
    let mut ts: Timespec = unsafe { core::mem::zeroed() };
    let ret = clock_gettime(0, &mut ts as *mut Timespec);
    if ret == 0 {
        write_str(STDOUT, b"Clock: ");
        print_num(ts.tv_sec as usize);
        putc(b'.');
        print_num(ts.tv_nsec as usize);
        putc(b'\n');
    }

    write_str(STDOUT, b"\nHello program completed successfully!\n");
    exit(0);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    write_str(STDERR, b"\nPanic in user program!\n");
    exit(1);
}
