// V27: Defense in Depth — ASLR, PIE, CHERI, Sandbox
//
// ASLR: Address Space Layout Randomization for kernel and user space
// PIE: Position-Independent Executable support
// CHERI: 128-bit fat pointer capability model (software emulation)
// Sandbox: Landlock-style path-based access control

use crate::mem::layout::PAGE_SIZE;

// ── ASLR ─────────────────────────────────────────────────────────────────────

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

// ── PIE support ──────────────────────────────────────────────────────────────

/// Compute the load address for a PIE binary.
pub fn pie_load_base() -> usize {
    if unsafe { ASLR_ENABLED } {
        0x10000 + aslr_offset()
    } else {
        0x10000
    }
}

// ── CHERI fat pointer emulation ──────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CheriCap {
    pub addr: u64,        // virtual address
    pub base: u64,        // lower bound
    pub bound: u64,       // upper bound (base + length)
    pub permissions: u16, // R=1, W=2, X=4, C=8 (capability)
    pub otype: u16,       // object type
}

impl CheriCap {
    pub const fn null() -> Self {
        CheriCap { addr: 0, base: 0, bound: 0, permissions: 0, otype: 0 }
    }

    /// Check if address + offset is within bounds.
    pub fn in_bounds(&self, offset: usize) -> bool {
        let target = self.addr.wrapping_add(offset as u64);
        target >= self.base && target < self.bound
    }

    /// Check if the capability has the required permissions.
    pub fn has_perm(&self, perm: u16) -> bool {
        (self.permissions & perm) == perm
    }

    /// Create a capability for a memory region.
    pub fn for_region(addr: usize, len: usize, perms: u16) -> Self {
        CheriCap {
            addr: addr as u64,
            base: addr as u64,
            bound: (addr + len) as u64,
            permissions: perms,
            otype: 0,
        }
    }
}

pub const CHERI_PERM_R: u16 = 1;
pub const CHERI_PERM_W: u16 = 2;
pub const CHERI_PERM_X: u16 = 4;

// ── Sandbox ──────────────────────────────────────────────────────────────────

const SANDBOX_MAX_RULES: usize = 32;

#[derive(Clone, Copy)]
struct SandboxRule {
    pid: u32,
    path_prefix: [u8; 32],
    path_len: usize,
    allow_read: bool,
    allow_write: bool,
}

static mut SANDBOX_RULES: [SandboxRule; SANDBOX_MAX_RULES] = [
    SandboxRule { pid: 0, path_prefix: [0; 32], path_len: 0, allow_read: false, allow_write: false }; SANDBOX_MAX_RULES
];
static mut SANDBOX_COUNT: usize = 0;

/// Add a sandbox path restriction for a process.
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

/// Check if a process is allowed to access a path.
pub fn sandbox_check(pid: u32, path: &[u8], wants_write: bool) -> bool {
    unsafe {
        for i in 0..SANDBOX_COUNT {
            if SANDBOX_RULES[i].pid != pid { continue; }
            // Check if the path starts with the sandbox prefix
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
    true // default allow if no rules match
}
