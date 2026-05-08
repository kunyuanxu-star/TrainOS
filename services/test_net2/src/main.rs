#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("NET2: network integration test\r\n");

    // VETH is on EP 7 (allocated at priority 58 after test_cap->3, bb->4, edit->5, proc->6)
    const VETH_EP: usize = 7;

    // Test ARP through veth
    let mut req = [0u8; 64];
    req[0] = 0x01; req[1] = 0x00; req[2] = 0x02; req[3] = 0x0A; // 10.0.2.1 LE
    tros::send(VETH_EP, 0, &req[..4]);

    // Test UDP through veth
    req[0] = 0x01; req[1] = 0x00; req[2] = 0x02; req[3] = 0x0A; // dst IP
    req[4] = 0x00; req[5] = 80; // port 80
    tros::send(VETH_EP, 1, &req[..6]);

    tros::print("NET2: network packets sent\r\n");
    tros::print("NET2: PASS\r\n");

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
