#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

// Expected EP at priority 58: test_cap->1, edit->2, proc->3, veth->4
#[allow(dead_code)]
const VETH_EP: usize = 4;

// Simple ARP table: IP -> MAC
static mut ARP_TABLE_IP: [u32; 4] = [0; 4];
static mut ARP_TABLE_MAC: [[u8; 6]; 4] = [[0; 6]; 4];
static mut ARP_COUNT: usize = 0;

#[no_mangle]
extern "C" fn _start() -> ! {
    // Create our IPC endpoint
    let ep = tros::ep_create();
    tros::print("VETH: virtual ethernet on ep=");
    print_small(ep);
    tros::print("\r\n");

    // Register known hosts
    register_arp(0x0A000201, &[0x52, 0x54, 0x00, 0x12, 0x34, 0x01]); // 10.0.2.1
    register_arp(0x0A000202, &[0x52, 0x54, 0x00, 0x12, 0x34, 0x02]); // 10.0.2.2

    let mut buf = [0u8; 64];

    loop {
        let (_sender, opcode) = tros::recv(ep, &mut buf);
        match opcode {
            0 => { // ARP_QUERY(ip: u32 LE)
                let ip = (buf[0] as u32) | ((buf[1] as u32) << 8)
                    | ((buf[2] as u32) << 16) | ((buf[3] as u32) << 24);
                tros::print("VETH: ARP who-has ");
                print_ip(ip);
                tros::print("\r\n");

                // Look up in table
                let mut found = false;
                unsafe {
                    for i in 0..ARP_COUNT {
                        if ARP_TABLE_IP[i] == ip {
                            tros::print("VETH: ARP reply MAC=");
                            print_mac(&ARP_TABLE_MAC[i]);
                            tros::print("\r\n");
                            found = true;
                            break;
                        }
                    }
                }
                if found {
                    tros::print("VETH: ARP resolved\r\n");
                } else {
                    tros::print("VETH: ARP not found\r\n");
                }
            }
            1 => { // UDP_SEND(dst_ip, dst_port, data...)
                let dst_ip = (buf[0] as u32) | ((buf[1] as u32) << 8)
                    | ((buf[2] as u32) << 16) | ((buf[3] as u32) << 24);
                let dst_port = ((buf[4] as u16) << 8) | (buf[5] as u16);
                tros::print("VETH: UDP to ");
                print_ip(dst_ip);
                tros::printf(":%u\r\n", dst_port as usize);
                tros::print("VETH: packet sent\r\n");
            }
            _ => {}
        }
    }
}

fn register_arp(ip: u32, mac: &[u8; 6]) {
    unsafe {
        if ARP_COUNT < 4 {
            ARP_TABLE_IP[ARP_COUNT] = ip;
            ARP_TABLE_MAC[ARP_COUNT] = *mac;
            ARP_COUNT += 1;
        }
    }
}

fn print_ip(ip: u32) {
    let b1 = (ip >> 24) & 0xFF;
    let b2 = (ip >> 16) & 0xFF;
    let b3 = (ip >> 8) & 0xFF;
    let b4 = ip & 0xFF;
    print_octet(b1 as usize);
    tros::putchar(b'.');
    print_octet(b2 as usize);
    tros::putchar(b'.');
    print_octet(b3 as usize);
    tros::putchar(b'.');
    print_octet(b4 as usize);
}

fn print_octet(n: usize) {
    let mut m = n;
    if m == 0 {
        tros::putchar(b'0');
        return;
    }
    let mut buf = [0u8; 4];
    let mut i = 4;
    loop {
        i -= 1;
        buf[i] = b'0' + (m - (m / 10) * 10) as u8;
        m = m / 10;
        if m == 0 { break; }
    }
    for j in i..4 {
        tros::putchar(buf[j]);
    }
}

fn print_small(n: usize) {
    let mut m = n;
    if m == 0 { tros::putchar(b'0'); return; }
    let mut buf = [0u8; 10];
    let mut i = 10;
    loop {
        i -= 1; buf[i] = b'0' + (m - (m / 10) * 10) as u8;
        m = m / 10; if m == 0 { break; }
    }
    for j in i..10 { tros::putchar(buf[j]); }
}

fn print_mac(mac: &[u8; 6]) {
    print_hex2(mac[0]); tros::putchar(b':');
    print_hex2(mac[1]); tros::putchar(b':');
    print_hex2(mac[2]); tros::putchar(b':');
    print_hex2(mac[3]); tros::putchar(b':');
    print_hex2(mac[4]); tros::putchar(b':');
    print_hex2(mac[5]);
}

fn print_hex2(b: u8) {
    let hi = b >> 4;
    let lo = b & 0xF;
    tros::putchar(if hi < 10 { b'0' + hi } else { b'a' + (hi - 10) });
    tros::putchar(if lo < 10 { b'0' + lo } else { b'a' + (lo - 10) });
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
