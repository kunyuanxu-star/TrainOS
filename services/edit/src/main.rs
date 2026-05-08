#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

// Line buffer: store up to 4 lines of 60 chars each
static mut LINES: [[u8; 60]; 4] = [[0; 60]; 4];
static mut LINE_COUNT: usize = 0;

/// Print a decimal number (avoid % operator due to release-mode bug)
fn print_num(n: usize) {
    if n == 0 {
        tros::putchar(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 10;
    let mut v = n;
    loop {
        i -= 1;
        buf[i] = b'0' + (v - (v / 10) * 10) as u8;
        v /= 10;
        if v == 0 { break; }
    }
    for j in i..10 {
        tros::putchar(buf[j]);
    }
}

#[no_mangle]
extern "C" fn _start() -> ! {
    // Create our IPC endpoint (will get EP 6 at this priority)
    let my_ep = tros::ep_create();

    tros::print("EDIT: line editor on ep=");
    print_num(my_ep);
    tros::print("\r\n");
    tros::print("EDIT: commands: show, ins <text>, del <N>, save\r\n");

    loop {
        // Fresh buffer each iteration to avoid stale data from previous recv
        let mut buf = [0u8; 64];
        let (_sender, opcode) = tros::recv(my_ep, &mut buf);

        match opcode {
            // opcode 0: SHOW — display all lines
            0 => {
                unsafe {
                    tros::print("EDIT: ");
                    print_num(LINE_COUNT);
                    tros::print(" lines\r\n");
                    for i in 0..LINE_COUNT {
                        tros::print("  ");
                        print_num(i + 1);
                        tros::print(": ");
                        for j in 0..60 {
                            if LINES[i][j] == 0 { break; }
                            tros::putchar(LINES[i][j]);
                        }
                        tros::print("\r\n");
                    }
                }
            }
            // opcode 1: INSERT — buf[0..] = text
            1 => {
                unsafe {
                    if LINE_COUNT < 4 {
                        let mut j = 0;
                        while j < 60 && j < 63 && buf[j] != 0 {
                            LINES[LINE_COUNT][j] = buf[j];
                            j += 1;
                        }
                        // Null-terminate to avoid stale data from partial recv overlap
                        if j < 60 { LINES[LINE_COUNT][j] = 0; }
                        LINE_COUNT += 1;
                        tros::print("EDIT: inserted line ");
                        print_num(LINE_COUNT);
                        tros::print("\r\n");
                    } else {
                        tros::print("EDIT: buffer full!\r\n");
                    }
                }
            }
            // opcode 2: DELETE N — delete line number N (1-based)
            2 => {
                let n = buf[0] as usize;
                unsafe {
                    if n > 0 && n <= LINE_COUNT {
                        // Shift remaining lines down
                        for i in (n - 1)..(LINE_COUNT - 1) {
                            for j in 0..60 { LINES[i][j] = LINES[i+1][j]; }
                        }
                        // Clear the last now-unused line
                        for j in 0..60 { LINES[LINE_COUNT - 1][j] = 0; }
                        LINE_COUNT -= 1;
                        tros::print("EDIT: deleted line ");
                        print_num(n);
                        tros::print("\r\n");
                    }
                }
            }
            // opcode 3: SAVE — write content to FS (EP 2)
            3 => {
                unsafe {
                    if LINE_COUNT > 0 {
                        let reply_ep = tros::ep_create();
                        let mut wbuf = [0u8; 64];
                        // FS WRITE protocol:
                        //   bytes [0..1]: reply_ep (little-endian u16)
                        //   byte  [2]:    data length
                        //   bytes [3..]:  data
                        wbuf[0] = reply_ep as u8;
                        wbuf[1] = (reply_ep >> 8) as u8;
                        let mut pos = 3;
                        for i in 0..LINE_COUNT {
                            for j in 0..60 {
                                if LINES[i][j] == 0 { break; }
                                if pos < 63 { wbuf[pos] = LINES[i][j]; pos += 1; }
                            }
                            if pos < 63 { wbuf[pos] = b'\n'; pos += 1; }
                        }
                        wbuf[2] = (pos - 3) as u8;
                        tros::send(2, 3, &wbuf[..pos]);
                        // Wait for FS ack
                        let mut rbuf = [0u8; 64];
                        let _ = tros::recv(reply_ep, &mut rbuf);
                        tros::print("EDIT: saved\r\n");
                    }
                }
            }
            _ => {}
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
