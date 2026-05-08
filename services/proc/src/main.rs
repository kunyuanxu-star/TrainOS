#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    // Create an EP for proc queries
    let ep = tros::ep_create();
    tros::print("PROC: listening on ep=");
    print_small(ep);
    tros::print("\r\n");

    let mut buf = [0u8; 64];

    loop {
        let (sender_pid, opcode) = tros::recv(ep, &mut buf);
        if sender_pid == usize::MAX { continue; }

        match opcode {
            // opcode 0: LIST — return process list
            0 => {
                let mut plist = [0u8; 128];
                let count = tros::proclist(&mut plist);

                tros::print("PROC: ");
                print_small(count);
                tros::print(" processes\r\n");

                for i in 0..count {
                    let off = i * 6;
                    let pid = (plist[off] as u32)
                        | ((plist[off+1] as u32) << 8)
                        | ((plist[off+2] as u32) << 16)
                        | ((plist[off+3] as u32) << 24);
                    let prio = plist[off + 4];
                    let state = plist[off + 5];

                    tros::print("  pid=");
                    print_small(pid as usize);
                    tros::print(" prio=");
                    print_small(prio as usize);
                    tros::print(" state=");
                    print_small(state as usize);
                    tros::print("\r\n");
                }
            }
            // opcode 1: KILL pid
            1 => {
                let pid = (buf[0] as u32) | ((buf[1] as u32) << 8)
                    | ((buf[2] as u32) << 16) | ((buf[3] as u32) << 24);
                let r = tros::kill(pid);
                if r == 0 {
                    tros::print("PROC: killed pid=");
                    print_small(pid as usize);
                    tros::print("\r\n");
                }
            }
            _ => {}
        }
    }
}

fn print_small(n: usize) {
    let mut m = n;
    let mut buf = [0u8; 10];
    let mut i = 10;
    if m == 0 { tros::putchar(b'0'); return; }
    loop {
        i -= 1; buf[i] = b'0' + (m - (m / 10) * 10) as u8;
        m = m / 10; if m == 0 { break; }
    }
    for j in i..10 { tros::putchar(buf[j]); }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
