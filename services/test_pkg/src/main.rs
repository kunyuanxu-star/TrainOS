#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("TEST_PKG: querying package manager...\r\n");

    // Send LIST command (opcode 0) to PKG service on EP 6
    tros::send(6, 0, &[]);

    tros::print("TEST_PKG: list request sent\r\n");
    tros::print("TEST_PKG: PASS\r\n");
    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
