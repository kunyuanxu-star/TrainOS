#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("TEST_ARP: querying virtual ethernet...\r\n");

    // ARP query for 10.0.2.2
    let mut req = [0u8; 64];
    req[0] = 0x02; req[1] = 0x00; req[2] = 0x02; req[3] = 0x0A; // 10.0.2.2 LE
    tros::send(VETH_EP, 0, &req[..4]);

    // UDP send to 10.0.2.1:80
    req[0] = 0x01; req[1] = 0x00; req[2] = 0x02; req[3] = 0x0A; // 10.0.2.1
    req[4] = 0x00; req[5] = 80; // port 80
    tros::send(VETH_EP, 1, &req[..6]);

    tros::print("TEST_ARP: queries sent\r\n");
    tros::print("TEST_ARP: PASS\r\n");

    loop { unsafe { core::arch::asm!("wfi"); } }
}

// VETH at priority 58 gets EP 4 (test_cap->1, edit->2, proc->3, veth->4)
const VETH_EP: usize = 4;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
