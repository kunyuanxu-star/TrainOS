#![no_std]
#![no_main]

// TrainOS Shell V2 — interactive command interpreter with VFS support
//
// Commands:
//   help      — show this help
//   ver       — show version
//   ps        — process listing (via syscall)
//   echo ...  — echo text
//   write F T — write text T to file F
//   read  F   — read file F
//   cat   F   — display file F
//   ls        — list root directory
//   date      — show date
//   whoami    — show user
//   uptime    — show uptime
//   perf      — show performance stats
//   mem       — show memory info
//   clear     — clear screen

use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("\r\nTrainOS Shell V2\r\n");
    tros::print("Type 'help' for commands.\r\n");
    tros::print("$ ");

    let mut cmd_buf = [0u8; 64];
    let mut cmd_len = 0;

    loop {
        let c = tros::getchar();
        if c == usize::MAX || c == 0 {
            // Yield and retry
            tros::yield_cpu();
            continue;
        }

        let byte = c as u8;
        if byte == b'\r' || byte == b'\n' {
            if cmd_len > 0 {
                tros::print("\r\n");
                process_command(&cmd_buf[..cmd_len]);
                cmd_len = 0;
                tros::print("$ ");
            }
        } else if byte == 0x7f || byte == 0x08 {
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

fn process_command(cmd: &[u8]) {
    if cmd == b"help" {
        tros::print("  help ver ps echo write read cat ls date whoami uptime perf mem clear\r\n");
    } else if cmd == b"ver" {
        tros::print("  TrainOS V16.0\r\n");
    } else if cmd == b"ps" {
        // Use proclist syscall to get real process data
        let mut buf = [0u8; 64];
        let count = tros::proclist(&mut buf);
        let mut i = 0;
        let mut shown = 0;
        while i < count && shown < count {
            let off = shown * 6;
            if off + 5 >= 64 { break; }
            let pid = buf[off] as u32 | ((buf[off+1] as u32) << 8)
                | ((buf[off+2] as u32) << 16) | ((buf[off+3] as u32) << 24);
            let prio = buf[off+4];
            let state = buf[off+5];
            tros::print("  pid=");
            tros::print_uint(pid as usize);
            tros::print(" prio=");
            tros::print_uint(prio as usize);
            let state_str = match state { 0 => "Ready", 1 => "Running", 2 => "Waiting", _ => "Dead" };
            tros::print(" ");
            tros::print(state_str);
            tros::print("\r\n");
            shown += 1;
        }
    } else if cmd.starts_with(b"echo ") {
        let msg = &cmd[5..];
        for &b in msg { tros::putchar(b); }
        tros::print("\r\n");
    } else if cmd.starts_with(b"write ") {
        let rest = &cmd[6..];
        if let Some(space_pos) = rest.iter().position(|&b| b == b' ') {
            let fname = &rest[..space_pos];
            let content = &rest[space_pos + 1..];
            // Open and write to VFS via POSIX syscalls
            let fd = tros::open_bytes(fname);
            if fd != usize::MAX {
                tros::write(fd, content);
                tros::close(fd);
            }
            tros::print("  wrote ");
            tros::print_uint(content.len());
            tros::print(" bytes\r\n");
        }
    } else if cmd.starts_with(b"read ") {
        let fname = &cmd[5..];
        let fd = tros::open_bytes(fname);
        if fd != usize::MAX {
            let mut buf = [0u8; 64];
            let n = tros::read(fd, &mut buf);
            tros::print("  ");
            for i in 0..n { tros::putchar(buf[i]); }
            tros::print("\r\n");
            tros::close(fd);
        } else {
            tros::print("  file not found\r\n");
        }
    } else if cmd.starts_with(b"cat ") {
        let fname = &cmd[4..];
        let fd = tros::open_bytes(fname);
        if fd != usize::MAX {
            let mut buf = [0u8; 64];
            let n = tros::read(fd, &mut buf);
            for i in 0..n { tros::putchar(buf[i]); }
            tros::print("\r\n");
            tros::close(fd);
        } else {
            tros::print("  cat: no such file\r\n");
        }
    } else if cmd == b"ls" {
        let mut buf = [0u8; 64];
        let n = tros::getdents64(0, &mut buf);
        tros::print("  ");
        for i in 0..n {
            if buf[i] == b'\n' { tros::print("  "); }
            else if buf[i] != 0 { tros::putchar(buf[i]); }
        }
        tros::print("\r\n");
    } else if cmd == b"date" {
        let uptime = tros::uptime_ms();
        tros::print("  uptime: ");
        tros::print_uint(uptime / 1000);
        tros::print("s\r\n");
    } else if cmd == b"whoami" {
        let uid = tros::getuid();
        if uid == 0 { tros::print("  root"); }
        else { tros::print("  user"); }
        tros::print(" (uid=");
        tros::print_uint(uid);
        tros::print(")\r\n");
    } else if cmd == b"uptime" {
        let ms = tros::uptime_ms();
        tros::print("  ");
        tros::print_uint(ms / 1000);
        tros::print(" seconds\r\n");
    } else if cmd == b"perf" {
        let (sends, recvs, ctx) = tros::perf_stats();
        tros::print("  sends="); tros::print_uint(sends);
        tros::print(" recvs="); tros::print_uint(recvs);
        tros::print(" ctx="); tros::print_uint(ctx);
        tros::print("\r\n");
    } else if cmd == b"mem" {
        let pages = tros::meminfo();
        tros::print("  allocated pages: ");
        tros::print_uint(pages);
        tros::print("\r\n");
    } else if cmd == b"clear" {
        tros::print("\r\n\r\n\r\n\r\n\r\n\r\n");
    } else if cmd == b"pid" {
        let pid = tros::getpid();
        let ppid = tros::getppid();
        tros::print("  pid="); tros::print_uint(pid);
        tros::print(" ppid="); tros::print_uint(ppid);
        tros::print("\r\n");
    } else if !cmd.is_empty() {
        tros::print("  unknown: ");
        for &b in cmd { tros::putchar(b); }
        tros::print("\r\n  Type 'help' for commands.\r\n");
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    tros::print("sh: PANIC\r\n");
    loop { unsafe { core::arch::asm!("wfi"); } }
}
