#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    // Create an IPC endpoint
    tros::ep_create();

    // Print "INIT" to identify this process
    tros::print("INIT\r\n");

    // Loop: receive messages on endpoint 1 (the first endpoint)
    let mut buf = [0u8; 64];
    loop {
        let (sender, _opcode) = tros::recv(1, &mut buf);
        if sender != usize::MAX {
            tros::print("TrainOS IPC OK\r\n");
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
