#![no_std]
#![no_main]

// TrainOS VFS Service — Virtual File System with procfs support
//
// Operations (opcode):
//   2 = READ(path)     — read file content
//   3 = WRITE(path)    — write/create file
//   4 = APPEND(path)   — append to file
//   5 = DELETE(path)   — delete file
//   6 = LIST(dir_path) — list directory entries
//   7 = STAT(path)     — file metadata (size, type)
//
// /proc virtual files (generated dynamically from kernel):
//   /proc/uptime    — system uptime in ms
//   /proc/meminfo   — memory statistics
//   /proc/proc      — process listing
//   /proc/version   — TrainOS version string
//   /proc/perf      — performance counters

use core::panic::PanicInfo;
use tros;

// File slot: stores one file's content
const SLOT_SIZE: usize = 64;
const MAX_FILES: usize = 16;
const MAX_PATH: usize = 32;

struct FileSlot {
    path: [u8; MAX_PATH],
    path_len: usize,
    data: [u8; SLOT_SIZE],
    data_len: usize,
    is_dir: bool,
    used: bool,
}

impl FileSlot {
    const fn new() -> Self {
        FileSlot {
            path: [0; MAX_PATH],
            path_len: 0,
            data: [0; SLOT_SIZE],
            data_len: 0,
            is_dir: false,
            used: false,
        }
    }
}

static mut SLOTS: [FileSlot; MAX_FILES] = [
    FileSlot::new(), FileSlot::new(), FileSlot::new(), FileSlot::new(),
    FileSlot::new(), FileSlot::new(), FileSlot::new(), FileSlot::new(),
    FileSlot::new(), FileSlot::new(), FileSlot::new(), FileSlot::new(),
    FileSlot::new(), FileSlot::new(), FileSlot::new(), FileSlot::new(),
];

// Pre-populate paths during init
unsafe fn init_vfs() {
    // Root directory
    make_slot(b"/\0", 0, &[], true);
    // /proc directory
    make_slot(b"/proc\0", 0, &[], true);
    // /home directory
    make_slot(b"/home\0", 0, &[], true);
    // /etc directory
    make_slot(b"/etc\0", 0, &[], true);
    // /tmp directory
    make_slot(b"/tmp\0", 0, &[], true);
}

unsafe fn make_slot(path: &[u8], existing_id: usize, data: &[u8], is_dir: bool) -> usize {
    // Check if path already exists
    for i in 0..MAX_FILES {
        if SLOTS[i].used && SLOTS[i].path_len == path.len() {
            let mut matches = true;
            for j in 0..path.len() {
                if SLOTS[i].path[j] != path[j] {
                    matches = false;
                    break;
                }
            }
            if matches {
                return i;
            }
        }
    }
    // Allocate new slot
    for i in 0..MAX_FILES {
        if !SLOTS[i].used {
            SLOTS[i].used = true;
            SLOTS[i].is_dir = is_dir;
            SLOTS[i].path_len = path.len();
            for j in 0..path.len().min(MAX_PATH) {
                SLOTS[i].path[j] = path[j];
            }
            let dlen = data.len().min(SLOT_SIZE);
            for j in 0..dlen {
                SLOTS[i].data[j] = data[j];
            }
            SLOTS[i].data_len = dlen;
            return i;
        }
    }
    existing_id
}

unsafe fn find_slot(path: &[u8]) -> Option<usize> {
    for i in 0..MAX_FILES {
        if !SLOTS[i].used {
            continue;
        }
        if SLOTS[i].path_len == path.len() {
            let mut matches = true;
            for j in 0..path.len() {
                if SLOTS[i].path[j] != path[j] {
                    matches = false;
                    break;
                }
            }
            if matches {
                return Some(i);
            }
        }
    }
    None
}

/// Generate /proc data dynamically
unsafe fn proc_read(path: &[u8]) -> Option<[u8; SLOT_SIZE]> {
    let mut result = [0u8; SLOT_SIZE];
    let mut rlen = 0;

    // /proc/uptime
    if path_match(path, b"/proc/uptime\0") {
        let ticks = tros::uptime_ms();
        rlen = fmt_num(ticks, &mut result, 0);
        return Some(result);
    }
    // /proc/meminfo
    if path_match(path, b"/proc/meminfo\0") {
        let pages = tros::meminfo();
        let s = b"allocated_pages: ";
        for (j, &c) in s.iter().enumerate() { result[j] = c; }
        rlen = fmt_num(pages, &mut result, s.len());
        return Some(result);
    }
    // /proc/perf
    if path_match(path, b"/proc/perf\0") {
        let (sends, recvs, ctx) = tros::perf_stats();
        let hdr = b"sends=";
        for (j, &c) in hdr.iter().enumerate() { result[j] = c; }
        rlen = fmt_num(sends, &mut result, hdr.len());
        result[rlen] = b' '; rlen += 1;
        let hdr2 = b"recvs=";
        for (j, &c) in hdr2.iter().enumerate() { result[rlen + j] = c; }
        rlen = fmt_num_at(recvs, &mut result, rlen + hdr2.len());
        result[rlen] = b' '; rlen += 1;
        let hdr3 = b"ctx=";
        for (j, &c) in hdr3.iter().enumerate() { result[rlen + j] = c; }
        rlen = fmt_num_at(ctx, &mut result, rlen + hdr3.len());
        return Some(result);
    }
    // /proc/version
    if path_match(path, b"/proc/version\0") {
        let ver = b"TrainOS V13.0 -- Microkernel OS";
        for (j, &c) in ver.iter().enumerate() { result[j] = c; }
        return Some(result);
    }
    // /proc/proc — process listing
    if path_match(path, b"/proc/proc\0") {
        // Fetch process list via kernel syscall
        let mut buf = [0u8; 64];
        let count = tros::proclist(&mut buf);
        rlen = count * 6;
        let rlen = rlen.min(SLOT_SIZE);
        for j in 0..rlen {
            result[j] = buf[j];
        }
        return Some(result);
    }
    // /proc/self — current PID
    if path_match(path, b"/proc/self\0") {
        let pid = tros::getpid();
        rlen = fmt_num(pid, &mut result, 0);
        return Some(result);
    }

    None
}

fn path_match(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() && a.len() + 1 != b.len() {
        return false;
    }
    let len = a.len().min(b.len());
    for i in 0..len {
        if a[i] != b[i] { return false; }
    }
    true
}

fn fmt_num(n: usize, buf: &mut [u8], off: usize) -> usize {
    fmt_num_at(n, buf, off)
}

fn fmt_num_at(mut n: usize, buf: &mut [u8], mut off: usize) -> usize {
    if n == 0 {
        buf[off] = b'0'; return off + 1;
    }
    let mut tmp = [0u8; 20];
    let mut ti = 20;
    loop {
        ti -= 1;
        tmp[ti] = b'0' + (n - (n / 10) * 10) as u8;
        n /= 10;
        if n == 0 { break; }
    }
    for j in ti..20 {
        buf[off] = tmp[j];
        off += 1;
    }
    off
}

#[no_mangle]
extern "C" fn _start() -> ! {
    let my_ep = 2;
    tros::print("VFS: ep=2 listening\r\n");

    unsafe { init_vfs(); }

    let mut buf = [0u8; 64];

    loop {
        let (sender_pid, opcode) = tros::recv(my_ep, &mut buf);
        if sender_pid == usize::MAX {
            continue;
        }

        // Parse path and reply_ep from message
        // Format: [reply_ep:2] [path_len:1] [path:path_len] [data_len:1] [data:data_len]
        let reply_ep = buf[0] as usize | ((buf[1] as usize) << 8);
        let path_len = (buf[2] as usize).min(MAX_PATH);
        let path = &buf[3..3 + path_len];

        match opcode {
            2 => {
                // READ
                let data = unsafe {
                    // Check /proc first
                    if path_len >= 5 {
                        let proc_data = proc_read(path);
                        if proc_data.is_some() {
                            proc_data
                        } else {
                            find_slot(path).map(|idx| {
                                let mut d = [0u8; SLOT_SIZE];
                                let len = SLOTS[idx].data_len.min(SLOT_SIZE);
                                for j in 0..len { d[j] = SLOTS[idx].data[j]; }
                                d
                            })
                        }
                    } else {
                        None
                    }
                };
                match data {
                    Some(d) => { tros::send(reply_ep, 0, &d); }
                    None => { tros::send(reply_ep, 0, b"ENOENT"); }
                }
            }
            3 => {
                // WRITE: create/overwrite file
                let data_off = 3 + path_len;
                let data_len = if data_off < 64 { buf[data_off] as usize } else { 0 };
                let data_start = data_off + 1;
                let dlen = data_len.min(SLOT_SIZE);
                let write_data = if data_start + dlen <= 64 { &buf[data_start..data_start + dlen] } else { &[] };

                unsafe {
                    let idx = make_slot(path, 0, write_data, false);
                    if !SLOTS[idx].is_dir {
                        for j in 0..dlen { SLOTS[idx].data[j] = write_data[j]; }
                        SLOTS[idx].data_len = dlen;
                    }
                }
                tros::send(reply_ep, 0, b"OK");
            }
            4 => {
                // APPEND
                let data_off = 3 + path_len;
                let data_len = if data_off < 64 { buf[data_off] as usize } else { 0 };
                let data_start = data_off + 1;
                let dlen = data_len.min(SLOT_SIZE);
                let append_data = if data_start + dlen <= 64 { &buf[data_start..data_start + dlen] } else { &[] };

                unsafe {
                    let idx = make_slot(path, 0, append_data, false);
                    let cur_len = SLOTS[idx].data_len;
                    let new_len = (cur_len + dlen).min(SLOT_SIZE);
                    for j in 0..(new_len - cur_len) {
                        SLOTS[idx].data[cur_len + j] = append_data[j];
                    }
                    SLOTS[idx].data_len = new_len;
                }
                tros::send(reply_ep, 0, b"OK");
            }
            5 => {
                // DELETE
                unsafe {
                    if let Some(idx) = find_slot(path) {
                        SLOTS[idx].used = false;
                    }
                }
                tros::send(reply_ep, 0, b"OK");
            }
            6 => {
                // LIST directory
                let mut listing = [0u8; 64];
                let mut pos: usize = 0;
                let dir_path = path;

                unsafe {
                    for i in 0..MAX_FILES {
                        if !SLOTS[i].used { continue; }
                        let ref p = SLOTS[i].path;
                        let plen = SLOTS[i].path_len;
                        // Check if file is in this directory
                        if plen > dir_path.len() && starts_with(&p[..plen], dir_path) {
                            // Extract just the filename (skip the directory prefix)
                            let rest = &p[dir_path.len()..plen];
                            // Only include direct children (no deeper paths)
                            let is_direct = !contains_slash(rest);
                            if is_direct && pos + rest.len() + 1 <= 64 {
                                for j in 0..rest.len() {
                                    listing[pos + j] = rest[j];
                                }
                                pos += rest.len();
                                listing[pos] = b'\n';
                                pos += 1;
                            }
                        }
                    }
                }
                tros::send(reply_ep, 0, &listing[..pos]);
            }
            7 => {
                // STAT
                unsafe {
                    if let Some(idx) = find_slot(path) {
                        let size = SLOTS[idx].data_len;
                        let is_dir = if SLOTS[idx].is_dir { 1u8 } else { 0u8 };
                        let resp = [size as u8, is_dir];
                        tros::send(reply_ep, 0, &resp);
                    } else {
                        tros::send(reply_ep, 0, b"ENOENT");
                    }
                }
            }
            _ => {}
        }
    }
}

fn starts_with(s: &[u8], prefix: &[u8]) -> bool {
    if s.len() < prefix.len() { return false; }
    for i in 0..prefix.len() {
        if s[i] != prefix[i] { return false; }
    }
    true
}

fn contains_slash(s: &[u8]) -> bool {
    for &c in s.iter() {
        if c == b'/' { return true; }
    }
    false
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    tros::print("VFS: PANIC\r\n");
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}
