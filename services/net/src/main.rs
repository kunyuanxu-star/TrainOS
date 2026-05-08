#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

// Port table: maps port number -> listener EP
static mut PORT_TABLE: [(u16, usize); 8] = [(0, 0); 8];
static mut PORT_COUNT: usize = 0;

#[no_mangle]
extern "C" fn _start() -> ! {
    // Create network endpoint (will be EP 2: INIT has EP 1)
    let ep = tros::ep_create();
    tros::print("NET: listening on ep=");
    print_small(ep);
    tros::print("\r\n");

    let mut buf = [0u8; 64];

    loop {
        let (sender_pid, opcode) = tros::recv(ep, &mut buf);
        if sender_pid == usize::MAX {
            continue;
        }

        match opcode {
            // opcode 1: REGISTER(port, listener_ep)
            1 => {
                let port = ((buf[0] as u16) << 8) | (buf[1] as u16);
                let listener_ep = ((buf[2] as usize) << 8) | (buf[3] as usize);
                unsafe {
                    if PORT_COUNT < 8 {
                        PORT_TABLE[PORT_COUNT] = (port, listener_ep);
                        PORT_COUNT += 1;
                        tros::print("NET: registered port=");
                        print_small(port as usize);
                        tros::print("\r\n");
                    }
                }
            }
            // opcode 2: SEND(dst_port, data...)
            2 => {
                let dst_port = ((buf[0] as u16) << 8) | (buf[1] as u16);
                let data_len = buf[2] as usize;

                // Copy data to separate buffer to avoid aliasing with recv
                let mut send_buf = [0u8; 64];
                let copy_end = core::cmp::min(data_len, 64);
                let mut i = 0;
                while i < copy_end {
                    send_buf[i] = buf[3 + i];
                    i += 1;
                }

                // Route to listener
                unsafe {
                    let mut found = false;
                    for i in 0..PORT_COUNT {
                        if PORT_TABLE[i].0 == dst_port {
                            let listener_ep = PORT_TABLE[i].1;
                            // Forward the data (opcode 0 for delivery)
                            tros::send(listener_ep, 0, &send_buf[..copy_end]);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        tros::print("NET: no listener for port=");
                        print_small(dst_port as usize);
                        tros::print("\r\n");
                    }
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
    if m == 0 {
        tros::putchar(b'0');
        return;
    }
    loop {
        i -= 1;
        buf[i] = b'0' + (m - (m / 10) * 10) as u8;
        m = m / 10;
        if m == 0 {
            break;
        }
    }
    for j in i..10 {
        tros::putchar(buf[j]);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
