//! Simple shell for trainOS
//!
//! A basic shell that can run commands

#![no_std]
#![no_main]

// Syscall numbers
const SYS_EXIT: usize = 93;
const SYS_READ: usize = 63;
const SYS_WRITE: usize = 64;
const SYS_GETPID: usize = 172;
const SYS_GETTID: usize = 178;
const SYS_SCHED_YIELD: usize = 124;
const SYS_SYSINFO: usize = 179;

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

// Yield
fn sched_yield() -> usize {
    syscall1(SYS_SCHED_YIELD, 0)
}

// Syscall1 wrapper for sysinfo
fn sysinfo(info: *mut SysInfo) -> usize {
    syscall1(SYS_SYSINFO, info as usize)
}

// Write N bytes
fn write_n(fd: usize, buf: *const u8, len: usize) {
    let _ = write(fd, buf, len);
}

// Write string (null-terminated)
fn write_str(fd: usize, s: *const u8) {
    let len = strlen(s);
    let _ = write(fd, s, len);
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

// Compare strings
fn strcmp(s1: *const u8, s2: *const u8) -> bool {
    let mut i = 0;
    loop {
        let c1 = unsafe { *s1.add(i) };
        let c2 = unsafe { *s2.add(i) };
        if c1 != c2 {
            return false;
        }
        if c1 == 0 {
            return true;
        }
        i += 1;
    }
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
    // Print in reverse order using putc
    let mut i = len;
    while i > 0 {
        i -= 1;
        putc(buf[i]);
    }
}

// Print prompt
fn print_prompt() {
    let pid = getpid();
    write_str(STDOUT, b"trainOS:~$\0".as_ptr() as *const u8);
    print_num(pid);
    write_str(STDOUT, b" $\0".as_ptr() as *const u8);
}

// Read a line from stdin
fn read_line(buf: *mut u8, max_len: usize) -> usize {
    let mut pos = 0;
    loop {
        if pos >= max_len - 1 {
            break;
        }
        let mut byte: u8 = 0;
        let n = read(STDIN, &mut byte, 1);
        if n == 0 {
            break;
        }
        if byte == b'\n' || byte == b'\r' {
            unsafe { *buf.add(pos) = 0; }
            putc(b'\n');
            break;
        }
        if byte == 127 || byte == 8 {  // Backspace
            if pos > 0 {
                pos -= 1;
                unsafe { *buf.add(pos) = 0; }
                putc(8);  // Backspace
                putc(32); // Space
                putc(8);  // Backspace
            }
        } else {
            unsafe { *buf.add(pos) = byte; }
            putc(byte);
            pos += 1;
        }
    }
    pos
}

// SysInfo structure
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

// Built-in commands
fn builtin_help() {
    write_str(STDOUT, b"Available commands:\n\0".as_ptr() as *const u8);
    write_str(STDOUT, b"  help    - Show this help message\n\0".as_ptr() as *const u8);
    write_str(STDOUT, b"  hello   - Print hello message\n\0".as_ptr() as *const u8);
    write_str(STDOUT, b"  pid     - Print process ID\n\0".as_ptr() as *const u8);
    write_str(STDOUT, b"  tid     - Print thread ID\n\0".as_ptr() as *const u8);
    write_str(STDOUT, b"  yield   - Yield to scheduler\n\0".as_ptr() as *const u8);
    write_str(STDOUT, b"  uptime  - Show system uptime\n\0".as_ptr() as *const u8);
    write_str(STDOUT, b"  clear   - Clear screen\n\0".as_ptr() as *const u8);
    write_str(STDOUT, b"  exit    - Exit the shell\n\0".as_ptr() as *const u8);
}

fn builtin_hello() {
    write_str(STDOUT, b"Hello from trainOS shell!\n\0".as_ptr() as *const u8);
}

fn builtin_pid() {
    write_str(STDOUT, b"PID: \0".as_ptr() as *const u8);
    print_num(getpid());
    write_str(STDOUT, b"\n\0".as_ptr() as *const u8);
}

fn builtin_tid() {
    write_str(STDOUT, b"TID: \0".as_ptr() as *const u8);
    print_num(gettid());
    write_str(STDOUT, b"\n\0".as_ptr() as *const u8);
}

fn builtin_yield() {
    let ret = sched_yield();
    write_str(STDOUT, b"sched_yield returned: \0".as_ptr() as *const u8);
    print_num(ret);
    write_str(STDOUT, b"\n\0".as_ptr() as *const u8);
}

fn builtin_uptime() {
    let mut info: SysInfo = unsafe { core::mem::zeroed() };
    let ret = sysinfo(&mut info as *mut SysInfo);
    if ret == 0 {
        write_str(STDOUT, b"Uptime: \0".as_ptr() as *const u8);
        print_num(info.uptime as usize);
        write_str(STDOUT, b" seconds\n\0".as_ptr() as *const u8);
        write_str(STDOUT, b"Procs: \0".as_ptr() as *const u8);
        print_num(info.procs as usize);
        write_str(STDOUT, b"\n\0".as_ptr() as *const u8);
    }
}

fn builtin_clear() {
    for _ in 0..40 {
        putc(b'\n');
    }
}

// Execute command
fn execute(cmd: *const u8) {
    if strcmp(cmd, b"help\0".as_ptr() as *const u8) {
        builtin_help();
    } else if strcmp(cmd, b"hello\0".as_ptr() as *const u8) {
        builtin_hello();
    } else if strcmp(cmd, b"pid\0".as_ptr() as *const u8) {
        builtin_pid();
    } else if strcmp(cmd, b"tid\0".as_ptr() as *const u8) {
        builtin_tid();
    } else if strcmp(cmd, b"yield\0".as_ptr() as *const u8) {
        builtin_yield();
    } else if strcmp(cmd, b"uptime\0".as_ptr() as *const u8) {
        builtin_uptime();
    } else if strcmp(cmd, b"clear\0".as_ptr() as *const u8) {
        builtin_clear();
    } else if strcmp(cmd, b"exit\0".as_ptr() as *const u8) {
        write_str(STDOUT, b"Goodbye!\n\0".as_ptr() as *const u8);
        exit(0);
    } else {
        write_str(STDOUT, b"Unknown command: \0".as_ptr() as *const u8);
        write_str(STDOUT, cmd);
        write_str(STDOUT, b"\nType 'help' for available commands.\n\0".as_ptr() as *const u8);
    }
}

// Main shell loop
#[no_mangle]
extern "C" fn _start() {
    write_str(STDOUT, b"\n\0".as_ptr() as *const u8);
    write_str(STDOUT, b"========================================\n\0".as_ptr() as *const u8);
    write_str(STDOUT, b"  Welcome to trainOS Shell\n\0".as_ptr() as *const u8);
    write_str(STDOUT, b"========================================\n\0".as_ptr() as *const u8);
    write_str(STDOUT, b"Type 'help' for available commands.\n\0".as_ptr() as *const u8);

    // Command buffer
    let mut cmd_buf = [0u8; 256];

    loop {
        print_prompt();

        // Read command
        let _cmd_len = read_line(cmd_buf.as_mut_ptr(), 256);

        // Execute command
        execute(cmd_buf.as_ptr());
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    write_str(STDERR, b"\nPanic in shell!\n\0".as_ptr() as *const u8);
    exit(1);
}
