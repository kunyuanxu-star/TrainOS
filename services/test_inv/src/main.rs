#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    let allocated = tros::meminfo();
    tros::print("INV: allocated pages=");
    print_small(allocated);
    tros::print("\r\n");

    if allocated > 0 {
        tros::print("INV: memory tracking OK\r\n");
        tros::print("INV: PASS\r\n");
    }

    tros::exit(0);
}

fn print_small(n: usize) {
    let mut m = n;
    let mut buf = [0u8; 10];
    let mut i = 10;
    if m == 0 { tros::putchar(b'0'); return; }
    loop { i -= 1; buf[i] = b'0' + (m - (m/10)*10) as u8; m = m/10; if m == 0 { break; } }
    for j in i..10 { tros::putchar(buf[j]); }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
