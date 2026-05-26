/// RISC-V Svnapot — Naturally-Aligned Power-Of-Two page sizes
///
/// Svnapot extends the page table with a N (napot) bit in leaf PTEs,
/// enabling 64 KiB pages (16 contiguous 4 KiB sub-blocks) without
/// requiring a separate level in the page-table walk.
///
/// NAPOT encoding uses PTE bit 63 (N) together with the PPN field:
///   N=1, PPN[3:0] = 0b0000  →  64 KiB page  (16 × 4 KiB)
///   N=1, PPN[3:0] = 0b0001  →  reserved
///   N=1, PPN[3:0] = 0b0011  →  reserved
///   ... (see RISC-V spec Table 77)
///
/// Standard superpages (2 MiB, 1 GiB) are **not** part of Svnapot;
/// they are already supported through the page-table hierarchy.
///
/// Integration: V35a mTHP uses NAPOT 64 KiB as an intermediate size
/// between 4 KiB and 2 MiB.

use core::sync::atomic::{AtomicBool, Ordering};
use crate::mem::sv39::PTE;

// ── Constants ─────────────────────────────────────────────────────────────

/// Number of contiguous 4 KiB pages in a NAPOT block.
pub const NAPOT_PAGES_64K: usize = 16;

/// Standard 64 KiB NAPOT order (encoded by PPN[3:0] = 0b0000).
pub const NAPOT_ORDER_64K: u8 = 4;  // 2^4 = 16 pages

/// Maximum NAPOT order supported (ppn[3:0] = 0b0000 → order 4).
/// Note: the architecture allows up to 2^63 pages for N=0, but RISC-V
/// limits NAPOT to the 64 KiB encoding for Sv39.
pub const NAPOT_MAX_ORDER: u8 = 4;

// ── Runtime detection ─────────────────────────────────────────────────────

static SVNAPOT_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Mark Svnapot as available (called during boot if detected).
pub fn set_svnapot_available() {
    SVNAPOT_AVAILABLE.store(true, Ordering::Relaxed);
}

/// Check whether the Svnapot extension is available.
pub fn svnapot_available() -> bool {
    SVNAPOT_AVAILABLE.load(Ordering::Relaxed)
}

// ── PTE helpers ───────────────────────────────────────────────────────────

/// NAPOT bit position in the PTE (bit 63).
const PTE_N_BIT: u64 = 1 << 63;

/// Check whether a PTE has the N (napot) bit set.
pub fn pte_is_napot(pte: u64) -> bool {
    (pte as u64) & PTE_N_BIT != 0
}

/// Set the N (napot) bit on a PTE value.
pub fn pte_set_napot(pte: u64) -> u64 {
    pte | PTE_N_BIT as u64
}

/// Clear the N (napot) bit on a PTE value.
pub fn pte_clear_napot(pte: u64) -> u64 {
    pte & !(PTE_N_BIT as u64)
}

/// Extract the NAPOT size exponent from a PTE value.
///
/// For the standard 64 KiB encoding (PPN[3:0] = 0b0000) this returns
/// `Order::Exact(4)`.  Unrecognised patterns return `None`.
pub fn napot_order_from_pte(pte: u64) -> Option<u8> {
    if !pte_is_napot(pte) {
        return None;
    }
    // Sv39: PPN[3:0] == 0b0000  →  64 KiB
    let ppn_lo = (pte >> 10) & 0xF;
    if ppn_lo == 0 {
        Some(4) // 16 × 4 KiB = 64 KiB
    } else {
        None // reserved encoding
    }
}

// ── Mapping helpers ───────────────────────────────────────────────────────

/// Encode a 64 KiB NAPOT PTE.
///
/// `pa` must be 64 KiB-aligned.  `flags` is a bitmask of R(1), W(2), X(4), U(8).
/// Returns the raw `u64` PTE value with the N bit set.
pub fn napot_pte_64k(pa: usize, flags: u8) -> u64 {
    debug_assert!(pa & 0xFFFF == 0, "napot_pte_64k: pa not 64K-aligned");

    let r = (flags & 1) != 0;
    let w = (flags & 2) != 0;
    let x = (flags & 4) != 0;
    let u = (flags & 8) != 0;

    let mut pte = 0u64;
    // V(0), R(1), W(2), X(3), U(4)
    pte |= 1;      // V
    if r { pte |= 1 << 1; }
    if w { pte |= 1 << 2; }
    if x { pte |= 1 << 3; }
    if u { pte |= 1 << 4; }
    // A(6), D(7)
    pte |= 1 << 6;  // A
    pte |= 1 << 7;  // D

    // PPN[43:0] = pa >> 12, but PPN[3:0] must be zero for 64 KiB.
    let ppn = (pa >> 12) & ((1 << 44) - 1);
    debug_assert!(ppn & 0xF == 0, "napot_pte_64k: PPN[3:0] must be zero");
    pte |= (ppn as u64) << 10;

    // N bit (Svapot)
    pte |= PTE_N_BIT;

    pte
}

/// Try to map a 64 KiB region using a single NAPOT PTE.
///
/// `root_phys` — physical address of the L2 page table.
/// `va` — must be 64 KiB-aligned.
/// `pa` — must be 64 KiB-aligned.
/// `flags` — bitmask R(1), W(2), X(4), U(8).
///
/// Returns `true` if the NAPOT mapping was created, `false` on failure
/// (e.g. Svnapot not available, misaligned addresses, or allocation failure).
pub fn try_map_64k(root_phys: usize, va: usize, pa: usize, flags: u8) -> bool {
    if !svnapot_available() {
        return false;
    }
    if va & 0xFFFF != 0 || pa & 0xFFFF != 0 {
        return false; // must be 64 KiB-aligned
    }

    let vpn2_idx = (va >> 30) & 0x1FF;
    let vpn1_idx = (va >> 21) & 0x1FF;

    unsafe {
        let l2 = &mut *(crate::mem::sv39::pa_to_kva(root_phys) as *mut [PTE; 512]);

        // Ensure L2 → L1 page exists
        if !l2[vpn2_idx].is_valid() {
            let l1_page = match crate::mem::buddy::alloc_page() {
                Some(p) => p,
                None => return false,
            };
            core::ptr::write_bytes(
                crate::mem::sv39::pa_to_kva(l1_page) as *mut u8,
                0,
                4096,
            );
            let mut entry = PTE::empty();
            entry.set_ppn(l1_page >> 12);
            l2[vpn2_idx] = entry;
        } else if l2[vpn2_idx].is_leaf() {
            return false; // 1 GiB superpage — can't sub-map
        }

        let l1 = &mut *(crate::mem::sv39::pa_to_kva(l2[vpn2_idx].phys_addr()) as *mut [PTE; 512]);
        if l1[vpn1_idx].is_valid() {
            return false; // already mapped
        }

        let raw_pte = napot_pte_64k(pa, flags);
        l1[vpn1_idx] = PTE::empty(); // overwrite; set_raw() via raw_pte
        // Write the full u64 PTE value through a raw pointer derived from the
        // base L1 page virtual address, avoiding reference-to-pointer UB.
        let l1_base = crate::mem::sv39::pa_to_kva(l2[vpn2_idx].phys_addr()) as *mut PTE;
        core::ptr::write_volatile(
            l1_base.add(vpn1_idx) as *mut u64,
            raw_pte,
        );
    }
    true
}

/// Split a 64 KiB NAPOT PTE into 16 regular 4 KiB pages.
///
/// Allocates an L0 page table page and fills it with individual PTEs.
/// Returns `Ok(())` on success or an error message.
pub fn split_napot_64k(root_phys: usize, va: usize) -> Result<(), &'static str> {
    let vpn2_idx = (va >> 30) & 0x1FF;
    let vpn1_idx = (va >> 21) & 0x1FF;

    unsafe {
        let l2 = &*(crate::mem::sv39::pa_to_kva(root_phys) as *const [PTE; 512]);
        if !l2[vpn2_idx].is_valid() || l2[vpn2_idx].is_leaf() {
            return Err("split_napot_64k: L2 invalid or leaf");
        }

        let l1_phys = l2[vpn2_idx].phys_addr();
        let l1 = &mut *(crate::mem::sv39::pa_to_kva(l1_phys) as *mut [PTE; 512]);

        // Read the current NAPOT PTE as raw u64
        let raw_pte = core::ptr::read_volatile(&l1[vpn1_idx] as *const PTE as *const u64);
        if !pte_is_napot(raw_pte) {
            return Err("split_napot_64k: not a NAPOT PTE");
        }

        let base_pa = (((raw_pte >> 10) & ((1 << 44) - 1)) << 12) as usize;
        let r = (raw_pte >> 1) & 1 != 0;
        let w = (raw_pte >> 2) & 1 != 0;
        let x = (raw_pte >> 3) & 1 != 0;
        let u = (raw_pte >> 4) & 1 != 0;

        // Allocate L0 page table page
        let l0_page = match crate::mem::buddy::alloc_page() {
            Some(p) => p,
            None => return Err("split_napot_64k: OOM"),
        };
        let l0 = &mut *(crate::mem::sv39::pa_to_kva(l0_page) as *mut [PTE; 512]);

        // Fill 16 × 4 KiB entries
        for i in 0usize..16 {
            let page_pa = base_pa + i * 4096;
            let mut pte = PTE::empty();
            pte.set_ppn(page_pa >> 12);
            pte.set_flags(r, w, x, u);
            pte.set_accessed(true);
            pte.set_dirty(true);
            l0[i] = pte;
        }

        // Replace NAPOT leaf with branch to L0
        let mut entry = PTE::empty();
        entry.set_ppn(l0_page >> 12);
        l1[vpn1_idx] = entry;

        core::arch::asm!("sfence.vma {}", in(reg) va);
    }
    Ok(())
}
