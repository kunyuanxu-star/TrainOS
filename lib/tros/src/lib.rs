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
    while i < s.len() && s[i] != 0 { i += 1; }
    i
}

/// memcpy
pub fn memcpy(dst: &mut [u8], src: &[u8], n: usize) {
    for i in 0..n { dst[i] = src[i]; }
}

/// memset
pub fn memset(buf: &mut [u8], val: u8, n: usize) {
    for i in 0..n { buf[i] = val; }
}

/// Simple sprintf-like: format a number into a buffer. Returns bytes written.
pub fn format_uint(mut n: usize, buf: &mut [u8]) -> usize {
    let mut i = buf.len();
    if n == 0 {
        i -= 1; buf[i] = b'0';
        return i;
    }
    loop {
        i -= 1;
        buf[i] = b'0' + (n - (n / 10) * 10) as u8;
        n = n / 10;
        if n == 0 { break; }
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
            if val == 0 { break; }
        }
    }
    for j in idx..20 { putchar(buf[j]); }
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
        let c = if nibble < 10 { b'0' + nibble as u8 } else { b'a' + (nibble - 10) as u8 };
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
        if aligned + size > 4096 { return core::ptr::null_mut(); }
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
pub fn exit(_code: i32) -> ! {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 0usize,
            in("a0") 0usize,
        );
    }
    loop { unsafe { core::arch::asm!("wfi"); } }
}

/// POSIX-compatible system calls.
/// These use the kernel's POSIX syscalls (50-53) which translate to IPC internally.

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
