/// RISC-V Svpbmt — Page-Based Memory Types
///
/// Svpbmt allows specifying the memory type per page-table entry,
/// overriding the default PMA (Physical Memory Attributes).
///
/// PTE bits [62:61] encode the memory type:
///   00 = PMA  — default (from PMA registers / platform)
///   01 = NC   — Non-cacheable, non-idempotent (e.g. MMIO)
///   10 = IO   — Non-cacheable, idempotent (e.g. device memory)
///   11 = reserved
///
/// Integration:
/// - PBMT_IO for GPU device memory mappings (V29 AI-Native OS)
/// - PBMT_NC for legacy MMIO mappings (CLINT, PLIC, UART, VirtIO)

use core::sync::atomic::{AtomicBool, Ordering};

// ── Memory type constants (PTE[62:61]) ────────────────────────────────────

/// Default PMA — follows platform Physical Memory Attributes.
pub const PBMT_PMA: u64 = 0;

/// Non-cacheable, non-idempotent — for MMIO regions (e.g. UART, VirtIO).
pub const PBMT_NC: u64 = 1;

/// Non-cacheable, idempotent — for device memory (e.g. GPU VRAM).
pub const PBMT_IO: u64 = 2;

// ── Runtime detection ─────────────────────────────────────────────────────

static SVPBMT_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Mark Svpbmt as available (called during boot if detected).
pub fn set_svpbmt_available() {
    SVPBMT_AVAILABLE.store(true, Ordering::Relaxed);
}

/// Check whether the Svpbmt extension is available.
pub fn svpbmt_available() -> bool {
    SVPBMT_AVAILABLE.load(Ordering::Relaxed)
}

// ── PTE bit manipulation ──────────────────────────────────────────────────

/// PBMT field mask (2 bits at position 61).
const PBMT_MASK: u64 = 3 << 61;

/// Set the PBMT memory type on a raw `u64` PTE value.
///
/// `mt` must be one of `PBMT_PMA`, `PBMT_NC`, or `PBMT_IO`.
#[inline]
pub fn set_memory_type(pte: u64, mt: u64) -> u64 {
    debug_assert!(mt <= 2, "set_memory_type: invalid memory type");
    (pte & !PBMT_MASK) | ((mt & 3) << 61)
}

/// Extract the PBMT memory type from a raw `u64` PTE value.
///
/// Returns one of `PBMT_PMA`, `PBMT_NC`, or `PBMT_IO`.
#[inline]
pub fn get_memory_type(pte: u64) -> u64 {
    (pte >> 61) & 3
}

// ── Apply Svpbmt memory types to specific (root_phys, va) mappings ─────────

/// Set the memory type for a page-table entry covering `va`.
///
/// Walks the page table rooted at `root_phys` and updates the leaf PTE's
/// PBMT field.  Works for 4 KiB, 64 KiB NAPOT, and 2 MiB leaf entries.
///
/// Returns `true` if the entry was found and updated, `false` otherwise.
pub unsafe fn set_pte_memory_type(
    root_phys: usize,
    va: usize,
    mt: u64,
) -> bool {
    use crate::mem::sv39;

    if mt > 2 {
        return false;
    }
    // Only apply PBMT bits on Svpbmt-capable hardware
    if !svpbmt_available() {
        return false;
    }

    let vpn2 = (va >> 30) & 0x1FF;
    let vpn1 = (va >> 21) & 0x1FF;
    let vpn0 = (va >> 12) & 0x1FF;

    let l2 = &*(sv39::pa_to_kva(root_phys) as *const [sv39::PTE; 512]);

    if !l2[vpn2].is_valid() {
        return false;
    }
    if l2[vpn2].is_leaf() {
        // 1 GiB superpage
        let ptr = &l2[vpn2] as *const sv39::PTE as *mut u64;
        let raw = ptr.read_volatile();
        ptr.write_volatile(set_memory_type(raw, mt));
        return true;
    }

    let l1 = &*(sv39::pa_to_kva(l2[vpn2].phys_addr()) as *const [sv39::PTE; 512]);
    if !l1[vpn1].is_valid() {
        return false;
    }
    if l1[vpn1].is_leaf() {
        // 2 MiB superpage
        let ptr = &l1[vpn1] as *const sv39::PTE as *mut u64;
        let raw = ptr.read_volatile();
        ptr.write_volatile(set_memory_type(raw, mt));
        return true;
    }

    // L0 level — 4 KiB (or NAPOT 64 KiB)
    let l0 = &*(sv39::pa_to_kva(l1[vpn1].phys_addr()) as *const [sv39::PTE; 512]);
    if !l0[vpn0].is_valid() {
        return false;
    }
    let ptr = &l0[vpn0] as *const sv39::PTE as *mut u64;
    let raw = ptr.read_volatile();
    ptr.write_volatile(set_memory_type(raw, mt));
    true
}

/// Helper: mark a mapping as non-cacheable (NC) for MMIO.
pub unsafe fn set_mmio_type(root_phys: usize, va: usize) -> bool {
    set_pte_memory_type(root_phys, va, PBMT_NC)
}

/// Helper: mark a mapping as IO memory (for GPU / device VRAM).
pub unsafe fn set_io_type(root_phys: usize, va: usize) -> bool {
    set_pte_memory_type(root_phys, va, PBMT_IO)
}

/// Helper: restore a mapping to default PMA.
pub unsafe fn set_pma_type(root_phys: usize, va: usize) -> bool {
    set_pte_memory_type(root_phys, va, PBMT_PMA)
}
