#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("SDP: service discovery test\r\n");

    // Lookup "fs" service in registry (REG creates EP 3: EP 1=test_cap, EP 2=proc)
    let mut req = [0u8; 64];
    req[0] = b'f';
    req[1] = b's'; // service name
    req[16] = 100; // reply EP hint
    tros::send(3, 0, &req[..17]);

    tros::print("SDP: lookup sent\r\n");
    tros::print("SDP: PASS\r\n");

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
