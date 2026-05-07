#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    // Print "PING" to identify this process
    tros::print("PING\r\n");

    // Send a message to init's endpoint (EP_ID=1)
    // Use a raw pointer to send with null payload, avoiding fat-pointer issues
    let result: usize;
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 11usize,
            in("a0") 1usize,       // ep_id = 1
            in("a1") 0usize,       // opcode = 0
            in("a2") 0usize,       // payload_ptr = 0 (no payload)
            lateout("a0") result,
        );
    }

    // Done
    loop { unsafe { core::arch::asm!("wfi"); } }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
