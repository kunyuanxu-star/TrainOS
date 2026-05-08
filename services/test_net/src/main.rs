#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("TEST_NET: sending to port 7...\r\n");

    // Send a datagram to port 7 (echo) via NET EP 2
    // Format: [port_hi, port_lo, data_len, data...]
    let pkt = [0u8, 7u8, 5u8, b'h', b'e', b'l', b'l', b'o'];
    tros::send(2, 2, &pkt);

    tros::print("TEST_NET: sent to net service\r\n");

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
