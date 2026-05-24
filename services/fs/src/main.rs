#![no_std]
#![no_main]

// TrainOS VFS Service V3 — Virtual File System with disk persistence
//
// Architecture:
//   - In-memory cache: 16 file slots, 64 bytes each (fast path)
//   - Disk persistence: file data mirrored to disk via blk_read/blk_write
//   - Superblock at sector 0: magic(8) + version(4) + file_count(4) = 16 bytes
//   - File table at sector 1: 16 entries x 8 bytes each (name_hash:4 + data_sector:4)
//   - File data: each file starts at its own sector (8 sectors per file, 4KB)
//
// Operations (opcode to EP 2):
//   2 = READ(path)     — read file content
//   3 = WRITE(path)    — write/create file
//   4 = APPEND(path)   — append to file
//   5 = DELETE(path)   — delete file
//   6 = LIST(dir)      — list directory entries
//   7 = STAT(path)     — file metadata

use core::panic::PanicInfo;
use tros;

const SLOT_SIZE: usize = 64;
const MAX_FILES: usize = 16;
const MAX_PATH: usize = 32;
const SECTORS_PER_FILE: usize = 8;
const FILE_DATA_BASE: usize = 16; // sectors 0-15 reserved

#[derive(Clone, Copy)]
struct FileSlot {
    path: [u8; MAX_PATH],
    path_len: usize,
    data: [u8; SLOT_SIZE],
    data_len: usize,
    is_dir: bool,
    used: bool,
    dirty: bool,
    disk_sector: usize, // starting sector on disk
}

impl FileSlot {
    const fn new() -> Self {
        FileSlot { path: [0; MAX_PATH], path_len: 0, data: [0; SLOT_SIZE], data_len: 0,
                   is_dir: false, used: false, dirty: false, disk_sector: 0 }
    }
}

static mut SLOTS: [FileSlot; MAX_FILES] = [FileSlot::new(); 16];

// ── Disk persistence ─────────────────────────────────────────────────────────

const SUPERBLOCK_SECTOR: usize = 0;
const FILE_TABLE_SECTOR: usize = 1;

unsafe fn disk_read(sector: usize, buf: &mut [u8]) -> bool {
    let result = tros::blk_read(sector, buf);
    result > 0
}

unsafe fn disk_write(sector: usize, data: &[u8]) -> bool {
    let mut sector_buf = [0u8; 512];
    let len = data.len().min(512);
    for i in 0..len { sector_buf[i] = data[i]; }
    let result = tros::blk_write(sector, &sector_buf);
    result > 0
}

unsafe fn load_from_disk() -> bool {
    // Read superblock
    let mut sb = [0u8; 512];
    if !disk_read(SUPERBLOCK_SECTOR, &mut sb) { return false; }
    if &sb[0..8] != b"TRAINOS2" { return false; }

    // Read file table
    let mut ft = [0u8; 512];
    if !disk_read(FILE_TABLE_SECTOR, &mut ft) { return false; }

    for i in 0..MAX_FILES {
        let off = i * 16;
        let name_len = ft[off] as usize;
        if name_len == 0 || name_len > MAX_PATH { continue; }
        let data_len = ft[off + 1] as usize;
        let disk_sector = (ft[off + 2] as usize) | ((ft[off + 3] as usize) << 8);

        SLOTS[i].used = true;
        SLOTS[i].path_len = name_len;
        for j in 0..name_len { SLOTS[i].path[j] = ft[off + 4 + j]; }
        SLOTS[i].data_len = data_len;
        SLOTS[i].disk_sector = disk_sector;
        SLOTS[i].is_dir = name_len > 0 && SLOTS[i].path[name_len - 1] == b'/';

        // Load file data from disk
        if data_len > 0 && disk_sector > 0 {
            let mut data_buf = [0u8; 512];
            disk_read(disk_sector, &mut data_buf);
            let copy_len = data_len.min(SLOT_SIZE);
            for j in 0..copy_len { SLOTS[i].data[j] = data_buf[j]; }
        }
    }

    tros::print("VFS: loaded from disk\r\n");
    true
}

unsafe fn save_to_disk() {
    // Write superblock
    let mut sb = [0u8; 512];
    sb[0..8].copy_from_slice(b"TRAINOS2");
    sb[8] = 3; sb[9] = 0; sb[10] = 0; sb[11] = 0; // version 3
    // file_count
    let mut count: u32 = 0;
    for i in 0..MAX_FILES { if SLOTS[i].used { count += 1; } }
    sb[12] = count as u8;
    sb[13] = (count >> 8) as u8;
    disk_write(SUPERBLOCK_SECTOR, &sb);

    // Write file table
    let mut ft = [0u8; 512];
    for i in 0..MAX_FILES {
        if !SLOTS[i].used { continue; }
        let off = i * 16;
        ft[off] = SLOTS[i].path_len as u8;
        ft[off + 1] = SLOTS[i].data_len as u8;
        ft[off + 2] = SLOTS[i].disk_sector as u8;
        ft[off + 3] = (SLOTS[i].disk_sector >> 8) as u8;
        for j in 0..SLOTS[i].path_len { ft[off + 4 + j] = SLOTS[i].path[j]; }
    }
    disk_write(FILE_TABLE_SECTOR, &ft);

    // Write dirty file data
    for i in 0..MAX_FILES {
        if SLOTS[i].used && SLOTS[i].dirty && SLOTS[i].disk_sector > 0 {
            disk_write(SLOTS[i].disk_sector, &SLOTS[i].data);
            SLOTS[i].dirty = false;
        }
    }
}

unsafe fn alloc_disk_sector() -> usize {
    // Simple: find an unused sector
    let mut used = [false; 128];
    for i in 0..MAX_FILES {
        if SLOTS[i].used && SLOTS[i].disk_sector > 0 {
            let sec = SLOTS[i].disk_sector;
            if sec < 128 { used[sec] = true; }
        }
    }
    for s in FILE_DATA_BASE..128 {
        if !used[s] { return s; }
    }
    FILE_DATA_BASE // fallback
}

// ── VFS operations ───────────────────────────────────────────────────────────

unsafe fn init_vfs() {
    // Default directories (created if not loaded from disk)
    if SLOTS.iter().all(|s| !s.used) {
        make_slot(b"/\0", true);
        make_slot(b"/proc\0", true);
        make_slot(b"/home\0", true);
        make_slot(b"/etc\0", true);
        make_slot(b"/tmp\0", true);

        // Pre-populate welcome file
        let welcome = b"Welcome to TrainOS V20.0 -- a microkernel OS for RISC-V";
        make_file(b"/welcome.txt\0", welcome);
    }
}

unsafe fn make_slot(path: &[u8], is_dir: bool) -> usize {
    for i in 0..MAX_FILES {
        if !SLOTS[i].used {
            SLOTS[i].used = true;
            SLOTS[i].is_dir = is_dir;
            SLOTS[i].path_len = path.len().min(MAX_PATH);
            for j in 0..SLOTS[i].path_len { SLOTS[i].path[j] = path[j]; }
            SLOTS[i].disk_sector = alloc_disk_sector();
            SLOTS[i].dirty = true;
            return i;
        }
    }
    0
}

unsafe fn make_file(path: &[u8], data: &[u8]) -> usize {
    let idx = make_slot(path, false);
    let dlen = data.len().min(SLOT_SIZE);
    SLOTS[idx].data_len = dlen;
    for j in 0..dlen { SLOTS[idx].data[j] = data[j]; }
    SLOTS[idx].dirty = true;
    idx
}

unsafe fn find_slot(path: &[u8]) -> Option<usize> {
    for i in 0..MAX_FILES {
        if !SLOTS[i].used { continue; }
        if SLOTS[i].path_len == path.len() {
            let mut matches = true;
            for j in 0..path.len() {
                if SLOTS[i].path[j] != path[j] { matches = false; break; }
            }
            if matches { return Some(i); }
        }
    }
    None
}

unsafe fn proc_read(path: &[u8]) -> Option<[u8; SLOT_SIZE]> {
    let mut result = [0u8; SLOT_SIZE];
    if path_match(path, b"/proc/uptime\0") {
        fmt_num(tros::uptime_ms(), &mut result, 0);
        return Some(result);
    }
    if path_match(path, b"/proc/meminfo\0") {
        let pages = tros::meminfo();
        let s = b"allocated_pages: ";
        for (j, &c) in s.iter().enumerate() { result[j] = c; }
        fmt_num(pages, &mut result, s.len());
        return Some(result);
    }
    if path_match(path, b"/proc/perf\0") {
        let (sends, recvs, ctx) = tros::perf_stats();
        let hdr = b"sends=";
        for (j, &c) in hdr.iter().enumerate() { result[j] = c; }
        let mut rlen = fmt_num(sends, &mut result, hdr.len());
        result[rlen] = b' '; rlen += 1;
        let hdr2 = b"recvs=";
        for (j, &c) in hdr2.iter().enumerate() { result[rlen + j] = c; }
        rlen = fmt_num(recvs, &mut result, rlen + hdr2.len());
        result[rlen] = b' '; rlen += 1;
        let hdr3 = b"ctx=";
        for (j, &c) in hdr3.iter().enumerate() { result[rlen + j] = c; }
        fmt_num(ctx, &mut result, rlen + hdr3.len());
        return Some(result);
    }
    if path_match(path, b"/proc/version\0") {
        let ver = b"TrainOS V20.0 -- Microkernel OS";
        for (j, &c) in ver.iter().enumerate() { result[j] = c; }
        return Some(result);
    }
    if path_match(path, b"/proc/proc\0") {
        let mut buf = [0u8; 64];
        let count = tros::proclist(&mut buf);
        let rlen = (count * 6).min(SLOT_SIZE);
        for j in 0..rlen { result[j] = buf[j]; }
        return Some(result);
    }
    if path_match(path, b"/proc/self\0") {
        fmt_num(tros::getpid(), &mut result, 0);
        return Some(result);
    }
    None
}

fn path_match(a: &[u8], b: &[u8]) -> bool {
    let len = a.len().min(b.len());
    for i in 0..len { if a[i] != b[i] { return false; } }
    true
}

fn fmt_num(mut n: usize, buf: &mut [u8], off: usize) -> usize {
    let start = off;
    if n == 0 { buf[off] = b'0'; return off + 1; }
    let mut tmp = [0u8; 20];
    let mut ti = 20;
    loop {
        ti -= 1; tmp[ti] = b'0' + (n - (n / 10) * 10) as u8;
        n /= 10; if n == 0 { break; }
    }
    let mut pos = start;
    for j in ti..20 { buf[pos] = tmp[j]; pos += 1; }
    pos
}

fn starts_with(s: &[u8], prefix: &[u8]) -> bool {
    if s.len() < prefix.len() { return false; }
    for i in 0..prefix.len() { if s[i] != prefix[i] { return false; } }
    true
}

fn contains_slash(s: &[u8]) -> bool {
    for &c in s.iter() { if c == b'/' { return true; } }
    false
}

// ── Main ─────────────────────────────────────────────────────────────────────

#[no_mangle]
extern "C" fn _start() -> ! {
    let my_ep = 2;
    tros::print("VFS: ep=2 starting\r\n");

    unsafe {
        // Try to load from disk first
        if !load_from_disk() {
            tros::print("VFS: no disk data, initializing fresh\r\n");
        }
        init_vfs();
    }

    let mut buf = [0u8; 64];
    let mut op_count: usize = 0;

    loop {
        let (sender_pid, opcode) = tros::recv(my_ep, &mut buf);
        if sender_pid == usize::MAX { continue; }

        let reply_ep = buf[0] as usize | ((buf[1] as usize) << 8);
        let path_len = (buf[2] as usize).min(MAX_PATH);
        let path = &buf[3..3 + path_len];

        match opcode {
            2 => { // READ
                let data = unsafe {
                    if path_len >= 5 {
                        if let Some(proc_data) = proc_read(path) { Some(proc_data) }
                        else { find_slot(path).map(|idx| {
                            let mut d = [0u8; SLOT_SIZE];
                            let len = SLOTS[idx].data_len.min(SLOT_SIZE);
                            for j in 0..len { d[j] = SLOTS[idx].data[j]; }
                            d
                        })}
                    } else { None }
                };
                match data {
                    Some(d) => { tros::send(reply_ep, 0, &d); },
                    None => { tros::send(reply_ep, 0, b"ENOENT"); },
                }
            }
            3 => { // WRITE
                let data_off = 3 + path_len;
                let data_len = if data_off < 64 { buf[data_off] as usize } else { 0 };
                let data_start = data_off + 1;
                let dlen = data_len.min(SLOT_SIZE);
                let write_data = if data_start + dlen <= 64 { &buf[data_start..data_start + dlen] } else { &[] };

                unsafe {
                    let idx = if let Some(existing) = find_slot(path) {
                        existing
                    } else {
                        make_slot(path, false)
                    };
                    let slot = &mut SLOTS[idx];
                    slot.data_len = dlen;
                    for j in 0..dlen { slot.data[j] = write_data[j]; }
                    slot.dirty = true;
                }
                tros::send(reply_ep, 0, b"OK");
            }
            4 => { // APPEND
                let data_off = 3 + path_len;
                let data_len = if data_off < 64 { buf[data_off] as usize } else { 0 };
                let data_start = data_off + 1;
                let dlen = data_len.min(SLOT_SIZE);
                let append_data = if data_start + dlen <= 64 { &buf[data_start..data_start + dlen] } else { &[] };

                unsafe {
                    let idx = if let Some(existing) = find_slot(path) {
                        existing
                    } else {
                        make_slot(path, false)
                    };
                    let cur_len = SLOTS[idx].data_len;
                    let new_len = (cur_len + dlen).min(SLOT_SIZE);
                    for j in 0..(new_len - cur_len) {
                        SLOTS[idx].data[cur_len + j] = append_data[j];
                    }
                    SLOTS[idx].data_len = new_len;
                    SLOTS[idx].dirty = true;
                }
                tros::send(reply_ep, 0, b"OK");
            }
            5 => { // DELETE
                unsafe {
                    if let Some(idx) = find_slot(path) { SLOTS[idx].used = false; }
                }
                tros::send(reply_ep, 0, b"OK");
            }
            6 => { // LIST
                let mut listing = [0u8; 64];
                let mut pos: usize = 0;
                let dir_path = path;
                unsafe {
                    for i in 0..MAX_FILES {
                        if !SLOTS[i].used { continue; }
                        let ref p = SLOTS[i].path;
                        let plen = SLOTS[i].path_len;
                        if plen > dir_path.len() && starts_with(&p[..plen], dir_path) {
                            let rest = &p[dir_path.len()..plen];
                            if !contains_slash(rest) && pos + rest.len() + 1 <= 64 {
                                for j in 0..rest.len() { listing[pos + j] = rest[j]; }
                                pos += rest.len();
                                listing[pos] = b'\n'; pos += 1;
                            }
                        }
                    }
                }
                tros::send(reply_ep, 0, &listing[..pos]);
            }
            7 => { // STAT
                unsafe {
                    if let Some(idx) = find_slot(path) {
                        let resp = [SLOTS[idx].data_len as u8, if SLOTS[idx].is_dir { 1u8 } else { 0u8 }];
                        tros::send(reply_ep, 0, &resp);
                    } else {
                        tros::send(reply_ep, 0, b"ENOENT");
                    }
                }
            }
            _ => {}
        }

        // Periodically save to disk (every 16 operations)
        op_count += 1;
        if op_count & 0xF == 0 {
            unsafe { save_to_disk(); }
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    tros::print("VFS: PANIC\r\n");
    loop { unsafe { core::arch::asm!("wfi"); } }
}
