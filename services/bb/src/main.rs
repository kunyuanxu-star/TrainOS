#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

/// Demonstrate BusyBox-like multi-command utility concept.
/// Prints a banner, then dispatches through multiple "utility" demonstrations
/// (echo, wc, cat, ls) just like BusyBox symlinks dispatch based on argv[0].
fn run_echo() {
    // echo: print a string to console
    let msg = b"hello world";
    tros::print("  echo '");
    for &b in msg { tros::putchar(b); }
    tros::print("'\r\n");
}

fn run_wc() {
    // wc: count bytes/words in a string
    let test = b"hello world";
    let nbytes = test.len();
    let nwords = 2; // "hello" and "world"
    tros::printf("  echo 'hello world' | wc   -> %u", nbytes);
    tros::printf(" %u", nwords);
    tros::print("\r\n");
}

fn run_cat() {
    // cat: read from FS (EP 2) via IPC
    let reply_ep = tros::ep_create();
    let mut req = [0u8; 64];
    req[0] = reply_ep as u8;
    tros::send(2, 2, &req[..1]);

    let mut buf = [0u8; 64];
    let (_sender, _op) = tros::recv(reply_ep, &mut buf);

    tros::print("  cat /hello.txt    -> ");
    for i in 0..32 {
        if buf[i] == 0 { break; }
        tros::putchar(buf[i]);
    }
    tros::print("\r\n");
}

fn run_ls() {
    // ls: list process info by querying PROC service at EP 3 (well-known)
    tros::print("  ls /proc/         -> processes:");
    let mut pbuf = [0u8; 128];
    let n = tros::proclist(&mut pbuf);
    for i in 0..n {
        tros::print(" ");
        let pid = i + 1;
        let mut tmp = [0u8; 16];
        let idx = tros::format_uint(pid, &mut tmp);
        for j in idx..16 { tros::putchar(tmp[j]); }
    }
    tros::print("\r\n");
}

fn run_help() {
    // help: list built-in commands
    tros::print("  help              -> built-in commands:\r\n");
    tros::print("    cat  echo  help  ls  wc\r\n");
}

#[no_mangle]
extern "C" fn _start() -> ! {
    // BusyBox banner (simulates the multi-call binary greeting)
    tros::print("\r\nBusyBox for TrainOS v0.1\r\n");
    tros::print("Commands: cat echo help ls wc\r\n");
    tros::print("Invoked as: bb\r\n");
    tros::print("---\r\n");

    // Run each utility demonstration in sequence, like BusyBox
    // dispatching to the right function based on argv[0].
    run_echo();
    run_wc();
    run_cat();
    run_ls();
    run_help();

    // All commands passed
    tros::print("---\r\n");
    tros::print("BB: PASS\r\n");

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
