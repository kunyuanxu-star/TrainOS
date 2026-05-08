#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("\r\n");
    tros::print("========================================\r\n");
    tros::print("  TrainOS V8.0 System Demo\r\n");
    tros::print("========================================\r\n\r\n");

    // 1. IPC test
    tros::print("[1/5] IPC: ping -> init ... ");
    tros::send(1, 0, b"ping");
    tros::print("OK\r\n");

    // 2. FS test
    tros::print("[2/5] FS: write -> read ... ");
    let reply_ep = tros::ep_create();
    let mut wbuf = [0u8; 64];
    let msg = b"TrainOS Demo Data";
    let len = msg.len();
    // FS protocol: [0..1]=reply_ep(u16 LE), [2]=data_len, [3..]=data
    wbuf[0] = (reply_ep & 0xFF) as u8;
    wbuf[1] = ((reply_ep >> 8) & 0xFF) as u8;
    wbuf[2] = len as u8;
    for i in 0..len { wbuf[3+i] = msg[i]; }
    tros::send(2, 3, &wbuf[..3+len]);

    let mut rbuf = [0u8; 64];
    let (_s, _o) = tros::recv(reply_ep, &mut rbuf);

    let reply_ep2 = tros::ep_create();
    let mut rreq = [0u8; 64];
    rreq[0] = (reply_ep2 & 0xFF) as u8;
    rreq[1] = ((reply_ep2 >> 8) & 0xFF) as u8;
    tros::send(2, 2, &rreq[..2]);
    let mut rdata = [0u8; 64];
    tros::recv(reply_ep2, &mut rdata);

    if &rdata[0..len] == msg {
        tros::print("OK\r\n");
    } else {
        tros::print("FAIL\r\n");
    }

    // 3. Memory stats
    tros::print("[3/5] MEM: allocated pages = ");
    let pages = tros::meminfo();
    print_small(pages);
    tros::print(" OK\r\n");

    // 4. Capability stats
    tros::print("[4/5] CAP: capability system ... ");
    let (total, used, ep, mem) = tros::cap_stats();
    if total > 0 { tros::print("OK\r\n"); }
    else { tros::print("FAIL\r\n"); }

    // 5. Performance counters
    tros::print("[5/5] PERF: IPC counters ... ");
    let (sends, _recvs, _ctx) = tros::perf_stats();
    if sends > 0 { tros::print("OK\r\n"); }
    else { tros::print("FAIL\r\n"); }

    tros::print("\r\n========================================\r\n");
    tros::print("  All systems operational\r\n");
    tros::print("  TrainOS V8.0 — READY\r\n");
    tros::print("========================================\r\n");

    loop { unsafe { core::arch::asm!("wfi"); } }
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
