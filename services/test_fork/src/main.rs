#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

fn spin(amount: usize) {
    for _ in 0..amount {
        unsafe { core::arch::asm!("nop"); }
    }
}

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("A");     // Before fork

    let child = tros::fork();   // returned child pid

    tros::print("B");     // After fork (should print in both parent and child)

    if child == 0 {
        tros::print("C");       // child path
    } else if child != usize::MAX {
        tros::print("P");       // parent path
    } else {
        tros::print("F");       // fail path
    }

    tros::print("D");

    // Spin for a while to let timer fire and other process run
    spin(5000000);

    tros::print("X\r\n"); // mark that we're done spinning

    loop { unsafe { core::arch::asm!("wfi"); } }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
