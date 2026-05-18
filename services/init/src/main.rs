#![no_std]
#![no_main]

// TrainOS Init Service V2 — System initialization and health monitoring
//
// Responsibilities:
//   1. Print system banner with version and boot info
//   2. Create well-known endpoints (EP 1 = init)
//   3. Print system health summary (memory, processes, uptime)
//   4. Listen for IPC messages (diagnostics, shutdown requests)
//   5. Periodically report system status

use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    // Banner
    tros::print("\r\n");
    tros::print("========================================\r\n");
    tros::print("  TrainOS V17.0 — Microkernel OS\r\n");
    tros::print("  RISC-V 64-bit | RustSBI | machina\r\n");
    tros::print("========================================\r\n");
    tros::print("\r\n");

    // System health check
    let pid = tros::getpid();
    tros::print("[INIT] pid=");
    tros::print_uint(pid);
    tros::print("\r\n");

    // Memory
    let pages = tros::meminfo();
    tros::print("[MEM]  allocated: ");
    tros::print_uint(pages);
    tros::print(" pages (");
    tros::print_uint(pages * 4096 / 1024);
    tros::print(" KiB)\r\n");

    // Performance counters
    let (sends, recvs, ctx) = tros::perf_stats();
    tros::print("[PERF] sends=");
    tros::print_uint(sends);
    tros::print(" recvs=");
    tros::print_uint(recvs);
    tros::print(" ctx=");
    tros::print_uint(ctx);
    tros::print("\r\n");

    // Uptime
    let ms = tros::uptime_ms();
    tros::print("[TIME] uptime: ");
    tros::print_uint(ms / 1000);
    tros::print("s\r\n");

    // User info
    let uid = tros::getuid();
    tros::print("[USER] uid=");
    tros::print_uint(uid);
    if uid == 0 { tros::print(" (root)"); }
    tros::print("\r\n");

    tros::print("\r\nSystem ready.\r\n");

    // EP 1 = init endpoint (well-known)
    let mut buf = [0u8; 64];
    let mut tick: usize = 0;

    loop {
        let (sender, opcode) = tros::recv(1, &mut buf);
        if sender != usize::MAX {
            match opcode {
                0 => {
                    // Ping — reply with status
                    tros::print("[INIT] ping from pid=");
                    tros::print_uint(sender);
                    tros::print("\r\n");
                }
                1 => {
                    // Shutdown request
                    tros::print("[INIT] shutdown requested\r\n");
                }
                2 => {
                    // Health check request
                    tros::print("[INIT] health: mem=");
                    tros::print_uint(tros::meminfo());
                    tros::print(" uptime=");
                    tros::print_uint(tros::uptime_ms() / 1000);
                    tros::print("s\r\n");
                }
                _ => {}
            }
        }

        // Periodic status every ~1000 loops
        tick += 1;
        if tick > 0 && tick - (tick / 1000) * 1000 == 0 {
            // Yield to avoid busy-waiting
            tros::yield_cpu();
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    tros::print("[INIT] PANIC\r\n");
    loop { unsafe { core::arch::asm!("wfi"); } }
}
