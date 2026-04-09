//! Init service - first user-space process
//!
//! In a full implementation, this would spawn driver and fs services.

#![no_std]
#![no_main]

// Syscall numbers (Linux-compatible RISC-V)
const SYS_WRITE: usize = 64;
const SYS_EXIT: usize = 93;

// File descriptor constants
const STDOUT: usize = 1;

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

/// Write to stdout
fn write(fd: usize, buf: *const u8, count: usize) -> usize {
    syscall3(SYS_WRITE, fd, buf as usize, count)
}

/// Write string to stdout
fn write_str(s: &[u8]) {
    write(STDOUT, s.as_ptr(), s.len());
}

/// Exit
fn exit(code: usize) -> ! {
    syscall1(SYS_EXIT, code);
    loop {}
}

/// Putchar via write syscall
fn putchar(c: u8) {
    let _ = write(STDOUT, &c, 1);
}

/// Print string
fn print(s: &str) {
    for b in s.bytes() {
        putchar(b);
        if b == b'\n' {
            putchar(b'\r');
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() {
    print("init: TrainOS microkernel init started\n");

    // In a full implementation:
    // 1. Create endpoint for driver service
    // 2. Spawn driver service process
    // 3. Wait for driver to initialize
    // 4. Create endpoint for fs service
    // 5. Spawn fs service
    // 6. Wait for fs to initialize
    // 7. Spawn shell

    print("init: Placeholder - services will be added in Phase 2\n");

    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    write_str(b"\nPanic in init service!\n");
    exit(1);
}
