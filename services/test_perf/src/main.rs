#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("PERF: benchmark starting...\r\n");

    // Do some IPC to generate stats
    let ep = tros::ep_create();

    // Send a few messages
    for _ in 0..3 {
        tros::send(1, 0, b"bench"); // send to init (EP 1)
    }

    // Get performance stats
    let (sends, recvs, ctx) = tros::perf_stats();

    tros::print("PERF: sends=");
    print_small(sends);
    tros::print(" recvs=");
    print_small(recvs);
    tros::print(" ctx_sw=");
    print_small(ctx);
    tros::print("\r\n");

    if sends > 0 {
        tros::print("PERF: IPC counters working\r\n");
        tros::print("PERF: PASS\r\n");
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
        m = m / 10;
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
