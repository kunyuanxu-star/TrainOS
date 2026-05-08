#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

const FS_EP: usize = 2; // FS service EP (well-known after init creates EP 1)

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("\r\nTrainOS Shell v0.1\r\n");
    tros::print("Type 'help' for commands.\r\n");
    prompt();

    let mut cmd_buf = [0u8; 64];
    let mut cmd_len = 0;

    loop {
        let c = tros::getchar();
        if c == usize::MAX || c == 0 {
            // No input, yield a bit
            for _ in 0..1000 { unsafe { core::arch::asm!("nop"); } }
            continue;
        }

        let byte = c as u8;

        if byte == b'\r' || byte == b'\n' {
            if cmd_len > 0 {
                tros::print("\r\n");
                process_command(&cmd_buf[..cmd_len]);
                cmd_len = 0;
                prompt();
            }
        } else if byte == 0x7f || byte == 0x08 {
            // Backspace
            if cmd_len > 0 {
                cmd_len -= 1;
                tros::putchar(0x08);
                tros::putchar(b' ');
                tros::putchar(0x08);
            }
        } else if cmd_len < 63 {
            cmd_buf[cmd_len] = byte;
            cmd_len += 1;
            tros::putchar(byte);
        }
    }
}

fn prompt() {
    tros::print("$ ");
}

fn process_command(cmd: &[u8]) {
    if cmd == b"help" {
        tros::print("Commands: help echo read write ps\r\n");
    } else if cmd.starts_with(b"echo ") {
        let msg = &cmd[5..];
        tros::putchar(b' ');
        for &b in msg { tros::putchar(b); }
        tros::print("\r\n");
    } else if cmd.starts_with(b"write ") {
        // write <text> to FS
        let text = &cmd[6..];
        let len = text.len();
        if len > 0 && len <= 62 {
            let mut buf = [0u8; 64];
            buf[0] = len as u8;
            for i in 0..len { buf[1 + i] = text[i]; }

            // Create reply EP
            let reply_ep = tros::ep_create();
            buf[63] = reply_ep as u8;

            tros::send(FS_EP, 3, &buf[..1 + len + 1]); // WRITE op=3

            let mut rbuf = [0u8; 64];
            let (_sender, _op) = tros::recv(reply_ep, &mut rbuf);
            tros::print("  written\r\n");
        }
    } else if cmd == b"read" {
        // Read from FS
        let reply_ep = tros::ep_create();
        let mut rbuf = [0u8; 64];
        rbuf[0] = reply_ep as u8;
        tros::send(FS_EP, 2, &rbuf[..1]); // READ op=2

        let (_sender, _op) = tros::recv(reply_ep, &mut rbuf);
        tros::print("  data: ");
        for i in 0..11 { tros::putchar(rbuf[i]); }
        tros::print("\r\n");
    } else if cmd == b"ls" {
        tros::print("  init  ping  fs  sh  drv  net  echo  proc  reg\r\n");
    } else if cmd == b"cat" {
        tros::print("  (type 'exec cat' to run cat service)\r\n");
    } else if cmd == b"exec" {
        tros::print("  exec not yet implemented\r\n");
    } else if cmd.starts_with(b"say ") {
        let msg = &cmd[4..];
        tros::print("  ");
        for &b in msg { tros::putchar(b); }
        tros::print("\r\n");
    } else if cmd == b"ver" {
        tros::print("  TrainOS v5.0\r\n");
    } else if cmd == b"date" {
        tros::print("  2026-05-07\r\n");
    } else if cmd == b"ps" {
        tros::print("  pid=sh (shell)\r\n");
        tros::print("  pid=fs (filesystem)\r\n");
        tros::print("  pid=init (init)\r\n");
    } else {
        tros::print("  unknown: ");
        for &b in cmd { tros::putchar(b); }
        tros::print("\r\n");
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
