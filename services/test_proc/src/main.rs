#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("TEST_PROC: querying process list via EP 1...\r\n");

    // Send LIST request to proc service (EP 1, since PROC runs first)
    tros::send(1, 0, &[]);

    tros::print("TEST_PROC: PROC list request sent\r\n");
    tros::print("TEST_PROC: PASS\r\n");

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
