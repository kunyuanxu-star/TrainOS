#![no_std]
#![no_main]

// TrainOS BusyBox V2 — multi-call system utilities
//
// Usage: bb <command> [args...]
// Commands: cp mv rm mkdir cat echo wc grep ls touch head help

use core::panic::PanicInfo;
use tros;

const ARG_MAX: usize = 4;
const ARG_LEN: usize = 32;

static mut ARGV: [[u8; ARG_LEN]; ARG_MAX] = [[0; ARG_LEN]; ARG_MAX];
static mut ARGC: usize = 0;

#[no_mangle]
extern "C" fn _start() -> ! {
    // Read command from stdin
    tros::print("bb> ");
    let mut cmd_buf = [0u8; 64];
    let mut cmd_len = 0;

    loop {
        let c = tros::getchar();
        if c == usize::MAX || c == 0 { tros::yield_cpu(); continue; }
        let b = c as u8;
        if b == b'\r' || b == b'\n' {
            if cmd_len > 0 {
                tros::print("\r\n");
                parse_and_run(&cmd_buf[..cmd_len]);
                cmd_len = 0;
                tros::print("bb> ");
            }
        } else if b == 0x7f || b == 0x08 {
            if cmd_len > 0 { cmd_len -= 1; tros::putchar(0x08); tros::putchar(b' '); tros::putchar(0x08); }
        } else if cmd_len < 63 {
            cmd_buf[cmd_len] = b; cmd_len += 1; tros::putchar(b);
        }
    }
}

fn parse_and_run(input: &[u8]) {
    unsafe { ARGC = 0; }
    let mut start = 0;
    for i in 0..=input.len() {
        if i == input.len() || input[i] == b' ' {
            if i > start && unsafe { ARGC < ARG_MAX } {
                let len = (i - start).min(ARG_LEN - 1);
                unsafe {
                    for j in 0..len { ARGV[ARGC][j] = input[start + j]; }
                    ARGV[ARGC][len] = 0;
                    ARGC += 1;
                }
            }
            start = i + 1;
        }
    }

    if unsafe { ARGC == 0 } { return; }

    let cmd = unsafe { &ARGV[0] };
    let cmd = core::str::from_utf8(cmd).unwrap_or("");
    // Find null terminator
    let cmd_end = cmd.bytes().position(|b| b == 0).unwrap_or(cmd.len());
    let cmd = &cmd[..cmd_end];

    match cmd {
        "cp" => cmd_cp(),
        "mv" => cmd_mv(),
        "rm" => cmd_rm(),
        "mkdir" => cmd_mkdir(),
        "cat" => cmd_cat(),
        "echo" => cmd_echo(),
        "wc" => cmd_wc(),
        "grep" => cmd_grep(),
        "ls" => cmd_ls(),
        "touch" => cmd_touch(),
        "head" => cmd_head(),
        "help" => cmd_help(),
        "" => {},
        other => { tros::print("bb: unknown command: "); tros::print(other); tros::print("\r\n"); }
    }
}

fn arg(idx: usize) -> &'static [u8] {
    if idx >= unsafe { ARGC } { return &[]; }
    unsafe {
        let a = &ARGV[idx];
        let mut len = 0;
        while len < ARG_LEN && a[len] != 0 { len += 1; }
        &a[..len]
    }
}

fn path_arg(idx: usize, out: &mut [u8; 64]) -> usize {
    let a = arg(idx);
    if a.is_empty() { return 0; }
    // Prepend "/" if not already present
    let mut off = 0;
    if a[0] != b'/' { out[0] = b'/'; off = 1; }
    let len = (a.len()).min(63 - off);
    for i in 0..len { out[off + i] = a[i]; }
    off + len
}

// ── Commands ──────────────────────────────────────────────────────────────────

fn cmd_cp() {
    if unsafe { ARGC } < 3 { tros::print("cp: usage: cp <src> <dst>\r\n"); return; }
    let mut src_path = [0u8; 64];
    let mut dst_path = [0u8; 64];
    let slen = path_arg(1, &mut src_path);
    let dlen = path_arg(2, &mut dst_path);
    if slen == 0 || dlen == 0 { tros::print("cp: invalid path\r\n"); return; }

    // Read source
    let fd = tros::open_bytes(&src_path[..slen]);
    if fd == usize::MAX { tros::print("cp: cannot open source\r\n"); return; }
    let mut buf = [0u8; 64];
    let n = tros::read(fd, &mut buf);
    tros::close(fd);

    // Write to destination
    let fd2 = tros::open_bytes(&dst_path[..dlen]);
    if fd2 == usize::MAX { tros::print("cp: cannot create dest\r\n"); return; }
    tros::write(fd2, &buf[..n]);
    tros::close(fd2);
    tros::print("cp: copied "); tros::print_uint(n); tros::print(" bytes\r\n");
}

fn cmd_mv() {
    if unsafe { ARGC } < 3 { tros::print("mv: usage: mv <src> <dst>\r\n"); return; }
    let mut src_path = [0u8; 64];
    let mut dst_path = [0u8; 64];
    let slen = path_arg(1, &mut src_path);
    let dlen = path_arg(2, &mut dst_path);
    if slen == 0 || dlen == 0 { return; }

    // Read source, write to dest, delete source
    let fd = tros::open_bytes(&src_path[..slen]);
    if fd == usize::MAX { tros::print("mv: source not found\r\n"); return; }
    let mut buf = [0u8; 64];
    let n = tros::read(fd, &mut buf);
    tros::close(fd);

    let fd2 = tros::open_bytes(&dst_path[..dlen]);
    if fd2 != usize::MAX {
        tros::write(fd2, &buf[..n]);
        tros::close(fd2);
    }

    // Delete source via VFS DELETE opcode
    let ep = tros::ep_create();
    let reply = tros::ep_create();
    let mut msg = [0u8; 64];
    msg[0] = reply as u8; msg[1] = (reply >> 8) as u8;
    msg[2] = slen as u8;
    for i in 0..slen { msg[3 + i] = src_path[i]; }
    tros::send(2, 5, &msg[..3 + slen]); // DELETE to VFS EP 2
    let mut _rb = [0u8; 64];
    tros::recv(reply, &mut _rb);
    tros::print("mv: moved\r\n");
}

fn cmd_rm() {
    if unsafe { ARGC } < 2 { tros::print("rm: usage: rm <path>\r\n"); return; }
    let mut path = [0u8; 64];
    let plen = path_arg(1, &mut path);
    if plen == 0 { return; }

    let ep = tros::ep_create();
    let reply = tros::ep_create();
    let mut msg = [0u8; 64];
    msg[0] = reply as u8; msg[1] = (reply >> 8) as u8;
    msg[2] = plen as u8;
    for i in 0..plen { msg[3 + i] = path[i]; }
    tros::send(2, 5, &msg[..3 + plen]);
    let mut _rb = [0u8; 64];
    tros::recv(reply, &mut _rb);
    tros::print("rm: deleted\r\n");
}

fn cmd_mkdir() {
    if unsafe { ARGC } < 2 { tros::print("mkdir: usage: mkdir <path>\r\n"); return; }
    let mut path = [0u8; 64];
    let plen = path_arg(1, &mut path);
    if plen == 0 { return; }

    let fd = tros::open_bytes(&path[..plen]);
    if fd != usize::MAX {
        tros::close(fd);
        tros::print("mkdir: created\r\n");
    } else {
        tros::print("mkdir: failed\r\n");
    }
}

fn cmd_cat() {
    if unsafe { ARGC } < 2 { tros::print("cat: usage: cat <path>\r\n"); return; }
    let mut path = [0u8; 64];
    let plen = path_arg(1, &mut path);
    if plen == 0 { return; }

    let fd = tros::open_bytes(&path[..plen]);
    if fd == usize::MAX { tros::print("cat: no such file\r\n"); return; }
    let mut buf = [0u8; 64];
    let n = tros::read(fd, &mut buf);
    tros::close(fd);
    for i in 0..n { tros::putchar(buf[i]); }
    tros::print("\r\n");
}

fn cmd_echo() {
    for i in 1..unsafe { ARGC } {
        if i > 1 { tros::putchar(b' '); }
        let a = arg(i);
        for &b in a { tros::putchar(b); }
    }
    tros::print("\r\n");
}

fn cmd_wc() {
    if unsafe { ARGC } < 2 { tros::print("wc: usage: wc <path>\r\n"); return; }
    let mut path = [0u8; 64];
    let plen = path_arg(1, &mut path);
    if plen == 0 { return; }

    let fd = tros::open_bytes(&path[..plen]);
    if fd == usize::MAX { tros::print("wc: no such file\r\n"); return; }
    let mut buf = [0u8; 64];
    let n = tros::read(fd, &mut buf);
    tros::close(fd);

    // Count lines, words, bytes
    let mut lines: usize = 0;
    let mut words: usize = 0;
    let mut in_word = false;
    for i in 0..n {
        if buf[i] == b'\n' { lines += 1; }
        if buf[i] == b' ' || buf[i] == b'\n' || buf[i] == b'\t' {
            in_word = false;
        } else if !in_word {
            words += 1; in_word = true;
        }
    }
    tros::print("  "); tros::print_uint(lines);
    tros::print(" "); tros::print_uint(words);
    tros::print(" "); tros::print_uint(n);
    tros::print("\r\n");
}

fn cmd_grep() {
    if unsafe { ARGC } < 3 { tros::print("grep: usage: grep <pattern> <path>\r\n"); return; }
    let pattern = arg(1);
    let mut path = [0u8; 64];
    let plen = path_arg(2, &mut path);
    if plen == 0 { return; }

    let fd = tros::open_bytes(&path[..plen]);
    if fd == usize::MAX { tros::print("grep: no such file\r\n"); return; }
    let mut buf = [0u8; 64];
    let n = tros::read(fd, &mut buf);
    tros::close(fd);

    // Simple substring search
    for i in 0..n {
        let mut matched = true;
        if i + pattern.len() > n { break; }
        for j in 0..pattern.len() {
            if buf[i + j] != pattern[j] { matched = false; break; }
        }
        if matched {
            // Print the matching line
            let start = {
                let mut s = i;
                while s > 0 && buf[s - 1] != b'\n' { s -= 1; }
                s
            };
            let end = {
                let mut e = i + pattern.len();
                while e < n && buf[e] != b'\n' { e += 1; }
                e
            };
            for k in start..end { tros::putchar(buf[k]); }
            tros::print("\r\n");
        }
    }
}

fn cmd_ls() {
    let mut buf = [0u8; 64];
    let n = tros::getdents64(0, &mut buf);
    for i in 0..n {
        if buf[i] == b'\n' { tros::print("\r\n"); }
        else if buf[i] != 0 { tros::putchar(buf[i]); }
    }
    if n == 0 { tros::print("(empty)\r\n"); }
    else { tros::print("\r\n"); }
}

fn cmd_touch() {
    if unsafe { ARGC } < 2 { tros::print("touch: usage: touch <path>\r\n"); return; }
    let mut path = [0u8; 64];
    let plen = path_arg(1, &mut path);
    if plen == 0 { return; }
    let fd = tros::open_bytes(&path[..plen]);
    if fd != usize::MAX {
        tros::close(fd);
        tros::print("touch: ok\r\n");
    } else {
        tros::print("touch: failed\r\n");
    }
}

fn cmd_head() {
    if unsafe { ARGC } < 2 { tros::print("head: usage: head <path>\r\n"); return; }
    let mut path = [0u8; 64];
    let plen = path_arg(1, &mut path);
    if plen == 0 { return; }
    let fd = tros::open_bytes(&path[..plen]);
    if fd == usize::MAX { tros::print("head: no such file\r\n"); return; }
    let mut buf = [0u8; 64];
    let n = tros::read(fd, &mut buf);
    tros::close(fd);
    let mut lines = 0;
    for i in 0..n {
        if lines >= 10 { break; }
        tros::putchar(buf[i]);
        if buf[i] == b'\n' { lines += 1; }
    }
    if n > 0 && buf[n-1] != b'\n' { tros::print("\r\n"); }
}

fn cmd_help() {
    tros::print("TrainOS BusyBox V2\r\n");
    tros::print("Commands: cp mv rm mkdir cat echo wc grep ls touch head help\r\n");
    tros::print("Usage: bb <command> [args...]\r\n");
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    tros::print("bb: PANIC\r\n");
    loop { unsafe { core::arch::asm!("wfi"); } }
}
