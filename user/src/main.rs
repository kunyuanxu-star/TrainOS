//! Hello world user program for trainOS

#![no_std]
#![no_main]

// Syscall numbers
const SYS_WRITE: usize = 1;
const SYS_EXIT: usize = 7;
const SYS_GETPID: usize = 8;

/// Call a syscall with 3 arguments
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

/// Write to a file descriptor (typically stdout = 1)
fn write(fd: usize, buf: *const u8, count: usize) -> usize {
    syscall3(SYS_WRITE, fd, buf as usize, count)
}

/// Exit the current process
fn exit(code: usize) -> ! {
    syscall3(SYS_EXIT, code, 0, 0);
    loop {}
}

/// Get current process ID
fn getpid() -> usize {
    syscall3(SYS_GETPID, 0, 0, 0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

#[no_mangle]
extern "C" fn main() {
    let msg = b"Hello from trainOS!\n";
    write(1, msg.as_ptr(), msg.len());
    let _pid = getpid();
    // Exit with code 42 to indicate success
    exit(42);
}
