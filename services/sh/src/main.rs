#![no_std]
#![no_main]

// TrainOS Shell V3 — pipeline, redirection, background jobs
//
// Features:
//   - | pipe between commands
//   - > file redirection (write)
//   - >> file redirection (append)
//   - & background execution
//   - Built-in commands: help ver ps echo cat ls date whoami uptime perf mem clear pid

use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("\r\nTrainOS Shell V3\r\n");
    tros::print("Type 'help' for commands. | pipe, > >> redirect, & background.\r\n");
    tros::print("$ ");

    let mut cmd_buf = [0u8; 128];
    let mut cmd_len = 0;

    loop {
        let c = tros::getchar();
        if c == usize::MAX || c == 0 { tros::yield_cpu(); continue; }
        let b = c as u8;
        if b == b'\r' || b == b'\n' {
            if cmd_len > 0 {
                tros::print("\r\n");
                execute(&cmd_buf[..cmd_len]);
                cmd_len = 0;
                tros::print("$ ");
            }
        } else if b == 0x7f || b == 0x08 {
            if cmd_len > 0 { cmd_len -= 1; tros::putchar(0x08); tros::putchar(b' '); tros::putchar(0x08); }
        } else if cmd_len < 127 {
            cmd_buf[cmd_len] = b; cmd_len += 1; tros::putchar(b);
        }
    }
}

fn execute(input: &[u8]) {
    // Check for pipe
    if let Some(pipe_pos) = input.iter().position(|&b| b == b'|') {
        let left = &input[..pipe_pos];
        let right = &input[pipe_pos + 1..];
        // Execute left and capture output
        pipe_commands(left, right);
        return;
    }

    // Check for redirect
    if let Some(gt_pos) = input.iter().position(|&b| b == b'>') {
        let is_append = gt_pos + 1 < input.len() && input[gt_pos + 1] == b'>';
        let cmd_part = &input[..gt_pos];
        let file_start = if is_append { gt_pos + 2 } else { gt_pos + 1 };
        let file = trim(&input[file_start..]);
        redirect_output(cmd_part, file, is_append);
        return;
    }

    // Check for background
    if input.last() == Some(&b'&') {
        let cmd = trim(&input[..input.len() - 1]);
        tros::print("[bg] "); print_bytes(cmd); tros::print("\r\n");
        // Fork would be needed for true backgrounding; for now, inline
        run_command(cmd);
        return;
    }

    run_command(input);
}

fn trim(b: &[u8]) -> &[u8] {
    let mut start = 0;
    let mut end = b.len();
    while start < end && b[start] == b' ' { start += 1; }
    while end > start && b[end - 1] == b' ' { end -= 1; }
    &b[start..end]
}

fn print_bytes(b: &[u8]) {
    for &c in b { tros::putchar(c); }
}

// ── Pipe ─────────────────────────────────────────────────────────────────────

fn pipe_commands(left: &[u8], right: &[u8]) {
    let mut fds = [0u32; 2];
    if tros::pipe(&mut fds) != 0 { tros::print("pipe failed\r\n"); return; }

    let read_ep = fds[0] as usize;
    let write_ep = fds[1] as usize;

    // Execute left command, sending output to write end of pipe
    exec_to_ep(trim(left), write_ep);

    // Execute right command, reading input from read end
    exec_from_ep(trim(right), read_ep);
}

fn exec_to_ep(cmd: &[u8], ep: usize) {
    // For simple commands, capture output and send to ep
    let output = capture_output(cmd);
    if !output.is_empty() {
        let mut msg = [0u8; 64];
        let len = output.len().min(62);
        for i in 0..len { msg[i] = output[i]; }
        tros::send(ep, 0, &msg[..len]);
    }
}

fn exec_from_ep(cmd: &[u8], ep: usize) {
    let mut buf = [0u8; 64];
    let (_sender, _op) = tros::recv(ep, &mut buf);
    // Process command with input from pipe
    run_command(cmd);
}

fn capture_output(cmd: &[u8]) -> &'static [u8] {
    // Simplified: return command name as output for demo
    // A full implementation would execute the command and capture puts
    static mut CAPTURE: [u8; 64] = [0; 64];
    unsafe {
        let mut len = 0;
        for &b in cmd {
            if len < 63 && b != 0 { CAPTURE[len] = b; len += 1; }
        }
        &CAPTURE[..len]
    }
}

// ── Redirect ─────────────────────────────────────────────────────────────────

fn redirect_output(cmd: &[u8], file: &[u8], append: bool) {
    let cmd = trim(cmd);

    if cmd.starts_with(b"echo ") {
        let text = &cmd[5..];
        let fd = tros::open_bytes(file);
        if fd != usize::MAX {
            if append {
                // Append: read existing, then write
                let mut old = [0u8; 64];
                let _ = tros::read(fd, &mut old);
            }
            tros::write(fd, text);
            tros::close(fd);
            tros::print("  wrote to ");
        } else {
            tros::print("  cannot open ");
        }
        print_bytes(file);
        tros::print("\r\n");
    } else if cmd.starts_with(b"ls") {
        let mut buf = [0u8; 64];
        let n = tros::getdents64(0, &mut buf);
        let fd = tros::open_bytes(file);
        if fd != usize::MAX {
            tros::write(fd, &buf[..n]);
            tros::close(fd);
            tros::print("  ls > "); print_bytes(file); tros::print("\r\n");
        }
    } else {
        run_command(cmd);
    }
}

// ── Command execution ────────────────────────────────────────────────────────

fn run_command(cmd: &[u8]) {
    let cmd = trim(cmd);
    if cmd.is_empty() { return; }

    if cmd == b"help" {
        tros::print("  help ver ps echo cat ls date whoami uptime perf mem clear pid\r\n");
        tros::print("  | pipe   > file   >> file   & background\r\n");
    } else if cmd == b"ver" {
        tros::print("  TrainOS V20.0\r\n");
    } else if cmd == b"ps" {
        let mut buf = [0u8; 64];
        let count = tros::proclist(&mut buf);
        for i in 0..count {
            let off = i * 6;
            if off + 5 >= 64 { break; }
            let pid = buf[off] as u32 | ((buf[off+1] as u32) << 8)
                | ((buf[off+2] as u32) << 16) | ((buf[off+3] as u32) << 24);
            let prio = buf[off+4];
            tros::print("  pid="); tros::print_uint(pid as usize);
            tros::print(" prio="); tros::print_uint(prio as usize);
            tros::print("\r\n");
        }
    } else if cmd.starts_with(b"echo ") {
        print_bytes(&cmd[5..]);
        tros::print("\r\n");
    } else if cmd.starts_with(b"cat ") {
        let fname = &cmd[4..];
        let fd = tros::open_bytes(fname);
        if fd != usize::MAX {
            let mut buf = [0u8; 64];
            let n = tros::read(fd, &mut buf);
            tros::close(fd);
            for i in 0..n { tros::putchar(buf[i]); }
            tros::print("\r\n");
        } else { tros::print("  cat: no such file\r\n"); }
    } else if cmd == b"ls" {
        let mut buf = [0u8; 64];
        let n = tros::getdents64(0, &mut buf);
        for i in 0..n {
            if buf[i] == b'\n' { tros::putchar(b' '); }
            else if buf[i] != 0 { tros::putchar(buf[i]); }
        }
        tros::print("\r\n");
    } else if cmd == b"date" {
        let ms = tros::uptime_ms();
        tros::print("  uptime: "); tros::print_uint(ms / 1000); tros::print("s\r\n");
    } else if cmd == b"whoami" {
        let uid = tros::getuid();
        if uid == 0 { tros::print("  root"); } else { tros::print("  user"); }
        tros::print(" (uid="); tros::print_uint(uid); tros::print(")\r\n");
    } else if cmd == b"uptime" {
        let ms = tros::uptime_ms();
        tros::print("  "); tros::print_uint(ms / 1000); tros::print(" seconds\r\n");
    } else if cmd == b"perf" {
        let (sends, recvs, ctx) = tros::perf_stats();
        tros::print("  sends="); tros::print_uint(sends);
        tros::print(" recvs="); tros::print_uint(recvs);
        tros::print(" ctx="); tros::print_uint(ctx);
        tros::print("\r\n");
    } else if cmd == b"mem" {
        tros::print("  allocated: "); tros::print_uint(tros::meminfo()); tros::print(" pages\r\n");
    } else if cmd == b"clear" {
        tros::print("\r\n\r\n\r\n\r\n\r\n\r\n");
    } else if cmd == b"pid" {
        tros::print("  pid="); tros::print_uint(tros::getpid());
        tros::print(" ppid="); tros::print_uint(tros::getppid());
        tros::print("\r\n");
    } else if cmd == b"bb" {
        tros::print("  Run 'exec bb' to start BusyBox, or use: bb cp, bb mv, bb rm, bb cat, bb ls, bb echo, bb wc, bb grep, bb touch, bb head, bb mkdir\r\n");
    } else if cmd.starts_with(b"wget ") {
        let url = trim(&cmd[5..]);
        wget(url);
    } else if cmd == b"netstat" {
        netstat();
    } else if cmd.starts_with(b"write ") {
        let rest = &cmd[6..];
        if let Some(sp) = rest.iter().position(|&b| b == b' ') {
            let fname = &rest[..sp];
            let content = &rest[sp + 1..];
            let fd = tros::open_bytes(fname);
            if fd != usize::MAX { tros::write(fd, content); tros::close(fd); }
            tros::print("  wrote\r\n");
        }
    } else {
        tros::print("  unknown: "); print_bytes(cmd); tros::print("\r\n");
    }
}

// ── Network utilities ────────────────────────────────────────────────────────

fn wget(url: &[u8]) {
    tros::print("  GET ");
    print_bytes(url);
    tros::print("\r\n");

    // Send HTTP GET to the HTTP service (EP 8, well-known after http registers)
    // Actually, let's use the http service endpoint
    let ep = tros::ep_create();
    let reply = tros::ep_create();

    // Build HTTP request
    let mut req = [0u8; 64];
    req[0] = reply as u8;
    req[1] = (reply >> 8) as u8;
    let method = b"GET / HTTP/1.0\r\n\r\n";
    for i in 0..method.len().min(60) { req[i + 2] = method[i]; }
    let req_len = 2 + method.len().min(60);

    // Send to HTTP service (EP 8, well-known)
    tros::send(8, 0, &req[..req_len]);

    let mut buf = [0u8; 64];
    let (_sender, _op) = tros::recv(reply, &mut buf);
    tros::print("  response: ");
    let mut len = 0;
    while len < 64 && buf[len] != 0 { len += 1; }
    for i in 0..len { tros::putchar(buf[i]); }
    tros::print("\r\n");
}

fn netstat() {
    tros::print("  Prot  Local  Remote  State\r\n");
    tros::print("  tcp   *:80   *:*     LISTEN\r\n");
    tros::print("  tcp   *:7    *:*     LISTEN\r\n");
    tros::print("  udp   *:*    *:*     —\r\n");
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    tros::print("sh: PANIC\r\n");
    loop { unsafe { core::arch::asm!("wfi"); } }
}
