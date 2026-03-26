//! User programs entry
//!
//! This is a placeholder for user programs

#![no_std]
#![no_main]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
