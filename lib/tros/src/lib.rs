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
