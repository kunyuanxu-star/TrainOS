#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("ECHO: starting on port 7\r\n");

    // Create an endpoint and register with NET service (NET is on EP 2)
    let my_ep = tros::ep_create();

    // Register port 7 with NET service
    let mut reg = [0u8; 64];
    reg[0] = 0;
    reg[1] = 7; // port 7 (big-endian u16)
    reg[2] = (my_ep >> 8) as u8; // listener_ep high byte
    reg[3] = (my_ep & 0xFF) as u8; // listener_ep low byte
    tros::send(2, 1, &reg[..4]);

    let mut buf = [0u8; 64];

    loop {
        let (_sender_pid, _opcode) = tros::recv(my_ep, &mut buf);
        if _sender_pid == usize::MAX {
            continue;
        }

        // Check for "hello" (5 bytes)
        if buf[0] == b'h' && buf[1] == b'e' && buf[2] == b'l' && buf[3] == b'l' && buf[4] == b'o' {
            tros::print("NET: PASS\r\n");
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
