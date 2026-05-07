#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    let child = tros::fork();

    if child == 0 {
        tros::print("[CHILD] I am alive!\r\n");
    } else if child != usize::MAX {
        tros::print("[PARENT] child pid=");
        let mut n = child;
        let mut buf = [0u8; 10];
        let mut i = 10;
        loop {
            i -= 1;
            buf[i] = b'0' + (n - (n / 10) * 10) as u8;
            n = n / 10;
            if n == 0 { break; }
        }
        for j in i..10 { tros::putchar(buf[j]); }
        tros::print("\r\n");
    } else {
        tros::print("[FORK] failed!\r\n");
    }

    tros::print("[DONE]\r\n");
    loop { unsafe { core::arch::asm!("wfi"); } }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
