#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    let pid = tros::getpid();
    tros::print("SMP_TEST: process pid=");
    print_small(pid);

    // Get scheduler stats to see which CPU we're on
    let (_s, _r, ctx) = tros::perf_stats();
    tros::print(" ctx_sw=");
    print_small(ctx);
    tros::print("\r\n");

    // Do some IPC to generate cross-core activity
    let ep = tros::ep_create();
    for _ in 0..3 {
        tros::send(1, 0, b"smp test ping");
    }

    tros::print("SMP_TEST: IPC sent from pid=");
    print_small(pid);
    tros::print("\r\n");

    // Verify fork still works under SMP
    let child = tros::fork();
    if child == 0 {
        tros::print("SMP_TEST: child process on fork\r\n");
        tros::exit(0);
    } else if child != usize::MAX {
        tros::print("SMP_TEST: parent, child pid=");
        print_small(child);
        tros::print("\r\n");
    }

    // Get final stats
    let (sends, recvs, ctx2) = tros::perf_stats();
    tros::print("SMP_TEST: stats sends=");
    print_small(sends);
    tros::print(" recvs=");
    print_small(recvs);
    tros::print(" ctx=");
    print_small(ctx2);
    tros::print("\r\n");

    if ctx2 > 0 && sends > 0 {
        tros::print("SMP_TEST: PASS\r\n");
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
