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
const SYS_GETPPID: usize = 173;
const SYS_GETUID: usize = 174;
const SYS_GETGID: usize = 176;
const SYS_GETEUID: usize = 175;
const SYS_GETEGID: usize = 177;
const SYS_SCHED_YIELD: usize = 124;
const SYS_SYSINFO: usize = 179;
const SYS_GETCWD: usize = 17;
const SYS_CHDIR: usize = 49;
const SYS_OPENAT: usize = 56;
const SYS_CLOSE: usize = 57;
const SYS_GETTIMEOFDAY: usize = 96;
const SYS_CLOCK_GETTIME: usize = 113;
const SYS_UNAME: usize = 160;
const SYS_PRCTL: usize = 167;
const SYS_RT_SIGACTION: usize = 134;
const SYS_GETRUSAGE: usize = 165;
const SYS_PRLIMIT64: usize = 261;

// File descriptor constants
const STDIN: usize = 0;
const STDOUT: usize = 1;
const STDERR: usize = 2;

// Open flags
const O_RDONLY: usize = 0;
const O_WRONLY: usize = 1;
const O_RDWR: usize = 2;
const O_CREAT: usize = 0o100;
const O_EXCL: usize = 0o200;
const O_TRUNC: usize = 0o1000;
const O_APPEND: usize = 0o2000;

// File mode
const S_IRWXU: usize = 0o700;
const S_IRUSR: usize = 0o400;
const S_IWUSR: usize = 0o200;
const S_IXUSR: usize = 0o100;

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

#[inline(always)]
fn syscall4(id: usize, arg0: usize, arg1: usize, arg2: usize, arg3: usize) -> usize {
    let ret;
    unsafe {
        core::arch::asm!(
            "mv a7, {0}",
            "mv a0, {1}",
            "mv a1, {2}",
            "mv a2, {3}",
            "mv a3, {4}",
            "ecall",
            in(reg) id,
            in(reg) arg0,
            in(reg) arg1,
            in(reg) arg2,
            in(reg) arg3,
            lateout("a0") ret
        );
    }
    ret
}

#[inline(always)]
fn syscall5(id: usize, arg0: usize, arg1: usize, arg2: usize, arg3: usize, arg4: usize) -> usize {
    let ret;
    unsafe {
        core::arch::asm!(
            "mv a7, {0}",
            "mv a0, {1}",
            "mv a1, {2}",
            "mv a2, {3}",
            "mv a3, {4}",
            "mv a4, {5}",
            "ecall",
            in(reg) id,
            in(reg) arg0,
            in(reg) arg1,
            in(reg) arg2,
            in(reg) arg3,
            in(reg) arg4,
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

// Get EUID
fn geteuid() -> usize {
    syscall1(SYS_GETEUID, 0)
}

// Get EGID
fn getegid() -> usize {
    syscall1(SYS_GETEGID, 0)
}

// Yield
fn sched_yield() -> usize {
    syscall1(SYS_SCHED_YIELD, 0)
}

// Get cwd
fn getcwd(buf: *mut u8, size: usize) -> usize {
    syscall2(SYS_GETCWD, buf as usize, size)
}

// Chdir
fn chdir(path: *const u8) -> usize {
    syscall1(SYS_CHDIR, path as usize)
}

// Open file
fn openat(dirfd: isize, path: *const u8, flags: usize, mode: usize) -> isize {
    syscall4(SYS_OPENAT, dirfd as usize, path as usize, flags, mode) as isize
}

// Close file
fn close(fd: usize) -> usize {
    syscall1(SYS_CLOSE, fd)
}

// Get time of day
fn gettimeofday() -> (i64, i64) {
    #[repr(C)]
    struct TimeVal {
        tv_sec: i64,
        tv_usec: i64,
    }
    let mut tv: TimeVal = unsafe { core::mem::zeroed() };
    let ret = syscall2(SYS_GETTIMEOFDAY, &mut tv as *mut TimeVal as usize, 0);
    if ret == 0 {
        (tv.tv_sec, tv.tv_usec)
    } else {
        (0, 0)
    }
}

// Get clock time
fn clock_gettime() -> (i64, i64) {
    #[repr(C)]
    struct Timespec {
        tv_sec: i64,
        tv_nsec: i64,
    }
    let mut ts: Timespec = unsafe { core::mem::zeroed() };
    let ret = syscall2(SYS_CLOCK_GETTIME, 0, &mut ts as *mut Timespec as usize);
    if ret == 0 {
        (ts.tv_sec, ts.tv_nsec)
    } else {
        (0, 0)
    }
}

// Sysinfo structure
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

// Get sysinfo
fn sysinfo() -> SysInfo {
    let mut info: SysInfo = unsafe { core::mem::zeroed() };
    let _ = syscall1(SYS_SYSINFO, &mut info as *mut SysInfo as usize);
    info
}

// Rusage structure
#[repr(C)]
struct RUsage {
    ru_utime: TimeVal2,
    ru_stime: TimeVal2,
    ru_maxrss: i64,
    ru_ixrss: i64,
    ru_idrss: i64,
    ru_isrss: i64,
    ru_minflt: i64,
    ru_majflt: i64,
    ru_nswap: i64,
    ru_inblock: i64,
    ru_oublock: i64,
    ru_msgsnd: i64,
    ru_msgrcv: i64,
    ru_nsignals: i64,
    ru_nvcsw: i64,
    ru_nivcsw: i64,
}

#[repr(C)]
struct TimeVal2 {
    tv_sec: i64,
    tv_usec: i64,
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

// String compare
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

// String copy
fn strcpy(dest: *mut u8, src: *const u8) {
    let mut i = 0;
    loop {
        let c = unsafe { *src.add(i) };
        unsafe { *dest.add(i) = c };
        if c == 0 {
            break;
        }
        i += 1;
    }
}

// Print a number
fn print_num(n: i64) {
    if n == 0 {
        putc(b'0');
        return;
    }
    let mut buf = [0u8; 24];
    let mut len = 0;
    let mut x = n;
    let negative = x < 0;
    if negative {
        x = -x;
    }
    while x > 0 {
        buf[len] = b'0' + (x % 10) as u8;
        len += 1;
        x /= 10;
    }
    if negative {
        putc(b'-');
    }
    for i in 0..len {
        putc(buf[len - 1 - i]);
    }
}

// Print unsigned number
fn print_num_u(n: u64) {
    if n == 0 {
        putc(b'0');
        return;
    }
    let mut buf = [0u8; 24];
    let mut len = 0;
    let mut x = n;
    while x > 0 {
        buf[len] = b'0' + (x % 10) as u8;
        len += 1;
        x /= 10;
    }
    for i in 0..len {
        putc(buf[len - 1 - i]);
    }
}

// Print prompt
fn print_prompt() {
    write_str(STDOUT, b"trainOS:~$ ".as_ptr() as *const u8);
}

// Print prompt with cwd
fn print_prompt_with_cwd() {
    let mut cwd = [0u8; 256];
    let ret = getcwd(cwd.as_mut_ptr(), 256);
    if ret > 0 {
        write_str(STDOUT, cwd.as_ptr());
        write_str(STDOUT, b" $ ".as_ptr() as *const u8);
    } else {
        write_str(STDOUT, b"trainOS:~$ ".as_ptr() as *const u8);
    }
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

// Get current working directory
fn builtin_pwd() {
    let mut cwd = [0u8; 256];
    let ret = getcwd(cwd.as_mut_ptr(), 256);
    if ret > 0 {
        write_str(STDOUT, cwd.as_ptr());
        putc(b'\n');
    } else {
        write_str(STDOUT, b"/\n".as_ptr() as *const u8);
    }
}

// Change directory
fn builtin_cd(args: &[*const u8]) {
    let dir = if args.len() > 1 {
        args[1]
    } else {
        b"/\0".as_ptr() as *const u8
    };
    let ret = chdir(dir);
    if ret < 0 {
        write_str(STDOUT, b"cd: no such directory\n".as_ptr() as *const u8);
    }
}

// Echo command
fn builtin_echo(args: &[*const u8]) {
    let mut i = 1;
    while i < args.len() {
        write_str(STDOUT, args[i]);
        if i < args.len() - 1 {
            putc(b' ');
        }
        i += 1;
    }
    putc(b'\n');
}

// Whoami command
fn builtin_whoami() {
    let uid = getuid();
    let euid = geteuid();
    let gid = getgid();
    let egid = getegid();
    write_str(STDOUT, b"uid=" .as_ptr() as *const u8);
    print_num(uid as i64);
    write_str(STDOUT, b" gid=" .as_ptr() as *const u8);
    print_num(gid as i64);
    write_str(STDOUT, b" euid=" .as_ptr() as *const u8);
    print_num(euid as i64);
    write_str(STDOUT, b" egid=" .as_ptr() as *const u8);
    print_num(egid as i64);
    putc(b'\n');
}

// Id command
fn builtin_id() {
    builtin_whoami();
}

// Date command
fn builtin_date() {
    let (sec, usec) = gettimeofday();
    write_str(STDOUT, b"Date: ".as_ptr() as *const u8);
    print_num(sec);
    write_str(STDOUT, b"." .as_ptr() as *const u8);
    print_num(usec / 1000);
    write_str(STDOUT, b" UTC\n".as_ptr() as *const u8);
}

// Uptime command
fn builtin_uptime() {
    let info = sysinfo();
    write_str(STDOUT, b" .uptime:  ".as_ptr() as *const u8);
    print_num(info.uptime);
    write_str(STDOUT, b" seconds\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  procs:   ".as_ptr() as *const u8);
    print_num_u(info.procs as u64);
    write_str(STDOUT, b"\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  load avg: ".as_ptr() as *const u8);
    print_num(info.loads[0] / 100);
    putc(b' ');
    print_num(info.loads[1] / 100);
    putc(b' ');
    print_num(info.loads[2] / 100);
    putc(b'\n');
}

// Memory info
fn builtin_free() {
    let info = sysinfo();
    write_str(STDOUT, b"              total        used        free\n".as_ptr() as *const u8);
    write_str(STDOUT, b"Mem:  ".as_ptr() as *const u8);
    print_num_u(info.totalram / 1024);
    write_str(STDOUT, b"  ".as_ptr() as *const u8);
    print_num_u((info.totalram - info.freeram) / 1024);
    write_str(STDOUT, b"  ".as_ptr() as *const u8);
    print_num_u(info.freeram / 1024);
    write_str(STDOUT, b"\nSwap: ".as_ptr() as *const u8);
    print_num_u(info.totalswap / 1024);
    write_str(STDOUT, b"  ".as_ptr() as *const u8);
    print_num_u((info.totalswap - info.freeswap) / 1024);
    write_str(STDOUT, b"  ".as_ptr() as *const u8);
    print_num_u(info.freeswap / 1024);
    putc(b'\n');
}

// Process info
fn builtin_ps() {
    let pid = getpid();
    let tid = gettid();
    let ppid = getppid();
    let uid = getuid();
    let info = sysinfo();

    write_str(STDOUT, b"  PID  TID  PPID  UID  S  COMMAND\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  ".as_ptr() as *const u8);
    print_num(pid as i64);
    write_str(STDOUT, b"  ".as_ptr() as *const u8);
    print_num(tid as i64);
    write_str(STDOUT, b"  ".as_ptr() as *const u8);
    print_num(ppid as i64);
    write_str(STDOUT, b"   ".as_ptr() as *const u8);
    print_num(uid as i64);
    write_str(STDOUT, b"  R  shell\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  procs: ".as_ptr() as *const u8);
    print_num_u(info.procs as u64);
    putc(b'\n');
}

// Cat command - read and display file
fn builtin_cat(args: &[*const u8]) {
    if args.len() < 2 {
        write_str(STDOUT, b"cat: missing file operand\n".as_ptr() as *const u8);
        return;
    }

    let mut i = 1;
    while i < args.len() {
        let path = args[i];
        let fd = openat(-100, path, O_RDONLY, 0);
        if fd < 0 {
            write_str(STDOUT, b"cat: ".as_ptr() as *const u8);
            write_str(STDOUT, path);
            write_str(STDOUT, b": No such file\n".as_ptr() as *const u8);
            i += 1;
            continue;
        }

        let mut buf = [0u8; 512];
        loop {
            let n = read(fd as usize, buf.as_mut_ptr(), 512);
            if n == 0 {
                break;
            }
            if n > 0 {
                let mut j = 0;
                while j < n {
                    putc(buf[j]);
                    j += 1;
                }
            }
            if n < 512 {
                break;
            }
        }
        let _ = close(fd as usize);
        i += 1;
    }
}

// Help command
fn builtin_help() {
    write_str(STDOUT, b"\nAvailable commands:\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  cd, pwd       - Change/show directory\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  echo          - Print text\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  cat           - Display file contents\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  ls            - List directory\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  mkdir         - Create directory\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  ps            - Show processes\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  free          - Show memory info\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  uptime        - Show system uptime\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  date          - Show current date/time\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  whoami, id    - Show user info\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  uname         - Show system info\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  clear         - Clear screen\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  exit          - Exit shell\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  help          - Show this help\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  ver           - Show OS version\n".as_ptr() as *const u8);
    putc(b'\n');
}

// Version
fn builtin_ver() {
    write_str(STDOUT, b"TrainOS 0.1.0 (RISC-V)\n".as_ptr() as *const u8);
    write_str(STDOUT, b"Built for educational purposes\n".as_ptr() as *const u8);
}

// Uname command
fn builtin_uname() {
    // Simplified uname - just print system info
    write_str(STDOUT, b"TrainOS trainOS 0.1.0 RISC-V\n".as_ptr() as *const u8);
}

// Clear command
fn builtin_clear() {
    for _ in 0..40 {
        putc(b'\n');
    }
}

// Ls command (simple)
fn builtin_ls(args: &[*const u8]) {
    let show_hidden = args.len() > 1 && strcmp(args[1], b"-a\0".as_ptr() as *const u8);
    let dir = if args.len() > 1 && !show_hidden {
        args[1]
    } else {
        b".\0".as_ptr() as *const u8
    };

    // For now, just show a simple listing since we don't have a real filesystem
    write_str(STDOUT, b".\n".as_ptr() as *const u8);
    write_str(STDOUT, b"..\n".as_ptr() as *const u8);
    write_str(STDOUT, b"bin\n".as_ptr() as *const u8);
    write_str(STDOUT, b"dev\n".as_ptr() as *const u8);
    write_str(STDOUT, b"etc\n".as_ptr() as *const u8);
    write_str(STDOUT, b"home\n".as_ptr() as *const u8);
    write_str(STDOUT, b"usr\n".as_ptr() as *const u8);
    write_str(STDOUT, b"var\n".as_ptr() as *const u8);
    write_str(STDOUT, b"tmp\n".as_ptr() as *const u8);
}

// Mkdir command
fn builtin_mkdir(args: &[*const u8]) {
    if args.len() < 2 {
        write_str(STDOUT, b"mkdir: missing operand\n".as_ptr() as *const u8);
        return;
    }
    // In a real implementation, this would call mkdirat
    write_str(STDOUT, b"mkdir: not implemented yet\n".as_ptr() as *const u8);
}

// Count arguments in command line
fn count_args(buf: *const u8) -> usize {
    let mut argc = 0;
    let mut i = 0;
    let len = strlen(buf);

    // Skip leading spaces
    while i < len && unsafe { *buf.add(i) } == b' ' {
        i += 1;
    }

    if i >= len {
        return 0;
    }

    while i < len {
        // Find end of arg
        while i < len && unsafe { *buf.add(i) } != b' ' {
            i += 1;
        }
        argc += 1;

        // Skip spaces
        while i < len && unsafe { *buf.add(i) } == b' ' {
            i += 1;
        }
    }
    argc
}

// Parse command line into arguments
fn parse_args(buf: *const u8, args: &mut [*const u8; 16]) -> usize {
    let mut argc = 0;
    let mut i = 0;
    let len = strlen(buf);

    // Skip leading spaces
    while i < len && unsafe { *buf.add(i) } == b' ' {
        i += 1;
    }

    while i < len && argc < 16 {
        let start = i;

        // Find end of arg
        while i < len && unsafe { *buf.add(i) } != b' ' {
            i += 1;
        }

        // Null-terminate and save
        if i > start {
            args[argc] = unsafe { buf.add(start) };
            argc += 1;
        }

        // Skip spaces
        while i < len && unsafe { *buf.add(i) } == b' ' {
            i += 1;
        }
    }
    argc
}

// Execute command
fn execute(cmd_buf: *const u8) {
    let argc = count_args(cmd_buf);
    if argc == 0 {
        return;
    }

    // Parse args
    let mut args: [*const u8; 16] = [core::ptr::null(); 16];
    parse_args(cmd_buf, &mut args);

    // Get command (first arg)
    let cmd = args[0];

    // Compare and execute built-in commands
    if strcmp(cmd, b"help\0".as_ptr() as *const u8) {
        builtin_help();
    } else if strcmp(cmd, b"exit\0".as_ptr() as *const u8) {
        write_str(STDOUT, b"Goodbye!\n".as_ptr() as *const u8);
        exit(0);
    } else if strcmp(cmd, b"clear\0".as_ptr() as *const u8) {
        builtin_clear();
    } else if strcmp(cmd, b"echo\0".as_ptr() as *const u8) {
        builtin_echo(&args);
    } else if strcmp(cmd, b"pwd\0".as_ptr() as *const u8) {
        builtin_pwd();
    } else if strcmp(cmd, b"cd\0".as_ptr() as *const u8) {
        builtin_cd(&args);
    } else if strcmp(cmd, b"whoami\0".as_ptr() as *const u8) {
        builtin_whoami();
    } else if strcmp(cmd, b"id\0".as_ptr() as *const u8) {
        builtin_id();
    } else if strcmp(cmd, b"date\0".as_ptr() as *const u8) {
        builtin_date();
    } else if strcmp(cmd, b"uptime\0".as_ptr() as *const u8) {
        builtin_uptime();
    } else if strcmp(cmd, b"free\0".as_ptr() as *const u8) {
        builtin_free();
    } else if strcmp(cmd, b"ps\0".as_ptr() as *const u8) {
        builtin_ps();
    } else if strcmp(cmd, b"cat\0".as_ptr() as *const u8) {
        builtin_cat(&args);
    } else if strcmp(cmd, b"ls\0".as_ptr() as *const u8) {
        builtin_ls(&args);
    } else if strcmp(cmd, b"mkdir\0".as_ptr() as *const u8) {
        builtin_mkdir(&args);
    } else if strcmp(cmd, b"uname\0".as_ptr() as *const u8) {
        builtin_uname();
    } else if strcmp(cmd, b"ver\0".as_ptr() as *const u8) {
        builtin_ver();
    } else if strcmp(cmd, b"pid\0".as_ptr() as *const u8) {
        write_str(STDOUT, b"PID: ".as_ptr() as *const u8);
        print_num(getpid() as i64);
        putc(b'\n');
    } else if strcmp(cmd, b"tid\0".as_ptr() as *const u8) {
        write_str(STDOUT, b"TID: ".as_ptr() as *const u8);
        print_num(gettid() as i64);
        putc(b'\n');
    } else if strcmp(cmd, b"ppid\0".as_ptr() as *const u8) {
        write_str(STDOUT, b"PPID: ".as_ptr() as *const u8);
        print_num(getppid() as i64);
        putc(b'\n');
    } else if strcmp(cmd, b"yield\0".as_ptr() as *const u8) {
        let ret = sched_yield();
        write_str(STDOUT, b"yield returned: ".as_ptr() as *const u8);
        print_num(ret as i64);
        putc(b'\n');
    } else {
        write_str(STDOUT, b"Unknown command: ".as_ptr() as *const u8);
        write_str(STDOUT, cmd);
        write_str(STDOUT, b"\nType 'help' for available commands.\n".as_ptr() as *const u8);
    }
}

// Main shell loop
#[no_mangle]
extern "C" fn _start() {
    write_str(STDOUT, b"\n".as_ptr() as *const u8);
    write_str(STDOUT, b"========================================\n".as_ptr() as *const u8);
    write_str(STDOUT, b"  Welcome to TrainOS Shell\n".as_ptr() as *const u8);
    write_str(STDOUT, b"========================================\n".as_ptr() as *const u8);
    write_str(STDOUT, b"Type 'help' for available commands.\n".as_ptr() as *const u8);

    // Command buffer
    let mut cmd_buf = [0u8; 512];

    loop {
        print_prompt();

        // Read command
        let _cmd_len = read_line(cmd_buf.as_mut_ptr(), 512);

        // Execute command
        execute(cmd_buf.as_ptr());
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    write_str(STDERR, b"\nPanic in shell!\n".as_ptr() as *const u8);
    exit(1);
}
