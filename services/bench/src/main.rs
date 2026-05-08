#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("\r\n=== TrainOS Performance Benchmarks ===\r\n\r\n");

    // Benchmark 1: IPC round-trip (send + recv on self)
    tros::print("[BENCH-1] IPC latency (self-send)...\r\n");
    let ep = tros::ep_create();
    let start_ticks = tros::uptime_ms();

    // Do 10 self-sends (send to self ep, no receiver needed - just counts sends)
    for i in 0..10 {
        tros::send(ep, i as u16, b"benchmark payload data 64 bytes max test message...");
    }

    let end_ticks = tros::uptime_ms();
    let elapsed = end_ticks - start_ticks;
    tros::print("  ");
    tros::print_uint(10);
    tros::print(" sends in ");
    tros::print_uint(elapsed);
    tros::print(" ms\r\n");

    // Benchmark 2: Memory allocation speed
    tros::print("[BENCH-2] Memory stats...\r\n");
    let pages = tros::meminfo();
    tros::print("  allocated pages: ");
    tros::print_uint(pages);
    tros::print("\r\n");

    // Benchmark 3: Context switch count
    tros::print("[BENCH-3] Scheduler activity...\r\n");
    let (sends, recvs, ctx_sw) = tros::perf_stats();
    tros::print("  sends=");
    tros::print_uint(sends);
    tros::print(" recvs=");
    tros::print_uint(recvs);
    tros::print(" ctx_sw=");
    tros::print_uint(ctx_sw);
    tros::print("\r\n");

    // Calculate approximate IPC rate
    if elapsed > 0 {
        let rate = sends * 1000 / elapsed;
        tros::print("  IPC rate: ~");
        tros::print_uint(rate);
        tros::print(" sends/sec\r\n");
    }

    // Benchmark 4: System uptime
    tros::print("[BENCH-4] System uptime...\r\n");
    let uptime = tros::uptime_ms();
    tros::print("  uptime: ");
    tros::print_uint(uptime);
    tros::print(" ms\r\n");

    // Summary
    tros::print("\r\n=== Benchmarks Complete ===\r\n");

    if sends > 0 {
        tros::print("BENCH: PASS\r\n");
    }

    tros::print("\r\n");
    tros::print("Performance Summary:\r\n");
    tros::print("  Metric          Value\r\n");
    tros::print("  --------------- -----\r\n");
    tros::print("  IPC sends       ");
    tros::print_uint(sends);
    tros::print("\r\n");
    tros::print("  Up time         ");
    tros::print_uint(uptime);
    tros::print(" ms\r\n");
    tros::print("  Pages used      ");
    tros::print_uint(pages);
    tros::print("\r\n");
    tros::print("  Context switches ");
    tros::print_uint(ctx_sw);
    tros::print("\r\n");
    tros::print("\r\n");

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
