#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("TEST_HTTP: sending GET to EP 8...\r\n");

    let reply_ep = tros::ep_create();

    // Build GET request with reply_ep in first 2 bytes (little-endian u16)
    let mut req = [0u8; 64];
    req[0] = reply_ep as u8;
    req[1] = (reply_ep >> 8) as u8;
    let get = b"GET / HTTP/1.0";
    for i in 0..get.len() { req[2 + i] = get[i]; }

    tros::send(8, 0, &req[..2 + get.len()]);
    tros::print("TEST_HTTP: GET sent\r\n");

    // Wait for response
    let mut resp = [0u8; 64];
    let (_sender, _op) = tros::recv(reply_ep, &mut resp);

    // Check for HTTP response
    if &resp[0..4] == b"HTTP" {
        tros::print("TEST_HTTP: got HTTP response\r\n");
        tros::print("TEST_HTTP: PASS\r\n");
    } else {
        tros::print("TEST_HTTP: unexpected response\r\n");
    }

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
