#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("TEST_FS: starting\r\n");

    // Create a reply endpoint for FS to respond on.
    let reply_ep = tros::ep_create();

    // Prepare WRITE payload:
    //   bytes [0..1]: reply_ep (little-endian u16)
    //   byte  [2]:    data length
    //   bytes [3..]:  data ("hello world")
    let mut wbuf = [0u8; 64];
    wbuf[0] = reply_ep as u8;
    wbuf[1] = (reply_ep >> 8) as u8;
    wbuf[2] = 11; // data length

    let hello = b"hello world";
    let mut i = 0;
    while i < hello.len() {
        wbuf[3 + i] = hello[i];
        i += 1;
    }

    // Send WRITE (opcode 3) to FS on EP 2
    tros::send(2, 3, &wbuf[..3 + hello.len()]);
    tros::print("TEST_FS: wrote data\r\n");

    // Wait for FS to reply on our reply endpoint
    let mut rbuf = [0u8; 64];
    let (_sender, _opcode) = tros::recv(reply_ep, &mut rbuf);
    tros::print("TEST_FS: got write ack\r\n");

    // Prepare READ payload with reply_ep
    let mut rbuf2 = [0u8; 64];
    rbuf2[0] = reply_ep as u8;
    rbuf2[1] = (reply_ep >> 8) as u8;

    // Send READ (opcode 2) to FS on EP 2
    tros::send(2, 2, &rbuf2[..2]);

    // Wait for FS to respond with stored data
    let mut rbuf3 = [0u8; 64];
    let (_sender, _opcode) = tros::recv(reply_ep, &mut rbuf3);
    tros::print("TEST_FS: got data: ");
    let mut j = 0;
    while j < 11 {
        tros::putchar(rbuf3[j]);
        j += 1;
    }
    tros::print("\r\n");
    tros::print("TEST_FS: PASS\r\n");

    // Done
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}
