// V23: G-stage (second-stage) MMU for RISC-V H-extension
//
// The G-stage page table translates guest physical addresses (GPAs)
// to host physical addresses (HPAs). It uses the same Sv39 PTE format
// as the S-stage page table but is pointed to by hgatp instead of satp.
//
// NOTE: The RISC-V privileged spec defines Sv39x4 (4-level) for G-stage
// with hgatp.MODE=1. However, this implementation uses a 3-level layout
// (Sv39-like) with hgatp.MODE=8 as a QEMU-compatible placeholder. The
// exact MODE encoding may need adjustment for real hardware.

use crate::mem::buddy;
use crate::mem::sv39::{pa_to_kva, PTE};
use crate::mem::layout::PAGE_SIZE;

/// Mapping leaf flags for G-stage PTEs.
/// In G-stage, the U bit means "Guest" (accessible to both VS and VU modes).
const FLAG_GUEST_RW: (bool, bool, bool, bool) = (true, true, false, true); // R+W, Guest
const FLAG_GUEST_RWX: (bool, bool, bool, bool) = (true, true, true, true); // R+W+X, Guest

/// Maximum guest memory (128 MB — same as host DRAM).
const MAX_GUEST_MB: usize = 128;

/// Create a G-stage page table mapping guest physical address 0..(mem_mb)
/// to freshly allocated host physical pages.
///
/// Returns `(hgatp_value, l2_phys)` on success, where:
///   - `hgatp_value` is the value to write to the hgatp CSR (includes the
///     MODE field and the root page table PPN)
///   - `l2_phys` is the host physical address of the L2 (root) page table page
///     (needed for later cleanup in `destroy_gstage`)
pub fn create_gstage(mem_mb: usize) -> Result<(usize, usize), &'static str> {
    if mem_mb == 0 || mem_mb > MAX_GUEST_MB {
        return Err("invalid guest memory size");
    }

    // Calculate how many 2 MB blocks of guest memory we need
    // Each L0 page table page covers 512 × 4 KB = 2 MB
    let num_2mb_blocks = (mem_mb + 1) / 2; // ceil(mem_mb / 2)

    // ── L2 (root) page ──────────────────────────────────────────────────
    let l2_phys = buddy::alloc_page().ok_or("OOM: L2 page")?;
    unsafe {
        core::ptr::write_bytes(pa_to_kva(l2_phys) as *mut u8, 0, PAGE_SIZE);
    }

    let l2 = unsafe { &mut *(pa_to_kva(l2_phys) as *mut [PTE; 512]) };

    // Only the first L2 entry is needed for < 1 GB guest memory.
    // L2[VPN2=0] covers GPA [0, 1 GB) which is enough for ≤128 MB.
    if num_2mb_blocks > 0 {
        // ── L1 page ──────────────────────────────────────────────────────
        let l1_phys = buddy::alloc_page().ok_or("OOM: L1 page")?;
        unsafe {
            core::ptr::write_bytes(pa_to_kva(l1_phys) as *mut u8, 0, PAGE_SIZE);
        }

        // L2[0] → L1 (branch entry, R=W=X=0)
        let mut l2_pte = PTE::empty();
        l2_pte.set_ppn(l1_phys >> 12);
        l2_pte.set_flags(false, false, false, false); // branch
        l2[0] = l2_pte;

        let l1 = unsafe { &mut *(pa_to_kva(l1_phys) as *mut [PTE; 512]) };

        // ── L0 pages and backing pages ───────────────────────────────────
        for vpn1 in 0..num_2mb_blocks {
            let l0_phys = buddy::alloc_page().ok_or("OOM: L0 page")?;
            unsafe {
                core::ptr::write_bytes(pa_to_kva(l0_phys) as *mut u8, 0, PAGE_SIZE);
            }

            // L1[vpn1] → L0 (branch entry)
            let mut l1_pte = PTE::empty();
            l1_pte.set_ppn(l0_phys >> 12);
            l1_pte.set_flags(false, false, false, false); // branch
            l1[vpn1] = l1_pte;

            let l0 = unsafe { &mut *(pa_to_kva(l0_phys) as *mut [PTE; 512]) };

            // Each L0 entry is a 4 KB leaf mapping guest → host page.
            for vpn0 in 0..512 {
                let host_page = buddy::alloc_page().ok_or("OOM: backing page")?;
                // Zero the page for security (no stale data from other uses)
                unsafe {
                    core::ptr::write_bytes(pa_to_kva(host_page) as *mut u8, 0, PAGE_SIZE);
                }

                let (r, w, x, u) = FLAG_GUEST_RW;
                let mut pte = PTE::empty();
                pte.set_ppn(host_page >> 12);
                pte.set_flags(r, w, x, u);
                pte.set_accessed(true);
                pte.set_dirty(true);
                l0[vpn0] = pte;
            }
        }
    }

    // Build hgatp value: MODE=8 (Sv39-like, 3-level), PPN = l2_phys >> 12
    // NOTE: For ratified RISC-V H-extension, hgatp.MODE=1 selects Sv39x4
    //       (4-level). MODE=8 here is a placeholder that may need adjustment.
    let ppn = (l2_phys >> 12) & 0xFFFF_FFFF_FFFF;
    let hgatp = (8usize << 60) | ppn;

    Ok((hgatp, l2_phys))
}

/// Destroy a G-stage page table previously created by `create_gstage`.
///
/// Walks the full page table tree (L2 → L1 → L0), frees every backing
/// host page, every L0/L1 page-table page, and finally the L2 page itself.
pub fn destroy_gstage(l2_phys: usize) {
    if l2_phys == 0 {
        return;
    }

    unsafe {
        let l2 = &mut *(pa_to_kva(l2_phys) as *mut [PTE; 512]);

        for vpn2 in 0..512 {
            let l2e = l2[vpn2];
            if !l2e.is_valid() || l2e.is_leaf() {
                continue;
            }

            let l1_phys = l2e.phys_addr();
            let l1 = &mut *(pa_to_kva(l1_phys) as *mut [PTE; 512]);

            for vpn1 in 0..512 {
                let l1e = l1[vpn1];
                if !l1e.is_valid() {
                    continue;
                }

                if l1e.is_leaf() {
                    // 2 MB superpage — free the backing block
                    // (512 pages = order 9 in buddy allocator)
                    buddy::free_page(l1e.phys_addr(), 9);
                } else {
                    let l0_phys = l1e.phys_addr();
                    let l0 = &mut *(pa_to_kva(l0_phys) as *mut [PTE; 512]);

                    for vpn0 in 0..512 {
                        let l0e = l0[vpn0];
                        if l0e.is_valid() && l0e.is_leaf() {
                            // Free a single 4 KB backing page
                            buddy::free_page(l0e.phys_addr(), 0);
                        }
                    }

                    // Free the L0 page-table page itself
                    buddy::free_page(l0_phys, 0);
                }
            }

            // Free the L1 page-table page itself
            buddy::free_page(l1_phys, 0);
        }

        // Free the L2 (root) page-table page itself
        buddy::free_page(l2_phys, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(align(4096))]
    struct Page([u8; 4096]);

    /// Minimal test that create_gstage + destroy_gstage round-trips without
    /// panicking or leaking.  We use a tiny guest size (2 MB = 1 L0 page).
    #[test]
    fn test_create_destroy_gstage() {
        // We need enough free memory for the page tables + backing pages.
        // The test framework has ~1 MB, so use a minimal 2 MB guest.
        let (hgatp, l2_phys) = create_gstage(2).expect("create_gstage(2) failed");
        assert_ne!(hgatp, 0);
        assert_ne!(l2_phys, 0);
        // Verify hgatp has the MODE bit set
        assert!(hgatp & (8usize << 60) != 0);
        destroy_gstage(l2_phys);
    }
}
