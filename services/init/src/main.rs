#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[no_mangle]
extern "C" fn _start() -> ! {
    let msg = b"TrainOS ready\r\n";
    for &byte in msg.iter() {
        unsafe {
            core::arch::asm!(
                "ecall",
                in("a7") 1usize,
                in("a0") byte as usize,
            );
        }
    }

    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
