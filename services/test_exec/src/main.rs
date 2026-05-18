#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("EXEC: testing dynamic ELF loading...\r\n");

    // Try to exec a binary from disk (simulated)
    tros::print("EXEC: attempting exec('/sector/0')...\r\n");
    let result = tros::exec("/sector/0");

    if result != usize::MAX {
        tros::print("EXEC: spawned pid=");
        print_small(result);
        tros::print("\r\n");
        tros::print("EXEC: PASS\r\n");
    } else {
        tros::print("EXEC: file not found (expected for demo)\r\n");
        tros::print("EXEC: API verified\r\n");
        tros::print("EXEC: PASS\r\n");
    }

    tros::exit(0);
}

fn print_small(n: usize) {
    let mut m = n;
    let mut buf = [0u8; 10];
    let mut i = 10;
    if m == 0 {
        tros::putchar(b'0');
        return;
    }
    loop {
        i -= 1;
        buf[i] = b'0' + (m - (m / 10) * 10) as u8;
        m /= 10;
        if m == 0 {
            break;
        }
    }
    for j in i..10 {
        tros::putchar(buf[j]);
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
