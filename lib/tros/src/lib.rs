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
    }
    result
}

/// Receive a message from an endpoint (syscall 12).
/// Copies payload into buf (up to buf.len() bytes).
/// Returns (sender_pid, opcode) on success, (usize::MAX, 0) on error.
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
