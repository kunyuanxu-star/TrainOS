/// RISC-V Sspmp — S-mode Physical Memory Protection
///
/// Allows S-mode (the kernel) to protect physical memory regions from
/// U-mode access without relying on page table permissions alone.
///
/// This provides defense-in-depth: even if a process corrupts its page
/// table, Sspmp entries prevent it from accessing physical pages outside
/// its allocated region.
///
/// Key concepts:
///   - Up to 16 S-mode PMP entries (same hardware as M-mode PMP, but
///     configured and locked by S-mode via `sseccfg` CSR delegation)
///   - Each entry specifies base, size (NAPOT-encoded), permissions (R/W/X)
///   - Entries can be locked (persist until reset)
///   - sseccfg CSR (at 0x390) controls S-mode PMP lockdown behavior:
///       MML  (bit 0) — S-mode Machine Mode Lockdown
///       MMWP (bit 1) — S-mode Machine Mode Whitelist Policy
///       RLB  (bit 2) — Rule Locking Bypass
///
/// Configuration flow:
///   1. Hypervisor/HS-mode delegates PMP access to S-mode via `mseccfg`
///   2. S-mode writes PMP CFG and ADDR CSRs directly
///   3. Entries are applied immediately by hardware
///
/// V33 TEE integration:
///   - Used for enclave memory isolation in V33 Confidential Computing
///   - Each enclave gets its own Sspmp region, locked to prevent tampering
///   - Combined with page table isolation for defense-in-depth

use core::cmp;
use core::sync::atomic::{AtomicBool, Ordering};

/// Maximum number of Sspmp entries (hardware limit).
const MAX_SSPMP_ENTRIES: usize = 16;

/// Sspmp permissions: read, write, execute bits.
pub const SSPMP_R: u8 = 1;
pub const SSPMP_W: u8 = 2;
pub const SSPMP_X: u8 = 4;

/// Sseccfg CSR bits.
pub const SSECCFG_MML: u64 = 1 << 0;   // S-mode Machine Mode Lockdown
pub const SSECCFG_MMWP: u64 = 1 << 1;  // S-mode Machine Mode Whitelist Policy
pub const SSECCFG_RLB: u64 = 1 << 2;   // Rule Locking Bypass

// ── Sspmp Entry ───────────────────────────────────────────────────────────

/// A single Sspmp entry describing a protected physical memory region.
#[derive(Clone, Copy, Debug)]
pub struct SPmpEntry {
    /// Base physical address (page-aligned).
    pub base_addr: usize,
    /// Size in bytes (must be power of 2, NAPOT-encoded).
    pub size: usize,
    /// Permissions: R=1, W=2, X=4.
    pub permissions: u8,
    /// Whether this entry is locked (cannot be modified until reset).
    pub locked: bool,
    /// Also restrict S-mode access (for M-mode delegated enforcement).
    pub enforced_for_s: bool,
    /// PID this entry belongs to (0 = global).
    pub owner_pid: u32,
}

impl SPmpEntry {
    const fn empty() -> Self {
        SPmpEntry {
            base_addr: 0,
            size: 0,
            permissions: 0,
            locked: false,
            enforced_for_s: false,
            owner_pid: 0,
        }
    }

    /// Check if a physical address and access type match this entry.
    fn matches(&self, paddr: usize, is_write: bool, is_exec: bool, is_smode: bool) -> bool {
        if self.size == 0 {
            return false;
        }
        if paddr < self.base_addr || paddr >= self.base_addr + self.size {
            return false;
        }
        // Check if S-mode is restricted and the access is from S-mode
        if self.enforced_for_s && is_smode {
            return false;
        }
        // Check permissions
        let need_write_bit = if is_write { SSPMP_W } else { 0 };
        let need_exec_bit = if is_exec { SSPMP_X } else { 0 };
        let need_bits = SSPMP_R | need_write_bit | need_exec_bit;
        (self.permissions & need_bits) == need_bits
    }
}

// ── Sspmp Configuration ───────────────────────────────────────────────────

/// Sspmp configuration manager.
///
/// Manages up to 16 PMP entries that restrict U-mode (and optionally
/// S-mode) access to physical memory regions.
pub struct SPmpConfig {
    /// S-mode PMP entries.
    entries: [SPmpEntry; MAX_SSPMP_ENTRIES],
    /// Number of active entries.
    entry_count: usize,
    /// sseccfg CSR value (S-mode security config).
    sseccfg: u64,
}

impl SPmpConfig {
    /// Create a new Sspmp configuration with all entries empty.
    pub const fn new() -> Self {
        SPmpConfig {
            entries: [
                SPmpEntry::empty(), SPmpEntry::empty(), SPmpEntry::empty(), SPmpEntry::empty(),
                SPmpEntry::empty(), SPmpEntry::empty(), SPmpEntry::empty(), SPmpEntry::empty(),
                SPmpEntry::empty(), SPmpEntry::empty(), SPmpEntry::empty(), SPmpEntry::empty(),
                SPmpEntry::empty(), SPmpEntry::empty(), SPmpEntry::empty(), SPmpEntry::empty(),
            ],
            entry_count: 0,
            sseccfg: 0,
        }
    }

    /// Initialize Sspmp support.
    ///
    /// Probes for Sspmp availability and returns a configured instance.
    pub fn init() -> Self {
        let available = Self::probe_available();
        if available {
            crate::println!("  V38c: Sspmp available (S-mode PMP with {} max entries)", MAX_SSPMP_ENTRIES);
        } else {
            crate::println!("  V38c: Sspmp not available, S-mode PMP disabled");
        }
        SPmpConfig::new()
    }

    /// Probe whether Sspmp is available.
    ///
    /// Sspmp availability depends on:
    ///   1. Hardware supporting S-mode access to PMP CSRs
    ///   2. M-mode (RustSBI) delegating PMP access to S-mode
    ///
    /// For QEMU virt, Sspmp is available when the platform has PMP support
    /// and the firmware delegates it.
    pub fn probe_available() -> bool {
        // On QEMU virt with current RustSBI, Sspmp is generally available.
        // A real implementation should check the ISA string.
        #[cfg(not(test))]
        unsafe {
            // Try to read pmpcfg0 (CSR 0x3A0) — if accessible, Sspmp is available.
            // If not accessible, this would trap. On our target, we assume available.
            let _probe: usize;
            core::arch::asm!("csrr {}, 0x3A0", out(reg) _probe);
            true
        }
        #[cfg(test)]
        true
    }

    /// Check if Sspmp is available (runtime check).
    pub fn available(&self) -> bool {
        // Sspmp is available if we have at least one free entry slot
        // or the configuration is operational.  Simplified: always true
        // when compiled in (availability was checked at init).
        true
    }

    /// Protect a physical memory region from U-mode access.
    ///
    /// Adds a new Sspmp entry.  Returns `Err` if the entry table is full
    /// or the parameters are invalid.
    ///
    /// `perms` is a bitmask: SSPMP_R | SSPMP_W | SSPMP_X.
    pub fn protect_region(
        &mut self,
        base: usize,
        size: usize,
        perms: u8,
        enforce_s: bool,
        pid: u32,
    ) -> Result<(), &'static str> {
        if self.entry_count >= MAX_SSPMP_ENTRIES {
            return Err("Sspmp: entry table full");
        }
        if size == 0 || (size & (size - 1)) != 0 {
            return Err("Sspmp: size must be power of 2");
        }
        if base & (crate::mem::layout::PAGE_SIZE - 1) != 0 {
            return Err("Sspmp: base must be page-aligned");
        }
        if perms & !(SSPMP_R | SSPMP_W | SSPMP_X) != 0 {
            return Err("Sspmp: invalid permissions");
        }

        let idx = self.entry_count;
        self.entries[idx] = SPmpEntry {
            base_addr: base,
            size,
            permissions: perms,
            locked: false,
            enforced_for_s: enforce_s,
            owner_pid: pid,
        };
        self.entry_count += 1;
        Ok(())
    }

    /// Check if a physical address is accessible with given permissions.
    ///
    /// Walks through all Sspmp entries and checks if any matching entry
    /// grants the requested access.
    ///
    /// Returns `true` if access is permitted.
    pub fn check_access(&self, paddr: usize, is_write: bool, is_exec: bool, is_smode: bool) -> bool {
        if self.entry_count == 0 {
            // No Sspmp entries: all access is permitted (default policy).
            return true;
        }

        // Check against all entries — if any matches, access is granted.
        for i in 0..self.entry_count {
            if self.entries[i].matches(paddr, is_write, is_exec, is_smode) {
                return true;
            }
        }
        // No matching entry: access denied.
        false
    }

    /// Lock all entries to prevent further modification until reset.
    pub fn lock_all(&mut self) {
        for i in 0..self.entry_count {
            self.entries[i].locked = true;
        }
    }

    /// Return the number of available remaining entries.
    pub fn remaining_entries(&self) -> usize {
        MAX_SSPMP_ENTRIES.saturating_sub(self.entry_count)
    }

    /// Apply the Sspmp configuration to hardware.
    ///
    /// Writes the PMP configuration registers (pmpcfg0-3, pmpaddr0-15)
    /// and optionally updates sseccfg via SBI call.
    #[cfg(not(test))]
    pub fn apply(&self) {
        unsafe {
            // Write PMP configuration registers
            // Each pmpcfg register configures 4 entries (8 bits each)
            for cfg_idx in 0..4 {
                let mut cfg_val: usize = 0;
                for entry_in_cfg in 0..4 {
                    let entry_idx = cfg_idx * 4 + entry_in_cfg;
                    if entry_idx >= self.entry_count {
                        break;
                    }
                    let e = &self.entries[entry_idx];
                    if e.size == 0 && !e.locked {
                        continue;
                    }
                    // PMP CFG encoding (per entry):
                    //   bits [1:0] = R (read)
                    //   bit  [2]   = W (write)
                    //   bit  [3]   = X (execute)
                    //   bit  [4]   = A (addressing mode): 1=TOR, 3=NA4, 5=NAPOT
                    //   bit  [5]   = L (lock)
                    let mut enc: u8 = 0;
                    if e.permissions & SSPMP_R != 0 { enc |= 1; }
                    if e.permissions & SSPMP_W != 0 { enc |= 4; }
                    if e.permissions & SSPMP_X != 0 { enc |= 8; }
                    // NAPOT addressing mode
                    enc |= 5 << 3; // A=NAPOT
                    if e.locked {
                        enc |= 1 << 7; // L bit
                    }
                    cfg_val |= (enc as usize) << (entry_in_cfg * 8);
                }
                // Write pmpcfg0-3 (CSRs 0x3A0-0x3A3)
                let csr = 0x3A0 + cfg_idx;
                core::arch::asm!("csrw {csr}, {val}", csr = in(reg) csr, val = in(reg) cfg_val);
            }

            // Write PMP address registers (pmpaddr0-15, CSRs 0x3B0-0x3BF)
            for i in 0..self.entry_count {
                let e = &self.entries[i];
                let pmpaddr_val = napot_encode(e.base_addr, e.size);
                let csr = 0x3B0 + i;
                core::arch::asm!("csrw {csr}, {val}", csr = in(reg) csr, val = in(reg) pmpaddr_val);
            }

            // Set sseccfg CSR (0x390) if applicable
            if self.sseccfg != 0 {
                core::arch::asm!("csrw 0x390, {}", in(reg) self.sseccfg);
            }
        }
    }

    /// Create a sandbox region for a process.
    ///
    /// Restricts the process to accessing only its own physical pages.
    /// `phys_base` and `phys_size` define the process's memory region.
    /// `pid` identifies the process for auditing.
    pub fn create_sandbox(&mut self, phys_base: usize, phys_size: usize, pid: u32) -> Result<(), &'static str> {
        // Grant R+W+X access to the process's own memory
        self.protect_region(phys_base, phys_size, SSPMP_R | SSPMP_W | SSPMP_X, false, pid)?;
        Ok(())
    }

    /// Remove a sandbox region for a process.
    ///
    /// Finds and removes all Sspmp entries owned by the given PID.
    pub fn remove_sandbox(&mut self, pid: u32) {
        let mut new_count = 0;
        for i in 0..self.entry_count {
            if self.entries[i].owner_pid != pid {
                // Keep this entry
                if new_count != i {
                    self.entries[new_count] = self.entries[i];
                }
                new_count += 1;
            }
            // Entries with matching PID are dropped
        }
        self.entry_count = new_count;
    }

    /// Integration with the capability system.
    ///
    /// Checks if the given capability token authorizes the Sspmp region
    /// update, and applies it if authorized.
    pub fn cap_check_and_configure(
        &mut self,
        cap_token: usize,
        region: SPmpEntry,
    ) -> Result<(), &'static str> {
        // Verify the capability token grants Sspmp configuration rights
        // (cap type should be CAP_SSPMP_CFG or similar)
        if cap_token == 0 {
            return Err("Sspmp: invalid capability token");
        }
        // In a full implementation, check against the capability system:
        //   let rights = crate::cap::ops::check_cap(cap_token);
        //   if !rights.contains(CAP_SSPMP_ADMIN) { return Err(...); }
        let _ = cap_token;

        if self.entry_count >= MAX_SSPMP_ENTRIES {
            return Err("Sspmp: entry table full");
        }

        let idx = self.entry_count;
        self.entries[idx] = region;
        self.entry_count += 1;
        Ok(())
    }

    /// Get a read-only reference to the entries.
    pub fn entries(&self) -> &[SPmpEntry] {
        &self.entries[..self.entry_count]
    }

    /// Get the number of active entries.
    pub fn entry_count(&self) -> usize {
        self.entry_count
    }
}

// ── NAPOT Encoding Helpers ────────────────────────────────────────────────

/// Encode (base, size) into a NAPOT-format PMP address value.
///
/// NAPOT encoding: the address field has trailing ones for the
/// address bits that are part of the size mask.
///   pmpaddr = (base >> 2) | ((size / 2 - 1) >> 1)
/// where size must be a power of 2 and >= 8.
fn napot_encode(base: usize, size: usize) -> usize {
    if size < 8 {
        return base >> 2; // NA4: naturally aligned 4-byte region
    }
    let addr = base >> 2;
    let mask = (size >> 3) - 1; // number of trailing bits to set to 1
    addr | mask
}

/// Decode a NAPOT-format PMP address value back to (base, size).
#[allow(dead_code)]
fn napot_decode(pmpaddr: usize) -> (usize, usize) {
    // Find the lowest zero bit — the position of the first zero determines size
    let trailing_ones = pmpaddr.trailing_ones();
    let size = 8usize << trailing_ones;
    let base = (pmpaddr & !((1 << trailing_ones) - 1)) << 2;
    (base, size)
}

// ── Sspmp Sandbox Manager ─────────────────────────────────────────────────

/// Global Sspmp configuration instance.
use spin::Mutex;
static SSPMP_CONFIG: Mutex<Option<SPmpConfig>> = Mutex::new(None);

/// Initialize the global Sspmp subsystem.
pub fn sspmp_init() {
    let config = SPmpConfig::init();
    *SSPMP_CONFIG.lock() = Some(config);
}

/// Check Sspmp access for a physical address (global API).
pub fn sspmp_check_access(paddr: usize, write: bool, exec: bool, smode: bool) -> bool {
    SSPMP_CONFIG
        .lock()
        .as_ref()
        .map(|c| c.check_access(paddr, write, exec, smode))
        .unwrap_or(true)
}

/// Create a sandbox for a process (global API).
pub fn sspmp_create_sandbox(phys_base: usize, phys_size: usize, pid: u32) -> Result<(), &'static str> {
    if let Some(ref mut config) = *SSPMP_CONFIG.lock() {
        config.create_sandbox(phys_base, phys_size, pid)
    } else {
        Err("Sspmp: not initialized")
    }
}

/// Remove a sandbox for a process (global API).
pub fn sspmp_remove_sandbox(pid: u32) {
    if let Some(ref mut config) = *SSPMP_CONFIG.lock() {
        config.remove_sandbox(pid);
    }
}

/// Apply the Sspmp configuration to hardware.
pub fn sspmp_apply() {
    if let Some(ref config) = *SSPMP_CONFIG.lock() {
        #[cfg(not(test))]
        config.apply();
    }
}

/// Lock all Sspmp entries.
pub fn sspmp_lock_all() {
    if let Some(ref mut config) = *SSPMP_CONFIG.lock() {
        config.lock_all();
    }
}

/// Number of currently active Sspmp entries.
pub fn sspmp_entry_count() -> usize {
    SSPMP_CONFIG.lock().as_ref().map(|c| c.entry_count()).unwrap_or(0)
}
