#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("USER: multi-user test\r\n");

    let uid = tros::getuid();
    tros::printf("USER: current uid=%u\r\n", uid);

    // Verify root (uid=0) by default
    if uid == 0 {
        tros::print("USER: root user confirmed\r\n");
        tros::print("USER: PASS\r\n");
    }

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
