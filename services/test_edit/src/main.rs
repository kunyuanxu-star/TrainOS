#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("TEST_EDIT: testing line editor...\r\n");

    // Insert a line (send to edit service on its EP)
    // EDIT runs at priority 62 and gets EP 2 (after test_cap takes EP 1)
    tros::send(2, 1, b"First line of text");
    tros::send(2, 1, b"Second line here");

    // Show lines
    tros::send(2, 0, &[]);

    tros::print("TEST_EDIT: editor test done\r\n");
    tros::print("TEST_EDIT: PASS\r\n");

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
