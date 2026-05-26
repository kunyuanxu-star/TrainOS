/// RISC-V Cache Management Operations (CMO)
///
/// Implements the Zicbom, Zicboz, and Zicbop extensions:
/// - Zicbom: Cache Block Management (clean, flush, invalidate)
/// - Zicboz: Cache Block Zero (zero a cache block without R/W)
/// - Zicbop: Cache Block Prefetch (future / placeholder)
///
/// All operations operate on the standard RISC-V cache block size (64 bytes).
///
/// Wire these into the block device layer for DMA coherency.

use core::sync::atomic::{AtomicBool, Ordering};

const CACHE_BLOCK_SIZE: usize = 64;

/// Whether CMO extensions have been detected at runtime.
static CMO_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Mark CMO as available (called during boot if Zicbom/Zicboz are present).
pub fn set_cmo_available() {
    CMO_AVAILABLE.store(true, Ordering::Relaxed);
}

/// Check whether CMO extensions (Zicbom / Zicboz) are available.
///
/// In real hardware this should inspect the ISA string (marchid / mimpid)
/// or device-tree.  On QEMU 8.0+ use `-cpu rv64,zicbom=true,zicboz=true`.
/// Until then, the feature is enabled via `set_cmo_available()`.
pub fn cmo_available() -> bool {
    CMO_AVAILABLE.load(Ordering::Relaxed)
}

// ── Single-block operations ───────────────────────────────────────────────

/// CBO.CLEAN — write back a cache block to main memory.
///
/// Ensures data in the cache block is written to the coherence point.
/// Used **before** a device DMA read so the device sees the latest data.
///
/// Encoding: funct3=001, opcode=0x0F (CBO)
#[inline]
pub unsafe fn cbo_clean(addr: usize) {
    if !cmo_available() {
        return;
    }
    #[cfg(not(test))]
    core::arch::asm!(".insn i 0x0F, 0x01, x0, 0({addr})", addr = in(reg) addr);
    #[cfg(test)]
    let _ = addr;
}

/// CBO.FLUSH — write back + invalidate a cache block.
///
/// Ensures data is written back and the cache block is marked invalid.
/// Used **before** a device DMA write so the cache doesn't hold stale data.
///
/// Encoding: funct3=010, opcode=0x0F (CBO)
#[inline]
pub unsafe fn cbo_flush(addr: usize) {
    if !cmo_available() {
        return;
    }
    #[cfg(not(test))]
    core::arch::asm!(".insn i 0x0F, 0x02, x0, 0({addr})", addr = in(reg) addr);
    #[cfg(test)]
    let _ = addr;
}

/// CBO.INVAL — invalidate a cache block (discard contents).
///
/// The cache block is marked invalid without write-back.  The caller MUST
/// ensure there is no dirty data in the block, otherwise data loss occurs.
/// Used **after** a device DMA write (device wrote fresh data to memory).
///
/// Encoding: funct3=000, opcode=0x0F (CBO)
#[inline]
pub unsafe fn cbo_inval(addr: usize) {
    if !cmo_available() {
        return;
    }
    #[cfg(not(test))]
    core::arch::asm!(".insn i 0x0F, 0x00, x0, 0({addr})", addr = in(reg) addr);
    #[cfg(test)]
    let _ = addr;
}

/// CBO.ZERO — zero an entire cache block without reading from memory first.
///
/// Much faster than a byte-by-byte memset for cache-block-aligned regions.
/// The block is written to the smallest cache-coherence group that
/// includes the address.
///
/// Encoding: funct3=100, opcode=0x0F (CBO)
#[inline]
pub unsafe fn cbo_zero(addr: usize) {
    if !cmo_available() {
        return;
    }
    #[cfg(not(test))]
    core::arch::asm!(".insn i 0x0F, 0x04, x0, 0({addr})", addr = in(reg) addr);
    #[cfg(test)]
    let _ = addr;
}

// ── Range operations ──────────────────────────────────────────────────────

/// Clean (write-back) every cache block touched by `[start, start+len)`.
///
/// Use before DMA reads so device memory is coherent.
pub fn cache_clean_range(start: usize, len: usize) {
    let end = start.wrapping_add(len);
    let mut addr = start & !(CACHE_BLOCK_SIZE - 1);
    while addr < end {
        unsafe { cbo_clean(addr); }
        addr = addr.wrapping_add(CACHE_BLOCK_SIZE);
    }
}

/// Flush (write-back + invalidate) every cache block in `[start, start+len)`.
///
/// Use before DMA writes so the cache does not hold stale data.
pub fn cache_flush_range(start: usize, len: usize) {
    let end = start.wrapping_add(len);
    let mut addr = start & !(CACHE_BLOCK_SIZE - 1);
    while addr < end {
        unsafe { cbo_flush(addr); }
        addr = addr.wrapping_add(CACHE_BLOCK_SIZE);
    }
}

/// Invalidate every cache block in `[start, start+len)`.
///
/// Use after DMA writes to pick up new data written by the device.
/// The caller MUST guarantee no dirty data exists in the range.
pub fn cache_inval_range(start: usize, len: usize) {
    let end = start.wrapping_add(len);
    let mut addr = start & !(CACHE_BLOCK_SIZE - 1);
    while addr < end {
        unsafe { cbo_inval(addr); }
        addr = addr.wrapping_add(CACHE_BLOCK_SIZE);
    }
}

/// Zero a 4 KiB page using CBO.ZERO (16× faster than byte-by-byte memset).
///
/// The page address must be page-aligned.  Falls back to a standard
/// `write_bytes` zero if CMO is not available.
pub fn cache_zero_page(page: usize) {
    debug_assert!(page & 0xFFF == 0, "cache_zero_page: unaligned address");

    if !cmo_available() {
        // Fallback: plain byte-by-byte zero (still correct, just slower)
        unsafe {
            core::ptr::write_bytes(page as *mut u8, 0, 4096);
        }
        return;
    }

    for i in 0..(4096 / CACHE_BLOCK_SIZE) {
        unsafe { cbo_zero(page + i * CACHE_BLOCK_SIZE); }
    }
}

/// Detect whether the current hart supports Zicbom.
///
/// Reads marchid / mimpid / misa to determine CMO availability.
/// Currently returns `false` (detection is deferred to the platform).
pub fn probe_cmo() -> bool {
    // Used by boot code; to be filled with ISA-string parsing once
    // the kernel has a proper extension detection framework.
    false
}
