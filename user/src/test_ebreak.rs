//! Test: ebreak syscall for putchar via kernel
#![no_std]
#![no_main]

#[no_mangle]
extern "C" fn _start() {
    // Putchar via ebreak (kernel handles syscall 1)
    unsafe {
        core::arch::asm!(
            "mv a7, {nr}",
            "mv a0, {ch}",
            "ebreak",
            nr = in(reg) 1u64,   // syscall 1 = putchar
            ch = in(reg) 88u64,  // 'X'
        );
    }
    // Loop forever
    loop {}
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
