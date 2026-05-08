#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("CAT: utility started\r\n");

    // Read from FS (EP 2) and echo to console
    let reply_ep = tros::ep_create();
    let mut req = [0u8; 64];
    req[0] = reply_ep as u8;
    tros::send(2, 2, &req[..1]); // READ opcode

    let mut buf = [0u8; 64];
    let (_sender, _op) = tros::recv(reply_ep, &mut buf);

    tros::print("CAT: ");
    for i in 0..32 { if buf[i] == 0 { break; } tros::putchar(buf[i]); }
    tros::print("\r\n");

    loop { unsafe { core::arch::asm!("wfi"); } }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
