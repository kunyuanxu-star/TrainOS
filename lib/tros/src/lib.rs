#![no_std]

/// SBI console putchar (syscall 1, forwarded to M-mode)
pub fn putchar(c: u8) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 1usize,
            in("a0") c as usize,
        );
    }
}

// ====== Mini C Library ======

/// strlen — count bytes until null
pub fn strlen(s: &[u8]) -> usize {
    let mut i = 0;
    while i < s.len() && s[i] != 0 {
        i += 1;
    }
    i
}

/// memcpy
pub fn memcpy(dst: &mut [u8], src: &[u8], n: usize) {
    for i in 0..n {
        dst[i] = src[i];
    }
}

/// memset
pub fn memset(buf: &mut [u8], val: u8, n: usize) {
    for i in 0..n {
        buf[i] = val;
    }
}

/// Simple sprintf-like: format a number into a buffer. Returns bytes written.
pub fn format_uint(mut n: usize, buf: &mut [u8]) -> usize {
    let mut i = buf.len();
    if n == 0 {
        i -= 1;
        buf[i] = b'0';
        return i;
    }
    loop {
        i -= 1;
        buf[i] = b'0' + (n - (n / 10) * 10) as u8;
        n = n / 10;
        if n == 0 {
            break;
        }
    }
    i
}

/// Print a 64-bit unsigned integer to console.
pub fn print_uint(mut val: usize) {
    let mut buf = [0u8; 20];
    let mut idx = 20;
    if val == 0 {
        idx = 19;
        buf[19] = 48;
    } else {
        loop {
            idx -= 1;
            buf[idx] = 48 + (val - (val / 10) * 10) as u8;
            val /= 10;
            if val == 0 {
                break;
            }
        }
    }
    for j in idx..20 {
        putchar(buf[j]);
    }
}

/// Print a string with an unsigned integer argument.
/// Format: "text %u more_text"
/// Only supports a single %u specifier.
/// NOTE: %u substitution may not work in release mode due to LLVM SWAR optimization bug.
/// Use print_uint() directly for reliable number printing.
pub fn printf(fmt: &str, arg: usize) {
    let b = fmt.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 1 < b.len() && b[i + 1] == b'u' {
            print_uint(arg);
            i += 2;
        } else {
            putchar(b[i]);
            i += 1;
        }
    }
}

/// Print hex value
pub fn print_hex(val: usize) {
    for i in (0..16).rev() {
        let nibble = (val >> (i * 4)) & 0xF;
        let c = if nibble < 10 {
            b'0' + nibble as u8
        } else {
            b'a' + (nibble - 10) as u8
        };
        putchar(c);
    }
}

/// Simple heap: static bump allocator for user-space
/// Returns aligned pointer (8-byte). Returns pointer to static buffer, or null.
static mut HEAP: [u8; 4096] = [0; 4096];
static mut HEAP_OFFSET: usize = 0;

pub fn malloc(size: usize) -> *mut u8 {
    unsafe {
        let aligned = (HEAP_OFFSET + 7) & !7;
        if aligned + size > 4096 {
            return core::ptr::null_mut();
        }
        HEAP_OFFSET = aligned + size;
        HEAP.as_mut_ptr().add(aligned)
    }
}

pub fn free(_ptr: *mut u8) {
    // Bump allocator: no-op
}

/// Read a character from console (SBI getchar, syscall 2)
/// Returns character byte, or usize::MAX if no input available
pub fn getchar() -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 2usize,
            lateout("a0") result,
        );
    }
    result
}

/// Print a string via putchar
pub fn print(s: &str) {
    for b in s.bytes() {
        putchar(b);
    }
}

/// Create an IPC endpoint (syscall 10)
pub fn ep_create() -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 10usize,
            lateout("a0") result,
        );
    }
    result
}

/// Send a message to an endpoint (syscall 11)
/// Returns 0 on success
#[inline(never)]
pub fn send(ep_id: usize, opcode: u16, data: &[u8]) -> usize {
    let mut result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 11usize,
            inout("a0") ep_id => result,
            in("a1") opcode as usize,
            in("a2") data.as_ptr() as usize,
            in("a3") data.len(),
        );
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
    result
}

/// Receive a message from an endpoint (syscall 12).
/// Copies payload into buf (up to buf.len() bytes).
/// Returns (sender_pid, opcode) on success, (usize::MAX, 0) on error.
#[inline(never)]
pub fn recv(ep_id: usize, buf: &mut [u8]) -> (usize, u16) {
    let mut result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 12usize,
            inout("a0") ep_id => result,
            in("a1") buf.as_ptr() as usize,
            in("a2") buf.len(),
        );
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
    if result == usize::MAX {
        return (usize::MAX, 0);
    }
    let opcode = ((result >> 24) & 0xFF) as u16;
    let sender_pid = result & 0x00FF_FFFF;
    (sender_pid, opcode)
}

/// Get the current process ID (syscall 5)
pub fn getpid() -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 5usize,
            lateout("a0") result,
        );
    }
    result
}

/// Voluntarily yield the CPU (syscall 6).
/// The thread stays ready but lets other threads run.
pub fn yield_cpu() {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 6usize,
        );
    }
}

/// Map a physical MMIO region into process address space (syscall 22).
/// Returns virtual address, or 0 on error.
pub fn map_mmio(phys: usize, size: usize) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 22usize,
            inout("a0") phys => result,
            in("a1") size,
        );
    }
    result
}

/// Map a physical MMIO region into process address space (syscall 20).
/// Returns virtual address of the mapping.
pub fn mmio_map(phys: usize, size: usize) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 20usize,
            in("a0") phys,
            in("a1") size,
            lateout("a0") result,
        );
    }
    result
}

/// Read a 32-bit value from a physical MMIO address via kernel proxy (syscall 23).
/// The kernel reads the MMIO register in S-mode on behalf of user-space.
pub fn mmio_read32(phys: usize) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 23usize,
            in("a0") phys,
            lateout("a0") result,
        );
    }
    result
}

/// Write a 32-bit value to a physical MMIO address via kernel proxy (syscall 24).
pub fn mmio_write32(phys: usize, val: usize) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 24usize,
            in("a0") phys,
            in("a1") val,
        );
    }
}

/// Fork the current process (syscall 4).
/// Returns child PID in parent, 0 in child.
pub fn fork() -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 4usize,
            lateout("a0") result,
        );
    }
    result
}

/// Exit current process (syscall 0)
/// Spawn a new process from ELF data (syscall 3).
pub fn spawn(elf_data: &[u8]) -> usize {
    let r: usize;
    unsafe {
        core::arch::asm!("ecall",
            in("a7") 3usize,
            in("a0") elf_data.as_ptr() as usize,
            in("a1") elf_data.len(),
            lateout("a0") r,
        );
    }
    r
}

/// Execute a new program, replacing the current process (syscall 7).
/// Path is looked up in the VFS.
pub fn exec(path: &str) -> usize {
    let r: usize;
    unsafe {
        core::arch::asm!("ecall",
            in("a7") 7usize,
            in("a0") path.as_ptr() as usize,
            lateout("a0") r,
        );
    }
    r
}

pub fn exit(_code: i32) -> ! {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 0usize,
            in("a0") 0usize,
        );
    }
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// Load and execute an ELF binary from disk (syscall 7).
/// The path format is "/sector/N" where N is a disk sector number.
/// Returns the new PID on success, or usize::MAX on error.

/// POSIX-compatible system calls.
/// These use the kernel's POSIX syscalls (50-53) which translate to IPC internally.

/// Open a file from a byte slice (syscall 50). Returns fd number.
/// Appends a null terminator for the kernel.
pub fn open_bytes(path: &[u8]) -> usize {
    let mut buf = [0u8; 32];
    let len = path.len().min(31);
    for i in 0..len { buf[i] = path[i]; }
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 50usize,
            in("a0") buf.as_ptr() as usize,
            in("a1") 0usize,
            in("a2") 0usize,
            lateout("a0") result,
        );
    }
    result
}

/// Open a file (syscall 50). Returns fd number.
pub fn open(path: &str) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 50usize,
            in("a0") path.as_ptr() as usize,
            in("a1") 0usize,
            in("a2") 0usize,
            lateout("a0") result,
        );
    }
    result
}

/// Read from a file descriptor (syscall 51). Returns bytes read.
pub fn read(fd: usize, buf: &mut [u8]) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 51usize,
            in("a0") fd,
            in("a1") buf.as_ptr() as usize,
            in("a2") buf.len(),
            lateout("a0") result,
        );
    }
    result
}

/// Write to a file descriptor (syscall 52). Returns bytes written.
pub fn write(fd: usize, data: &[u8]) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 52usize,
            in("a0") fd,
            in("a1") data.as_ptr() as usize,
            in("a2") data.len(),
            lateout("a0") result,
        );
    }
    result
}

/// Close a file descriptor (syscall 53).
pub fn close(fd: usize) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 53usize,
            in("a0") fd,
            lateout("a0") result,
        );
    }
    result
}

/// stat(fd, buf) — get file status (syscall 54).
pub fn stat(_fd: usize, buf: &mut [u8]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 54usize, in("a0") 0usize, in("a1") buf.as_ptr() as usize, lateout("a0") r); }
    r
}

/// lseek(fd, offset, whence) — reposition read/write offset (syscall 55).
pub fn lseek(fd: usize, offset: isize, whence: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 55usize, in("a0") fd, in("a1") offset as usize, in("a2") whence, lateout("a0") r); }
    r
}

/// dup(fd) — duplicate a file descriptor (syscall 56).
pub fn dup(fd: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 56usize, in("a0") fd, lateout("a0") r); }
    r
}

/// getcwd(buf) — get current working directory (syscall 57).
pub fn getcwd(buf: &mut [u8]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 57usize, in("a0") buf.as_ptr() as usize, in("a1") buf.len(), lateout("a0") r); }
    r
}

/// Read a disk sector from the VirtIO block device (syscall 40).
/// sector: logical block address (512-byte units)
/// buf: mutable buffer to receive data (must be >= 512 bytes)
/// Returns: number of bytes read
pub fn blk_read(sector: usize, buf: &mut [u8]) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 40usize,
            in("a0") sector,
            in("a1") buf.as_ptr() as usize,
            in("a2") buf.len(),
            lateout("a0") result,
        );
    }
    result
}

/// Write a disk sector to the VirtIO block device (syscall 45).
/// sector: logical block address (512-byte units)
/// data: buffer with data to write (must be >= 512 bytes)
/// Returns: number of bytes written
pub fn blk_write(sector: usize, data: &[u8]) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 45usize,
            in("a0") sector,
            in("a1") data.as_ptr() as usize,
            in("a2") data.len(),
            lateout("a0") result,
        );
    }
    result
}

/// Query process list (syscall 41).
/// Fills buf with process info. Returns number of processes written.
pub fn proclist(buf: &mut [u8]) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 41usize,
            in("a0") buf.as_ptr() as usize,
            in("a1") buf.len(),
            lateout("a0") result,
        );
    }
    result
}

/// Kill a process by PID (syscall 42). Returns 0 on success.
pub fn kill(pid: u32) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 42usize,
            in("a0") pid as usize,
            lateout("a0") result,
        );
    }
    result
}

/// Query memory allocation info (syscall 43).
/// Returns the number of allocated pages.
pub fn meminfo() -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 43usize,
            lateout("a0") result,
        );
    }
    result
}

/// Delete a capability from the calling process's CNode (syscall 33).
/// slot: index of the capability slot to delete.
/// Returns 0 on success.
pub fn cap_delete(slot: usize) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 33usize,
            in("a0") slot,
            lateout("a0") result,
        );
    }
    result
}

/// Returns capability statistics for the calling process (syscall 34).
/// Returns (total_slots, used_slots, ep_caps, mem_caps).
pub fn cap_stats() -> (usize, usize, usize, usize) {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 34usize,
            lateout("a0") result,
        );
    }
    let total = result & 0xFFFF;
    let used = (result >> 16) & 0xFFFF;
    let ep = (result >> 32) & 0xFFFF;
    let mem = (result >> 48) & 0xFFFF;
    (total, used, ep, mem)
}

/// Returns system uptime in milliseconds (syscall 46).
/// Each tick is 10ms, so we multiply ticks by 10.
pub fn uptime_ms() -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 46usize,
            lateout("a0") result,
        );
    }
    result * 10 // ticks * 10ms per tick
}

/// Get the current process's user ID (syscall 60).
pub fn getuid() -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 60usize, lateout("a0") r); }
    r
}

/// Set the current process's user ID (syscall 61). Only root (uid=0) can change UID.
pub fn setuid(uid: u32) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 61usize, in("a0") uid as usize, lateout("a0") r); }
    r
}

/// Change file permissions (syscall 62). Simplified: always succeeds for root.
pub fn chmod(_path: &str, _mode: u16) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 62usize, in("a0") 0usize, in("a1") 0usize, lateout("a0") r); }
    r
}

/// Register a signal handler (syscall 63).
/// Returns 0 on success.
pub fn signal(sig: u32, handler: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 63usize, in("a0") sig as usize, in("a1") handler, lateout("a0") r); }
    r
}

/// Wait for a child process to exit (syscall 64).
/// pid == -1: wait for any child; pid > 0: wait for specific child.
/// Returns child pid, or 0 if no dead child yet.
pub fn waitpid(pid: i32, status: &mut i32, options: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 64usize, in("a0") pid as usize, in("a1") status as *const i32 as usize, in("a2") options, lateout("a0") r); }
    r
}

/// Map a shared memory page into another process (syscall 25).
/// Shares the current process's page at vaddr with target_pid.
/// Returns the shared virtual address in the target process.
pub fn shm_map(target_pid: u32, vaddr: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 25usize, in("a0") target_pid as usize, in("a1") vaddr, lateout("a0") r); }
    r
}

/// Returns performance counters: (send_count, recv_count, ctx_switch_count) (syscall 44).
pub fn perf_stats() -> (usize, usize, usize) {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 44usize,
            lateout("a0") result,
        );
    }
    let sends = result & 0xFFFFF;
    let recvs = (result >> 20) & 0xFFFFF;
    let ctx = (result >> 40) & 0xFFFFFF;
    (sends, recvs, ctx)
}

// ── V14.0 Extended Syscalls ──────────────────────────────────────────────────

/// Get parent process ID (syscall 65).
pub fn getppid() -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 65usize, lateout("a0") r); }
    r
}

/// Get thread ID (syscall 66).
pub fn gettid() -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 66usize, lateout("a0") r); }
    r
}

/// Nanosleep (syscall 67).
pub fn nanosleep(seconds: u64, nanoseconds: u64) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 67usize, in("a0") seconds as usize, in("a1") nanoseconds as usize, lateout("a0") r); }
    r
}

/// clock_gettime(clk_id, ts_ptr) (syscall 68).
pub fn clock_gettime(clk_id: usize, ts: &mut [u64; 2]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 68usize, in("a0") clk_id, in("a1") ts.as_ptr() as usize, lateout("a0") r); }
    r
}

/// Set umask (syscall 69).
pub fn umask(mask: u16) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 69usize, in("a0") mask as usize, lateout("a0") r); }
    r
}

/// Create new session (syscall 70).
pub fn setsid() -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 70usize, lateout("a0") r); }
    r
}

/// Get system info (syscall 71).
pub fn sysinfo(buf: &mut [u8]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 71usize, in("a0") buf.as_ptr() as usize, lateout("a0") r); }
    r
}

/// Create a pipe (syscall 72). fds[0]=read end, fds[1]=write end.
pub fn pipe(fds: &mut [u32; 2]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 72usize, in("a0") fds.as_ptr() as usize, lateout("a0") r); }
    r
}

/// fcntl(fd, cmd, arg) (syscall 73).
pub fn fcntl(fd: usize, cmd: usize, arg: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 73usize, in("a0") fd, in("a1") cmd, in("a2") arg, lateout("a0") r); }
    r
}

/// ioctl(fd, request, arg) (syscall 74).
pub fn ioctl(fd: usize, req: usize, arg: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 74usize, in("a0") fd, in("a1") req, in("a2") arg, lateout("a0") r); }
    r
}

/// getdents64(fd, buf, len) (syscall 75).
pub fn getdents64(fd: usize, buf: &mut [u8]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 75usize, in("a0") fd, in("a1") buf.as_ptr() as usize, in("a2") buf.len(), lateout("a0") r); }
    r
}

/// mkdir(path, mode) (syscall 76).
pub fn mkdir(path: &str, mode: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 76usize, in("a0") path.as_ptr() as usize, in("a1") mode, lateout("a0") r); }
    r
}

/// rmdir(path) (syscall 77).
pub fn rmdir(path: &str) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 77usize, in("a0") path.as_ptr() as usize, lateout("a0") r); }
    r
}

/// unlink(path) (syscall 78).
pub fn unlink(path: &str) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 78usize, in("a0") path.as_ptr() as usize, lateout("a0") r); }
    r
}

/// rename(old, new) (syscall 79).
pub fn rename(old: &str, new: &str) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 79usize, in("a0") old.as_ptr() as usize, in("a1") new.as_ptr() as usize, lateout("a0") r); }
    r
}

/// chdir(path) (syscall 80).
pub fn chdir(path: &str) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 80usize, in("a0") path.as_ptr() as usize, lateout("a0") r); }
    r
}

/// access(path, mode) (syscall 81).
pub fn access(path: &str, mode: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 81usize, in("a0") path.as_ptr() as usize, in("a1") mode, lateout("a0") r); }
    r
}

/// truncate(path, length) (syscall 82).
pub fn truncate(path: &str, length: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 82usize, in("a0") path.as_ptr() as usize, in("a1") length, lateout("a0") r); }
    r
}

/// mmap(addr, length, prot, flags, fd, offset) (syscall 83).
pub fn mmap(addr: usize, length: usize, prot: usize, flags: usize, fd: usize, offset: usize) -> usize {
    let r: usize;
    unsafe {
        core::arch::asm!("ecall",
            in("a7") 83usize,
            in("a0") addr,
            in("a1") length,
            in("a2") prot,
            in("a3") flags,
            in("a4") fd,
            in("a5") offset,
            lateout("a0") r,
        );
    }
    r
}

/// munmap(addr, length) (syscall 84).
pub fn munmap(addr: usize, length: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 84usize, in("a0") addr, in("a1") length, lateout("a0") r); }
    r
}

/// mprotect(addr, length, prot) (syscall 85).
pub fn mprotect(addr: usize, length: usize, prot: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 85usize, in("a0") addr, in("a1") length, in("a2") prot, lateout("a0") r); }
    r
}

/// brk(addr) — set program break (syscall 86).
pub fn brk(addr: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 86usize, in("a0") addr, lateout("a0") r); }
    r
}

/// socket(domain, typ, proto) (syscall 90).
pub fn socket(domain: usize, typ: usize, proto: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 90usize, in("a0") domain, in("a1") typ, in("a2") proto, lateout("a0") r); }
    r
}

/// bind(fd, addr_ptr, addr_len) (syscall 91).
pub fn bind(fd: usize, addr: &[u8], addr_len: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 91usize, in("a0") fd, in("a1") addr.as_ptr() as usize, in("a2") addr_len, lateout("a0") r); }
    r
}

/// listen(fd, backlog) (syscall 92).
pub fn listen(fd: usize, backlog: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 92usize, in("a0") fd, in("a1") backlog, lateout("a0") r); }
    r
}

/// accept(fd) (syscall 93).
pub fn accept(fd: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 93usize, in("a0") fd, lateout("a0") r); }
    r
}

/// connect(fd, addr_ptr, addr_len) (syscall 94).
pub fn connect(fd: usize, addr: &[u8], addr_len: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 94usize, in("a0") fd, in("a1") addr.as_ptr() as usize, in("a2") addr_len, lateout("a0") r); }
    r
}

/// sendto(fd, buf, len, flags, addr, addr_len) (syscall 95).
pub fn sendto(fd: usize, buf: &[u8], len: usize, flags: usize, addr: &[u8], addr_len: usize) -> usize {
    let r: usize;
    unsafe {
        core::arch::asm!("ecall",
            in("a7") 95usize,
            in("a0") fd,
            in("a1") buf.as_ptr() as usize,
            in("a2") len,
            in("a3") flags,
            in("a4") addr.as_ptr() as usize,
            in("a5") addr_len,
            lateout("a0") r,
        );
    }
    r
}

/// recvfrom(fd, buf, len, flags, addr, addr_len_ptr) (syscall 96).
pub fn recvfrom(fd: usize, buf: &mut [u8], len: usize, flags: usize) -> usize {
    let r: usize;
    unsafe {
        core::arch::asm!("ecall",
            in("a7") 96usize,
            in("a0") fd,
            in("a1") buf.as_ptr() as usize,
            in("a2") len,
            in("a3") flags,
            lateout("a0") r,
        );
    }
    r
}

/// epoll_create(size) (syscall 100).
pub fn epoll_create(size: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 100usize, in("a0") size, lateout("a0") r); }
    r
}

/// epoll_ctl(epfd, op, fd, events) (syscall 101).
pub fn epoll_ctl(epfd: usize, op: usize, fd: usize, events: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 101usize, in("a0") epfd, in("a1") op, in("a2") fd, in("a3") events, lateout("a0") r); }
    r
}

/// epoll_wait(epfd, events_ptr, maxevents, timeout) (syscall 102).
pub fn epoll_wait(epfd: usize, events: &mut [u8], maxevents: usize, timeout: isize) -> usize {
    let r: usize;
    unsafe {
        core::arch::asm!("ecall",
            in("a7") 102usize,
            in("a0") epfd,
            in("a1") events.as_ptr() as usize,
            in("a2") maxevents,
            in("a3") timeout as usize,
            lateout("a0") r,
        );
    }
    r
}

// ── V15.0 Extended Syscalls ──────────────────────────────────────────────────

pub fn unshare(flags: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 110usize, in("a0") flags, lateout("a0") r); }
    r
}
pub fn sethostname(name: &[u8], len: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 111usize, in("a0") name.as_ptr() as usize, in("a1") len, lateout("a0") r); }
    r
}
pub fn gethostname(buf: &mut [u8], len: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 112usize, in("a0") buf.as_ptr() as usize, in("a1") len, lateout("a0") r); }
    r
}
pub fn setns(fd: usize, nstype: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 113usize, in("a0") fd, in("a1") nstype, lateout("a0") r); }
    r
}
pub fn sched_setaffinity(pid: usize, mask: &[u64]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 114usize, in("a0") pid, in("a1") 8usize, in("a2") mask.as_ptr() as usize, lateout("a0") r); }
    r
}
pub fn sched_getaffinity(pid: usize, mask: &mut [u64]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 115usize, in("a0") pid, in("a1") 8usize, in("a2") mask.as_ptr() as usize, lateout("a0") r); }
    r
}
pub fn times(buf: &mut [u64; 4]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 116usize, in("a0") buf.as_ptr() as usize, lateout("a0") r); }
    r
}
pub fn getrusage(who: usize, buf: &mut [u64; 4]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 117usize, in("a0") who, in("a1") buf.as_ptr() as usize, lateout("a0") r); }
    r
}
pub fn register_drv(name: &str, drv_type: usize, probe_ep: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 118usize, in("a0") name.as_ptr() as usize, in("a1") drv_type, in("a2") probe_ep, lateout("a0") r); }
    r
}
pub fn unregister_drv(drv_id: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 119usize, in("a0") drv_id, lateout("a0") r); }
    r
}
pub fn list_drvs(buf: &mut [u8]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 120usize, in("a0") buf.as_ptr() as usize, in("a1") buf.len(), lateout("a0") r); }
    r
}
pub fn sync() -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 121usize, lateout("a0") r); }
    r
}
pub fn reboot(magic: usize, cmd: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 122usize, in("a0") magic, in("a1") cmd, lateout("a0") r); }
    r
}

// ── V21 Security syscalls ────────────────────────────────────────────────────

/// seccomp_add(syscall_nr, action) — add seccomp rule (syscall 130).
/// action: 0=allow, 1=kill, 2=log
pub fn seccomp_add(syscall_nr: usize, action: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 130usize, in("a0") syscall_nr, in("a1") action, lateout("a0") r); }
    r
}

/// cap_audit(buf, len) — read capability audit log (syscall 131).
pub fn cap_audit(buf: &mut [u8]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 131usize, in("a0") buf.as_ptr() as usize, in("a1") buf.len(), lateout("a0") r); }
    r
}

// ── V22-V26 syscall wrappers ─────────────────────────────────────────────────

pub fn io_uring_setup(entries: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 140usize, in("a0") entries, lateout("a0") r); }
    r
}
pub fn io_uring_enter(ring_id: usize, to_submit: usize, min_complete: usize) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 141usize, in("a0") ring_id, in("a1") to_submit, in("a2") min_complete, lateout("a0") r); }
    r
}
pub fn vm_create(memory_mb: usize) -> usize {
    let r: usize; unsafe { core::arch::asm!("ecall", in("a7") 150usize, in("a0") memory_mb, lateout("a0") r); }
    r
}
pub fn vm_destroy(vm_id: u32) -> usize {
    let r: usize; unsafe { core::arch::asm!("ecall", in("a7") 151usize, in("a0") vm_id as usize, lateout("a0") r); }
    r
}
pub fn ext_register(hook_type: usize, bytecode: &[u8]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 160usize, in("a0") hook_type, in("a1") bytecode.as_ptr() as usize, lateout("a0") r); }
    r
}
pub fn numa_nodes(buf: &mut [u8]) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 170usize, in("a0") buf.as_ptr() as usize, in("a1") buf.len(), lateout("a0") r); }
    r
}
pub fn remote_node_add(ip: &[u8], port: u16) -> usize {
    let r: usize;
    unsafe { core::arch::asm!("ecall", in("a7") 180usize, in("a0") ip.as_ptr() as usize, in("a1") port as usize, lateout("a0") r); }
    r
}
