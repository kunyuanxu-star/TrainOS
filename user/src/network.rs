//! Network service - user-space TCP/IP stack
//!
//! Implements the TCP/IP protocol stack in user space.
//! Communicates with driver service via IPC for raw frame I/O.

#![no_std]
#![no_main]

mod net;
mod driver_mmio;

use net::*;

/// Network service port
const NETWORK_PORT: u32 = 3;

/// IPC operation types for network service
const NET_OP_RECV: u32 = 0;
const NET_OP_SEND: u32 = 1;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

/// Write character to console
fn putchar(c: u8) {
    unsafe {
        core::arch::asm!("li a7, 1; mv a0, {0}; ecall", in(reg) c);
    }
}

/// Print string
fn print(s: &str) {
    for b in s.bytes() {
        putchar(b);
        if b == b'\n' {
            putchar(b'\r');
        }
    }
}

/// Print hex number
fn print_hex(val: usize) {
    let hex = b"0123456789abcdef";
    for i in (0..16).rev() {
        putchar(hex[(val >> (i * 4)) & 0xf as usize]);
    }
}

/// Print decimal number
fn print_num(mut val: usize) {
    if val == 0 {
        putchar(b'0');
        return;
    }
    let mut digits: [u8; 20] = [0; 20];
    let mut idx = 0;
    while val > 0 {
        digits[idx] = b'0' + (val % 10) as u8;
        val /= 10;
        idx += 1;
    }
    while idx > 0 {
        idx -= 1;
        putchar(digits[idx]);
    }
}

/// Make a syscall
fn syscall(n: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize) -> isize {
    let ret;
    unsafe {
        core::arch::asm!(
            "mv a7, {syscall_num}",
            "mv a0, {arg0}; mv a1, {arg1}; mv a2, {arg2}; mv a3, {arg3}; mv a4, {arg4}; mv a5, {arg5}",
            "ecall",
            lateout("a0") ret,
            arg0 = in(reg) a0,
            arg1 = in(reg) a1,
            arg2 = in(reg) a2,
            arg3 = in(reg) a3,
            arg4 = in(reg) a4,
            arg5 = in(reg) a5,
            syscall_num = in(reg) n,
        );
    }
    ret
}

// Syscall numbers
const SYS_ENDPOINT_CREATE: usize = 1000;
const SYS_SEND: usize = 1002;
const SYS_RECV: usize = 1003;
const SYS_EXIT: usize = 93;
const SYS_SCHED_YIELD: usize = 124;

/// Process incoming packet from driver
fn process_incoming_packet(buffer: &mut NetBuffer) {
    // Process Ethernet frame
    if !eth::eth_input(buffer, &NetInterface::default()) {
        return;
    }

    // Parse Ethernet header to get EtherType
    if let Some(frame) = eth::EthFrame::parse(&buffer.data[..buffer.len]) {
        match frame.ether_type() {
            ETH_TYPE_IPV4 => {
                // Handle IPv4
                let packet = match ipv4::IpPacket::parse(&buffer.data[14..buffer.len]) {
                    Some(p) => p,
                    None => return,
                };

                match packet.protocol() {
                    IP_PROTO_TCP => {
                        tcp::tcp_input(buffer, packet.header.src_ip(), packet.header.dst_ip());
                    }
                    IP_PROTO_UDP => {
                        udp::udp_input(buffer, packet.header.src_ip(), packet.header.dst_ip());
                    }
                    IP_PROTO_ICMP => {
                        // ICMP echo (ping) could be handled here
                    }
                    _ => {}
                }
            }
            ETH_TYPE_ARP => {
                arp::arp_input(buffer, &NetInterface::default());
            }
            _ => {}
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() {
    print("network: Network service starting\n");

    // Initialize the TCP/IP stack
    net::init();

    // Get network interface info
    let iface = net::get_interface();
    if let Some(ref i) = *iface {
        print("network: Interface: eth0, IP: ");
        let octets = i.ip.octets();
        print_num(octets[0] as usize);
        putchar(b'.');
        print_num(octets[1] as usize);
        putchar(b'.');
        print_num(octets[2] as usize);
        putchar(b'.');
        print_num(octets[3] as usize);
        print("\n");
    }
    drop(iface);

    // Create endpoint for network service
    let net_port = syscall(SYS_ENDPOINT_CREATE, 0, 0, 0, 0, 0, 0) as u32;
    if net_port < 2 {
        print("network: Failed to create endpoint\n");
        loop {
            unsafe { core::arch::asm!("wfi"); }
        }
    }

    print("network: Listening on port ");
    print_hex(net_port as usize);
    print("\n");

    // Buffer for incoming frames
    let mut recv_buffer: [u8; 2048] = [0; 2048];

    // Main loop - receive frames and process
    loop {
        // Receive frame from driver (blocking)
        let size = syscall(
            SYS_RECV,
            net_port as usize,
            recv_buffer.as_mut_ptr() as usize,
            2048,
            0, 0, 0,
        ) as usize;

        if size > 14 {
            // Process the incoming packet
            let mut net_buffer = NetBuffer::new();
            net_buffer.len = size.min(MAX_PACKET_SIZE);
            net_buffer.data[..net_buffer.len].copy_from_slice(&recv_buffer[..net_buffer.len]);

            process_incoming_packet(&mut net_buffer);
        }

        // Yield to allow other processes to run
        syscall(SYS_SCHED_YIELD, 0, 0, 0, 0, 0, 0);
    }
}
