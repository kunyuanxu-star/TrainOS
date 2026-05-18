#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

const HTTP_EP: usize = 8;

// TCP states
#[allow(dead_code)]
#[derive(PartialEq)]
enum TcpState {
    Closed, Listen, SynRcvd, SynSent, Established, FinWait1, FinWait2, CloseWait, LastAck, TimeWait,
}

static mut STATE: TcpState = TcpState::Closed;

#[no_mangle]
extern "C" fn _start() -> ! {
    unsafe { STATE = TcpState::Listen; }
    tros::print("HTTP: TCP server on EP 8\r\n");
    tros::print("HTTP: state=LISTEN\r\n");

    let mut buf = [0u8; 64];

    loop {
        let (sender_pid, opcode) = tros::recv(HTTP_EP, &mut buf);
        if sender_pid == usize::MAX { continue; }

        // "TCP handshake": process the request
        unsafe { STATE = TcpState::Established; }

        // Extract reply_ep from payload (client puts it in first 2 bytes, little-endian)
        let reply_ep = buf[0] as usize | ((buf[1] as usize) << 8);

        match opcode {
            0 => { // GET request
                tros::print("HTTP: GET request: ");
                for i in 2..18 { if buf[i] == 0 { break; } tros::putchar(buf[i]); }
                tros::print("\r\n");

                // Send HTTP response
                let response = b"HTTP/1.0 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>TrainOS HTTP Server</h1><p>Running on microkernel IPC</p></body></html>";
                tros::send(reply_ep, 0x200, response);
                tros::print("HTTP: 200 OK sent\r\n");
            }
            _ => {
                tros::print("HTTP: unknown opcode\r\n");
            }
        }

        // Connection close
        unsafe { STATE = TcpState::Closed; }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
