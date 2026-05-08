#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("CAP_TEST: checking capabilities...\r\n");

    // Get capability stats for this process
    let (total, used, ep_caps, mem_caps) = tros::cap_stats();

    tros::print("CAP_TEST: CNode has ");
    print_small(total);
    tros::print(" slots, ");
    print_small(used);
    tros::print(" used\r\n");

    tros::print("CAP_TEST: EP caps=");
    print_small(ep_caps);
    tros::print(" Mem caps=");
    print_small(mem_caps);
    tros::print("\r\n");

    // This process should have 0 caps by default (no EP created)
    // The kernel auto-stores EP caps on ep_create

    // Create an EP and check again
    let ep = tros::ep_create();
    let (_total2, used2, ep_caps2, _mem_caps2) = tros::cap_stats();

    if ep_caps2 > ep_caps {
        tros::print("CAP_TEST: EP cap auto-stored OK\r\n");
    }

    // Test delete
    // Delete the EP cap (slot 0 has our EP cap since it's the first non-null)
    let r = tros::cap_delete(0);
    if r == 0 {
        tros::print("CAP_TEST: cap delete OK\r\n");
    }

    let (_total3, used3, ep_caps3, _mem_caps3) = tros::cap_stats();
    if ep_caps3 < ep_caps2 {
        tros::print("CAP_TEST: cap count decreased after delete\r\n");
        tros::print("CAP_TEST: PASS\r\n");
    } else {
        tros::print("CAP_TEST: cap count unchanged!\r\n");
    }

    tros::exit(0);
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
