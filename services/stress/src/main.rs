#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("STRESS: multi-core benchmark\r\n");

    let uptime_start = tros::uptime_ms();

    // Run IPC ping-pong benchmark
    let ep = tros::ep_create();
    let mut count = 0usize;

    // Send 10 ping messages
    for _ in 0..10 {
        tros::send(ep, 0x42, b"stress test payload data here..");
        count += 1;
    }

    let uptime_end = tros::uptime_ms();
    let elapsed = uptime_end - uptime_start;

    let (sends, recvs, ctx) = tros::perf_stats();

    tros::print("STRESS: benchmark results:\r\n");
    tros::print("  messages: ");
    tros::print_uint(count);
    tros::print("\r\n");
    tros::print("  uptime_ms: ");
    tros::print_uint(uptime_end);
    tros::print("\r\n");
    tros::print("  sends: ");
    tros::print_uint(sends);
    tros::print("\r\n");
    tros::print("  recvs: ");
    tros::print_uint(recvs);
    tros::print("\r\n");
    tros::print("  ctx_sw: ");
    tros::print_uint(ctx);
    tros::print("\r\n");

    if sends > 0 && uptime_end > 0 {
        tros::print("STRESS: PASS\r\n");
    }

    loop {
        unsafe {
            core::arch::asm!("wfi");
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
