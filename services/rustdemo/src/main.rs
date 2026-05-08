#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("\r\n=== Rust User Program Demo ===\r\n");

    // 1. IPC: create endpoint and test
    tros::print("[RUST-IPC] Creating endpoint... ");
    let ep = tros::ep_create();
    tros::print("OK (ep=");
    print_small(ep);
    tros::print(")\r\n");

    // 2. Memory info
    tros::print("[RUST-MEM] Querying memory... ");
    let pages = tros::meminfo();
    tros::print("allocated=");
    print_small(pages);
    tros::print(" pages\r\n");

    // 3. Capability stats
    tros::print("[RUST-CAP] Capability system... ");
    let (total, used, ep_caps, _mem_caps) = tros::cap_stats();
    tros::print("slots=");
    print_small(total);
    tros::print(" used=");
    print_small(used);
    tros::print(" ep=");
    print_small(ep_caps);
    tros::print("\r\n");

    // 4. Performance counters
    tros::print("[RUST-PERF] Performance... ");
    let (sends, recvs, ctx) = tros::perf_stats();
    tros::print("sends=");
    print_small(sends);
    tros::print(" recvs=");
    print_small(recvs);
    tros::print(" ctx=");
    print_small(ctx);
    tros::print("\r\n");

    // 5. Send IPC message to init
    tros::print("[RUST-IPC] Sending to init... ");
    tros::send(1, 0, b"hello from rust!");
    tros::print("OK\r\n");

    // 6. FS write/read test
    tros::print("[RUST-FS] File system test... ");
    let reply_ep = tros::ep_create();
    let mut wbuf = [0u8; 64];
    let msg = b"Rust says hello!";
    let len = msg.len();
    // FS protocol: bytes [0..1]=reply_ep(u16 LE), [2]=data_len, [3..]=data
    wbuf[0] = (reply_ep & 0xFF) as u8;
    wbuf[1] = ((reply_ep >> 8) & 0xFF) as u8;
    wbuf[2] = len as u8;
    for i in 0..len { wbuf[3 + i] = msg[i]; }
    tros::send(2, 3, &wbuf[..3 + len]);

    let mut ack = [0u8; 64];
    tros::recv(reply_ep, &mut ack);

    // Read back
    let r_ep = tros::ep_create();
    let mut rreq = [0u8; 64];
    rreq[0] = (r_ep & 0xFF) as u8;
    rreq[1] = ((r_ep >> 8) & 0xFF) as u8;
    tros::send(2, 2, &rreq[..2]);
    let mut rdata = [0u8; 64];
    tros::recv(r_ep, &mut rdata);

    if &rdata[0..len] == msg {
        tros::print("OK\r\n");
    } else {
        tros::print("FAIL\r\n");
    }

    tros::print("=== Rust Demo Complete ===\r\n");
    tros::print("RUST: PASS\r\n");

    tros::exit(0);
}

fn print_small(n: usize) {
    let mut m = n;
    let mut buf = [0u8; 10];
    let mut i = 10;
    if m == 0 { tros::putchar(b'0'); return; }
    loop {
        i -= 1;
        buf[i] = b'0' + (m - (m / 10) * 10) as u8;
        m /= 10;
        if m == 0 { break; }
    }
    for j in i..10 { tros::putchar(buf[j]); }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    tros::print("RUST: PANIC!\r\n");
    loop { unsafe { core::arch::asm!("wfi"); } }
}
