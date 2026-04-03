//! Minimal test user program - just exits immediately
#![no_std]
#![no_main]

const SYS_EXIT: usize = 93;

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

fn exit(code: usize) -> ! {
    syscall1(SYS_EXIT, code);
    loop {}
}

#[no_mangle]
extern "C" fn _start() {
    // Immediately exit with code 42
    exit(42);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
