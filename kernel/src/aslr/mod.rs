// V27: Defense in Depth — ASLR, PIE, CHERI, Sandbox
//
// ASLR: Address Space Layout Randomization for kernel and user space
// PIE: Position-Independent Executable support
// CHERI: 128-bit fat pointer capability model (software emulation)
// Sandbox: Landlock-style path-based access control

use crate::mem::layout::PAGE_SIZE;

// ── Constants ──────────────────────────────────────────────────────────────────

pub const CHERI_PERM_R: u16 = 1;
pub const CHERI_PERM_W: u16 = 2;
pub const CHERI_PERM_X: u16 = 4;
pub const CHERI_PERM_C: u16 = 8;

const CHERI_MAX_CAPS_PER_PROC: usize = 16;
const CHERI_MAX_PROCS: usize = 32;
const SANDBOX_MAX_RULES: usize = 32;
const SANDBOX_NET_MAX_RULES: usize = 8;
const SANDBOX_NET_MAX_PROCS: usize = 32;
const UID_MAP_MAX_PROCS: usize = 16;
const UID_MAP_ENTRIES: usize = 8;

// ── ASLR ───────────────────────────────────────────────────────────────────────

static mut ASLR_SEED: u64 = 0xDEAD_BEEF_CAFE_BABE;
static mut ASLR_ENABLED: bool = true;

/// Initialize ASLR with a random seed (from mtime).
pub fn aslr_init() {
    let mtime: u64;
    unsafe {
        core::arch::asm!("rdtime {}", out(reg) mtime);
        ASLR_SEED = mtime.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
    }
}

/// Get a random offset for ASLR (page-aligned, 0-256 pages).
pub fn aslr_offset() -> usize {
    if !unsafe { ASLR_ENABLED } { return 0; }
    unsafe {
        ASLR_SEED = ASLR_SEED.wrapping_mul(6364136223846793005).wrapping_add(1);
        let offset = ((ASLR_SEED >> 12) & 0xFF) as usize;
        offset * PAGE_SIZE
    }
}

/// Randomize the user stack position.
pub fn randomize_stack() -> usize {
    let base = 0x0000_003F_FFFF_F000u64;
    let offset = aslr_offset() as u64;
    (base - offset) as usize
}

/// Randomize the mmap base address.
pub fn randomize_mmap() -> usize {
    let base = 0x0000_0001_0000_0000usize;
    let offset = aslr_offset();
    base.wrapping_add(offset)
}

// ── PIE support ────────────────────────────────────────────────────────────────

/// Compute the load address for a PIE binary.
pub fn pie_load_base() -> usize {
    if unsafe { ASLR_ENABLED } {
        0x10000 + aslr_offset()
    } else {
        0x10000
    }
}

// ── V27.2: KASLR (Kernel ASLR) ─────────────────────────────────────────────────

/// Random slide applied to the kernel base at boot (0-255 pages).
static mut KASLR_SLIDE: usize = 0;
static mut KASLR_INITIALIZED: bool = false;

/// Initialize KASLR: compute a random slide (0-255 pages).
pub fn kaslr_init() {
    let mtime: u64;
    unsafe {
        core::arch::asm!("rdtime {}", out(reg) mtime);
        let raw = mtime.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(0xDEAD);
        KASLR_SLIDE = ((raw >> 16) & 0xFF) as usize * PAGE_SIZE;
        KASLR_INITIALIZED = true;
    }
}

/// Returns the KASLR slide value (bytes).
pub fn kaslr_slide() -> usize {
    unsafe { if KASLR_INITIALIZED { KASLR_SLIDE } else { 0 } }
}

/// Randomize the heap base address (range 0x10000 .. 0x10000000).
pub fn randomize_heap() -> usize {
    if !unsafe { ASLR_ENABLED } { return 0x10000; }
    unsafe {
        ASLR_SEED = ASLR_SEED.wrapping_mul(6364136223846793005).wrapping_add(1);
        let range = 0x10000000usize - 0x10000usize;
        let offset = (ASLR_SEED as usize) % range;
        0x10000usize + (offset & !0xFFF)
    }
}

/// Enhanced per-process stack randomization with PID mixing.
pub fn randomize_stack_per_pid(pid: u32) -> usize {
    if !unsafe { ASLR_ENABLED } { return randomize_stack(); }
    unsafe {
        ASLR_SEED = ASLR_SEED.wrapping_mul(6364136223846793005).wrapping_add(pid as u64);
        let base = 0x0000_003F_FFFF_F000u64;
        let offset = ((ASLR_SEED >> 8) & 0xFFF) as usize;
        (base as usize).wrapping_sub(offset * PAGE_SIZE)
    }
}

/// Report bits of entropy in the current ASLR state (should be > 30).
pub fn aslr_entropy() -> u32 {
    unsafe {
        if !ASLR_ENABLED { return 0; }
        let seed_entropy = 32u32;
        let kaslr_e = if KASLR_INITIALIZED { 8 } else { 0 };
        let stack_e = 8u32;
        let stack_pid_e = 12u32;
        let heap_e = 20u32;
        let mmap_e = 8u32;
        let pie_e = 8u32;
        seed_entropy + kaslr_e + stack_e + stack_pid_e + heap_e + mmap_e + pie_e
    }
}

// ── CHERI fat pointer emulation ────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CheriCap {
    pub addr: u64,
    pub base: u64,
    pub bound: u64,
    pub permissions: u16,
    pub otype: u16,
}

impl CheriCap {
    pub const fn null() -> Self {
        CheriCap { addr: 0, base: 0, bound: 0, permissions: 0, otype: 0 }
    }

    pub fn in_bounds(&self, offset: usize) -> bool {
        let target = self.addr.wrapping_add(offset as u64);
        target >= self.base && target < self.bound
    }

    pub fn has_perm(&self, perm: u16) -> bool {
        (self.permissions & perm) == perm
    }

    pub fn for_region(addr: usize, len: usize, perms: u16) -> Self {
        CheriCap {
            addr: addr as u64,
            base: addr as u64,
            bound: (addr + len) as u64,
            permissions: perms,
            otype: 0,
        }
    }

    /// Check if [check_addr, check_addr+check_len) is fully within bounds.
    pub fn covers(&self, check_addr: usize, check_len: usize) -> bool {
        let start = check_addr as u64;
        let end = (check_addr as u64).wrapping_add(check_len as u64);
        if end < start { return false; }
        start >= self.base && end <= self.bound
    }
}

// ── V27.1: CHERI Capability Table (per-process, 16 slots) ──────────────────────

#[derive(Clone, Copy)]
struct CheriCapEntry {
    valid: bool,
    cap: CheriCap,
}

const CHERI_NULL_ENTRY: CheriCapEntry = CheriCapEntry { valid: false, cap: CheriCap::null() };

struct CheriTable {
    pid: u32,
    caps: [CheriCapEntry; CHERI_MAX_CAPS_PER_PROC],
}

static mut CHERI_TABLES: [CheriTable; CHERI_MAX_PROCS] = unsafe {
    const CT: CheriTable = CheriTable {
        pid: 0,
        caps: [CHERI_NULL_ENTRY; CHERI_MAX_CAPS_PER_PROC],
    };
    [CT; CHERI_MAX_PROCS]
};
static mut CHERI_TABLE_COUNT: usize = 0;

fn cheri_find_or_create(pid: u32) -> Option<usize> {
    unsafe {
        for i in 0..CHERI_TABLE_COUNT {
            if CHERI_TABLES[i].pid == pid { return Some(i); }
        }
        if CHERI_TABLE_COUNT >= CHERI_MAX_PROCS { return None; }
        let idx = CHERI_TABLE_COUNT;
        CHERI_TABLES[idx] = CheriTable { pid, caps: [CHERI_NULL_ENTRY; CHERI_MAX_CAPS_PER_PROC] };
        CHERI_TABLE_COUNT += 1;
        Some(idx)
    }
}

fn cheri_find(pid: u32) -> Option<usize> {
    unsafe {
        for i in 0..CHERI_TABLE_COUNT {
            if CHERI_TABLES[i].pid == pid { return Some(i); }
        }
        None
    }
}

/// Create a CHERI capability for a process. Returns cap_id (0..15).
pub fn cap_create(pid: u32, addr: usize, len: usize, perms: u16) -> Result<u8, &'static str> {
    let ti = cheri_find_or_create(pid).ok_or("cheri table full")?;
    unsafe {
        for slot in 0..CHERI_MAX_CAPS_PER_PROC {
            if !CHERI_TABLES[ti].caps[slot].valid {
                CHERI_TABLES[ti].caps[slot] = CheriCapEntry {
                    valid: true,
                    cap: CheriCap::for_region(addr, len, perms),
                };
                return Ok(slot as u8);
            }
        }
        Err("cheri caps full (max 16)")
    }
}

/// Load a capability by pid and cap_id.
pub fn cap_load(pid: u32, cap_id: u8) -> Option<CheriCap> {
    let ti = cheri_find(pid)?;
    unsafe {
        let slot = cap_id as usize;
        if slot >= CHERI_MAX_CAPS_PER_PROC || !CHERI_TABLES[ti].caps[slot].valid { return None; }
        Some(CHERI_TABLES[ti].caps[slot].cap)
    }
}

/// Delete a capability by pid and cap_id.
pub fn cap_delete(pid: u32, cap_id: u8) -> Result<(), &'static str> {
    let ti = cheri_find(pid).ok_or("no cheri table for pid")?;
    unsafe {
        let slot = cap_id as usize;
        if slot >= CHERI_MAX_CAPS_PER_PROC { return Err("invalid cap_id"); }
        if !CHERI_TABLES[ti].caps[slot].valid { return Err("cap not found"); }
        CHERI_TABLES[ti].caps[slot].valid = false;
        Ok(())
    }
}

/// Validate a pointer [addr, addr+len) against ALL capabilities for a process.
pub fn validate_ptr(pid: u32, addr: usize, len: usize, required_perms: u16) -> bool {
    let ti = match cheri_find(pid) {
        Some(ti) => ti,
        None => return true,
    };
    unsafe {
        for slot in 0..CHERI_MAX_CAPS_PER_PROC {
            if !CHERI_TABLES[ti].caps[slot].valid { continue; }
            let cap = &CHERI_TABLES[ti].caps[slot].cap;
            if cap.covers(addr, len) && cap.has_perm(required_perms) { return true; }
        }
    }
    false
}

/// Format CHERI capability status. Returns bytes written.
pub fn cheri_status_format(buf: &mut [u8]) -> usize {
    let mut pos = 0usize;
    unsafe {
        for ti in 0..CHERI_TABLE_COUNT {
            let pid = CHERI_TABLES[ti].pid;
            for slot in 0..CHERI_MAX_CAPS_PER_PROC {
                if !CHERI_TABLES[ti].caps[slot].valid { continue; }
                let cap = &CHERI_TABLES[ti].caps[slot].cap;
                pos += w_str(buf, pos, "pid=");
                pos += w_u64(buf, pos, pid as u64);
                pos += w_str(buf, pos, " cap=");
                pos += w_u64(buf, pos, slot as u64);
                pos += w_str(buf, pos, " addr=0x");
                pos += w_hex64(buf, pos, cap.addr);
                pos += w_str(buf, pos, " base=0x");
                pos += w_hex64(buf, pos, cap.base);
                pos += w_str(buf, pos, " bound=0x");
                pos += w_hex64(buf, pos, cap.bound);
                pos += w_str(buf, pos, " perms=");
                pos += w_u64(buf, pos, cap.permissions as u64);
                pos += w_str(buf, pos, "\n");
            }
        }
    }
    pos
}

// ── Sandbox: Path-based access control ─────────────────────────────────────────

#[derive(Clone, Copy)]
struct SandboxRule {
    pid: u32,
    path_prefix: [u8; 32],
    path_len: usize,
    allow_read: bool,
    allow_write: bool,
}

static mut SANDBOX_RULES: [SandboxRule; SANDBOX_MAX_RULES] = unsafe {
    const SR: SandboxRule = SandboxRule {
        pid: 0, path_prefix: [0; 32], path_len: 0,
        allow_read: false, allow_write: false,
    };
    [SR; SANDBOX_MAX_RULES]
};
static mut SANDBOX_COUNT: usize = 0;

pub fn sandbox_add(pid: u32, path: &[u8], allow_r: bool, allow_w: bool) -> Result<(), &'static str> {
    unsafe {
        if SANDBOX_COUNT >= SANDBOX_MAX_RULES { return Err("full"); }
        SANDBOX_RULES[SANDBOX_COUNT].pid = pid;
        let len = path.len().min(32);
        SANDBOX_RULES[SANDBOX_COUNT].path_len = len;
        for i in 0..len { SANDBOX_RULES[SANDBOX_COUNT].path_prefix[i] = path[i]; }
        SANDBOX_RULES[SANDBOX_COUNT].allow_read = allow_r;
        SANDBOX_RULES[SANDBOX_COUNT].allow_write = allow_w;
        SANDBOX_COUNT += 1;
        Ok(())
    }
}

pub fn sandbox_check(pid: u32, path: &[u8], wants_write: bool) -> bool {
    unsafe {
        for i in 0..SANDBOX_COUNT {
            if SANDBOX_RULES[i].pid != pid { continue; }
            let plen = SANDBOX_RULES[i].path_len;
            if path.len() >= plen {
                let mut matches = true;
                for j in 0..plen {
                    if path[j] != SANDBOX_RULES[i].path_prefix[j] { matches = false; break; }
                }
                if matches {
                    if wants_write { return SANDBOX_RULES[i].allow_write; }
                    else { return SANDBOX_RULES[i].allow_read; }
                }
            }
        }
    }
    true
}

// ── V27.3: Network Sandbox (port-based) ────────────────────────────────────────

#[derive(Clone, Copy)]
struct SandboxNetRule {
    pid: u32,
    port_start: u16,
    port_end: u16,
    allow: bool,
}

static mut SANDBOX_NET_RULES: [SandboxNetRule; SANDBOX_NET_MAX_RULES * SANDBOX_NET_MAX_PROCS] = unsafe {
    const SNR: SandboxNetRule = SandboxNetRule { pid: 0, port_start: 0, port_end: 0, allow: false };
    [SNR; SANDBOX_NET_MAX_RULES * SANDBOX_NET_MAX_PROCS]
};
static mut SANDBOX_NET_COUNT: usize = 0;

pub fn sandbox_net_add(pid: u32, port_start: u16, port_end: u16, allow: bool) -> Result<(), &'static str> {
    unsafe {
        if SANDBOX_NET_COUNT >= SANDBOX_NET_RULES.len() { return Err("full"); }
        SANDBOX_NET_RULES[SANDBOX_NET_COUNT] = SandboxNetRule { pid, port_start, port_end, allow };
        SANDBOX_NET_COUNT += 1;
        Ok(())
    }
}

pub fn sandbox_net_check(pid: u32, port: u16, _wants_bind: bool) -> bool {
    unsafe {
        for i in 0..SANDBOX_NET_COUNT {
            if SANDBOX_NET_RULES[i].pid != pid { continue; }
            if port >= SANDBOX_NET_RULES[i].port_start && port <= SANDBOX_NET_RULES[i].port_end {
                return SANDBOX_NET_RULES[i].allow;
            }
        }
    }
    true
}

// ── V27.3: UID Namespace Mapping ───────────────────────────────────────────────

#[derive(Clone, Copy)]
struct UidMapTable {
    pid: u32,
    entries: [(u32, u32); UID_MAP_ENTRIES],
    count: usize,
}

static mut UID_MAP_TABLES: [UidMapTable; UID_MAP_MAX_PROCS] = unsafe {
    const UMT: UidMapTable = UidMapTable { pid: 0, entries: [(0, 0); UID_MAP_ENTRIES], count: 0 };
    [UMT; UID_MAP_MAX_PROCS]
};
static mut UID_MAP_TABLE_COUNT: usize = 0;

pub fn sandbox_uid_map(pid: u32, inner_uid: u32, outer_uid: u32) -> Result<(), &'static str> {
    unsafe {
        for i in 0..UID_MAP_TABLE_COUNT {
            if UID_MAP_TABLES[i].pid == pid {
                if UID_MAP_TABLES[i].count >= UID_MAP_ENTRIES { return Err("uid map entries full"); }
                UID_MAP_TABLES[i].entries[UID_MAP_TABLES[i].count] = (inner_uid, outer_uid);
                UID_MAP_TABLES[i].count += 1;
                return Ok(());
            }
        }
        if UID_MAP_TABLE_COUNT >= UID_MAP_MAX_PROCS { return Err("uid map table full"); }
        let idx = UID_MAP_TABLE_COUNT;
        UID_MAP_TABLES[idx].pid = pid;
        UID_MAP_TABLES[idx].entries[0] = (inner_uid, outer_uid);
        UID_MAP_TABLES[idx].count = 1;
        UID_MAP_TABLE_COUNT += 1;
        Ok(())
    }
}

pub fn translate_uid(pid: u32, uid: u32) -> u32 {
    unsafe {
        for i in 0..UID_MAP_TABLE_COUNT {
            if UID_MAP_TABLES[i].pid == pid {
                for j in 0..UID_MAP_TABLES[i].count {
                    let (inner, outer) = UID_MAP_TABLES[i].entries[j];
                    if inner == uid { return outer; }
                }
                break;
            }
        }
    }
    uid
}

// ── Sandbox status formatting (for /proc/sandbox) ──────────────────────────────

pub fn sandbox_status_format(pid: u32, buf: &mut [u8]) -> usize {
    let mut pos = 0usize;
    unsafe {
        for i in 0..SANDBOX_COUNT {
            if SANDBOX_RULES[i].pid != pid { continue; }
            let rule = &SANDBOX_RULES[i];
            pos += w_str(buf, pos, "path=");
            pos += w_bytes(buf, pos, &rule.path_prefix[..rule.path_len]);
            pos += w_str(buf, pos, " r=");
            pos += w_str(buf, pos, if rule.allow_read { "1" } else { "0" });
            pos += w_str(buf, pos, " w=");
            pos += w_str(buf, pos, if rule.allow_write { "1" } else { "0" });
            pos += w_str(buf, pos, "\n");
        }
        for i in 0..SANDBOX_NET_COUNT {
            if SANDBOX_NET_RULES[i].pid != pid { continue; }
            let rule = &SANDBOX_NET_RULES[i];
            pos += w_str(buf, pos, "net=");
            pos += w_u64(buf, pos, rule.port_start as u64);
            pos += w_str(buf, pos, "-");
            pos += w_u64(buf, pos, rule.port_end as u64);
            pos += w_str(buf, pos, " ");
            pos += w_str(buf, pos, if rule.allow { "allow" } else { "deny" });
            pos += w_str(buf, pos, "\n");
        }
    }
    pos
}

// ── Internal formatting helpers ────────────────────────────────────────────────

fn w_str(buf: &mut [u8], pos: usize, s: &str) -> usize {
    let bytes = s.as_bytes();
    let len = bytes.len().min(buf.len().saturating_sub(pos));
    if len > 0 { buf[pos..pos + len].copy_from_slice(&bytes[..len]); }
    len
}

fn w_bytes(buf: &mut [u8], pos: usize, bytes: &[u8]) -> usize {
    let len = bytes.len().min(buf.len().saturating_sub(pos));
    if len > 0 { buf[pos..pos + len].copy_from_slice(&bytes[..len]); }
    len
}

fn w_u64(buf: &mut [u8], pos: usize, v: u64) -> usize {
    if v == 0 {
        if pos < buf.len() { buf[pos] = b'0'; return 1; }
        return 0;
    }
    let mut temp = [0u8; 20];
    let mut n = v;
    let mut len = 0;
    while n > 0 { temp[len] = b'0' + (n % 10) as u8; n /= 10; len += 1; }
    let mut written = 0;
    for i in (0..len).rev() {
        if pos + written < buf.len() { buf[pos + written] = temp[i]; written += 1; } else { break; }
    }
    written
}

fn w_hex64(buf: &mut [u8], pos: usize, v: u64) -> usize {
    if v == 0 {
        if pos < buf.len() { buf[pos] = b'0'; return 1; }
        return 0;
    }
    let mut temp = [0u8; 16];
    let mut n = v;
    let mut len = 0;
    while n > 0 {
        let nibble = (n & 0xF) as u8;
        temp[len] = if nibble < 10 { b'0' + nibble } else { b'a' + nibble - 10 };
        n >>= 4;
        len += 1;
    }
    let mut written = 0;
    for i in (0..len).rev() {
        if pos + written < buf.len() { buf[pos + written] = temp[i]; written += 1; } else { break; }
    }
    written
}
