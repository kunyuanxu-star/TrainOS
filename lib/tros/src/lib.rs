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
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 11usize,
            in("a0") ep_id,
            in("a1") opcode as usize,
            in("a2") data.as_ptr() as usize,
            in("a3") data.len(),
            lateout("a0") result,
        );
    }
    result
}

/// Receive a message from an endpoint (syscall 12)
/// Returns sender_pid on success, blocks if no message
pub fn recv(ep_id: usize) -> usize {
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 12usize,
            in("a0") ep_id,
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
