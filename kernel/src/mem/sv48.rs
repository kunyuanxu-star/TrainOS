/// RISC-V Sv48/Sv57 — Extended Virtual Addressing
///
/// Sv48: 48-bit virtual address, 4-level page table (256 TiB user space)
/// Sv57: 57-bit virtual address, 5-level page table (128 PiB user space)
///
/// SATP modes:
///   Sv39 = 8  (512 GiB,  3-level, current default)
///   Sv48 = 9  (256 TiB,  4-level)
///   Sv57 = 10 (128 PiB,  5-level)
///
/// This module provides:
///   - Sv48 page table walk (4-level: L3 → L2 → L1 → L0)
///   - Sv57 detection and helpers (5-level)
///   - AddressingMode enum for runtime mode selection
///   - SATP value construction for each mode
///
/// The PTE format is the same for Sv39, Sv48, and Sv57 (64-bit,
/// with 44-bit PPN in bits 53:10).  We reuse `sv39::PTE` directly.

use super::layout::PAGE_SIZE;
use super::sv39::{page_align_down, page_align_up, PTE};

// ── Constants ──────────────────────────────────────────────────────────

/// Bit-width of VPN fields at each page table level.
pub const VPN_BITS: usize = 9;

/// VPN mask for a single level (9 bits).
pub const VPN_MASK: usize = 0x1FF;

/// Shift amounts for Sv48 virtual address decomposition.
pub const VPN3_SHIFT: usize = 39; // VPN[3] (top level for Sv48)
pub const VPN2_SHIFT: usize = 30; // VPN[2]
pub const VPN1_SHIFT: usize = 21; // VPN[1]
pub const VPN0_SHIFT: usize = 12; // VPN[0] (leaf level)

/// Shift amount for Sv57 virtual address decomposition (extra level).
pub const VPN4_SHIFT: usize = 48; // VPN[4] (top level for Sv57)

/// Page table levels.
pub const SV48_LEVELS: usize = 4;
pub const SV57_LEVELS: usize = 5;

// ── VPN Decomposition Helpers ──────────────────────────────────────────

/// Extract VPN[3] (bits 47:39) from a virtual address — top level for Sv48.
pub fn vpn3(va: usize) -> usize {
    (va >> VPN3_SHIFT) & VPN_MASK
}

/// Extract VPN[2] (bits 38:30).
pub fn vpn2(va: usize) -> usize {
    (va >> VPN2_SHIFT) & VPN_MASK
}

/// Extract VPN[1] (bits 29:21).
pub fn vpn1(va: usize) -> usize {
    (va >> VPN1_SHIFT) & VPN_MASK
}

/// Extract VPN[0] (bits 20:12) — leaf level index.
pub fn vpn0(va: usize) -> usize {
    (va >> VPN0_SHIFT) & VPN_MASK
}

/// Extract VPN[4] (bits 56:48) — top level for Sv57.
pub fn vpn4(va: usize) -> usize {
    (va >> VPN4_SHIFT) & VPN_MASK
}

/// Extract the page offset (bits 11:0).
pub fn offset(va: usize) -> usize {
    va & (PAGE_SIZE - 1)
}

// ── Addressing Mode ────────────────────────────────────────────────────

/// Virtual addressing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressingMode {
    /// Sv39 — 512 GiB virtual address space, 3-level page table.
    Sv39,
    /// Sv48 — 256 TiB virtual address space, 4-level page table.
    Sv48,
    /// Sv57 — 128 PiB virtual address space, 5-level page table.
    Sv57,
}

impl AddressingMode {
    /// Detect the current addressing mode from the live SATP CSR.
    #[cfg(not(test))]
    pub fn detect() -> Self {
        unsafe {
            let satp: usize;
            core::arch::asm!("csrr {}, satp", out(reg) satp);
            let mode = satp >> 60;
            match mode {
                9 => AddressingMode::Sv48,
                10 => AddressingMode::Sv57,
                _ => AddressingMode::Sv39,
            }
        }
    }

    #[cfg(test)]
    pub fn detect() -> Self {
        AddressingMode::Sv39
    }

    /// Return the number of page table levels for this mode.
    pub fn page_table_levels(&self) -> usize {
        match self {
            AddressingMode::Sv39 => 3,
            AddressingMode::Sv48 => 4,
            AddressingMode::Sv57 => 5,
        }
    }

    /// Return the number of virtual address bits for this mode.
    pub fn virtual_address_bits(&self) -> usize {
        match self {
            AddressingMode::Sv39 => 39,
            AddressingMode::Sv48 => 48,
            AddressingMode::Sv57 => 57,
        }
    }

    /// Return the SATP MODE field value for this mode.
    pub fn satp_mode_value(&self) -> usize {
        match self {
            AddressingMode::Sv39 => 8,
            AddressingMode::Sv48 => 9,
            AddressingMode::Sv57 => 10,
        }
    }

    /// Return the highest canonical user-space address for this mode.
    pub fn user_end(&self) -> usize {
        match self {
            AddressingMode::Sv39 => 0x0000_003F_FFFF_FFFF,
            AddressingMode::Sv48 => 0x0000_FFFF_FFFF_FFFF,
            AddressingMode::Sv57 => 0x00FF_FFFF_FFFF_FFFF,
        }
    }

    /// Return the lowest canonical kernel-space address for this mode.
    pub fn kernel_start(&self) -> usize {
        match self {
            AddressingMode::Sv39 => 0xFFFF_FFC0_0000_0000,
            AddressingMode::Sv48 => 0xFFFF_8000_0000_0000,
            AddressingMode::Sv57 => 0xFF00_0000_0000_0000,
        }
    }

    /// Maximum physical address supported (Sv48 and Sv57 both use 57-bit PA).
    pub fn max_phys_addr(&self) -> usize {
        match self {
            AddressingMode::Sv39 => (1usize << 56) - 1,
            AddressingMode::Sv48 => (1usize << 56) - 1,
            AddressingMode::Sv57 => (1usize << 56) - 1,
        }
    }
}

// ── SATP Construction ──────────────────────────────────────────────────

/// Make a SATP CSR value for the given mode and page table root.
pub fn make_satp(mode: AddressingMode, root_phys: usize) -> usize {
    (mode.satp_mode_value() << 60) | (root_phys >> 12)
}

/// Make a SATP CSR value for Sv48 mode.
pub fn sv48_make_satp(root_phys: usize) -> usize {
    make_satp(AddressingMode::Sv48, root_phys)
}

/// Make a SATP CSR value for Sv57 mode.
pub fn sv57_make_satp(root_phys: usize) -> usize {
    make_satp(AddressingMode::Sv57, root_phys)
}

/// Make a SATP CSR value for Sv39 mode (compatibility wrapper).
pub fn sv39_make_satp(root_phys: usize) -> usize {
    make_satp(AddressingMode::Sv39, root_phys)
}

// ── Availability Detection ─────────────────────────────────────────────

/// Probe for Sv48 support by attempting to write SATP with Sv48 mode.
///
/// On platforms that support Sv48, this will succeed.  On those that
/// don't (e.g. QEMU virt without explicit Sv48 configuration), the
/// write will be ignored or trap.
///
/// SAFETY: Modifying SATP at runtime without a valid page table in
/// Sv48 format will crash the system.  This function should only be
/// called during early boot before user processes are running, with
/// a valid Sv48 page table already prepared.
///
/// Returns `true` if Sv48 mode was accepted.
#[cfg(not(test))]
pub unsafe fn sv48_probe() -> bool {
    // Read current SATP to get the root page table PPB
    let satp: usize;
    core::arch::asm!("csrr {}, satp", out(reg) satp);
    let root_ppn = satp & ((1usize << 44) - 1);

    // Try writing SATP with Sv48 mode (mode=9)
    let sv48_val = (9usize << 60) | root_ppn;
    core::arch::asm!("csrw satp, {}", in(reg) sv48_val);
    core::arch::asm!("sfence.vma");

    // Read back to check if mode field accepted Sv48
    let readback: usize;
    core::arch::asm!("csrr {}, satp", out(reg) readback);
    let mode = readback >> 60;

    // Restore original mode regardless of result
    core::arch::asm!("csrw satp, {}", in(reg) satp);
    core::arch::asm!("sfence.vma");

    mode == 9 || mode == 10
}

/// Probe for Sv57 support (similar to Sv48 probe).
#[cfg(not(test))]
pub unsafe fn sv57_probe() -> bool {
    let satp: usize;
    core::arch::asm!("csrr {}, satp", out(reg) satp);
    let root_ppn = satp & ((1usize << 44) - 1);

    let sv57_val = (10usize << 60) | root_ppn;
    core::arch::asm!("csrw satp, {}", in(reg) sv57_val);
    core::arch::asm!("sfence.vma");

    let readback: usize;
    core::arch::asm!("csrr {}, satp", out(reg) readback);
    let mode = readback >> 60;

    core::arch::asm!("csrw satp, {}", in(reg) satp);
    core::arch::asm!("sfence.vma");

    mode == 10
}

/// Check if Sv48 is available without probing (via ISA string check).
/// For platforms where the ISA string is known, this avoids the risk
/// of the SATP probe.  Default: false (assume only Sv39).
pub fn sv48_is_supported() -> bool {
    false
}

/// Check if Sv57 is available.
pub fn sv57_is_supported() -> bool {
    false
}

// ── Sv48 Page Table Walk ──────────────────────────────────────────────

/// Walk a 4-level Sv48 page table for a given virtual address.
///
/// Returns `(phys_addr_of_l0_page_table, vpn0_index)` on success.
/// Creates intermediate page table pages if `alloc` is true.
///
/// Returns `None` if:
///   - An intermediate level is missing and `alloc` is false, or
///   - A large page (1 GiB or 2 MiB) is encountered (not yet handled).
pub unsafe fn sv48_walk(root_phys: usize, va: usize, alloc: bool) -> Option<(usize, usize)> {
    let v3 = vpn3(va);
    let v2 = vpn2(va);
    let v1 = vpn1(va);
    let v0 = vpn0(va);

    // L3 (root) → L2
    let l3 = &mut *(super::sv39::pa_to_kva(root_phys) as *mut [PTE; 512]);
    if l3[v3].is_leaf() {
        return None; // 512 GiB superpage — unsupported
    }
    let l2_phys = ensure_level(l3, v3, alloc)?;

    // L2 → L1
    let l2 = &mut *(super::sv39::pa_to_kva(l2_phys) as *mut [PTE; 512]);
    if l2[v2].is_leaf() {
        return None; // 1 GiB superpage — unsupported at leaf level
    }
    let l1_phys = ensure_level(l2, v2, alloc)?;

    // L1 → L0
    let l1 = &mut *(super::sv39::pa_to_kva(l1_phys) as *mut [PTE; 512]);
    if l1[v1].is_leaf() {
        return None; // 2 MiB superpage — unsupported at leaf level
    }
    let l0_phys = ensure_level(l1, v1, alloc)?;

    Some((l0_phys, v0))
}

/// Helper: look up a PTE at a given index.  If the entry is invalid
/// and `alloc` is true, allocate a fresh page table page.
unsafe fn ensure_level(table: &mut [PTE; 512], index: usize, alloc: bool) -> Option<usize> {
    if !table[index].is_valid() {
        if !alloc {
            return None;
        }
        let new_page = super::buddy::alloc_page()?;
        let new_pt = &mut *(super::sv39::pa_to_kva(new_page) as *mut [PTE; 512]);
        for pte in new_pt.iter_mut() {
            *pte = PTE::empty();
        }
        let mut entry = PTE::empty();
        entry.set_ppn(new_page >> 12);
        entry.set_flags(false, false, false, false); // non-leaf
        table[index] = entry;
        Some(new_page)
    } else {
        // Must be a branch (non-leaf) PTE
        if table[index].is_leaf() {
            return None;
        }
        Some(table[index].phys_addr())
    }
}

// ── Sv48 Map / Unmap ──────────────────────────────────────────────────

/// Map a virtual address to a physical page using Sv48.
///
/// `flags` is a bitmask: bit 0 = R, bit 1 = W, bit 2 = X, bit 3 = U.
pub unsafe fn sv48_map(root_phys: usize, va: usize, pa: usize, flags: u8) {
    let (l0_phys, idx) = sv48_walk(root_phys, va, true)
        .expect("sv48_map: page table walk failed");
    let l0 = &mut *(super::sv39::pa_to_kva(l0_phys) as *mut [PTE; 512]);
    let r = flags & 1 != 0;
    let w = flags & 2 != 0;
    let x = flags & 4 != 0;
    let u = flags & 8 != 0;
    let mut pte = PTE::empty();
    pte.set_ppn(pa >> 12);
    pte.set_flags(r, w, x, u);
    pte.set_accessed(true);
    pte.set_dirty(true);
    l0[idx] = pte;
}

/// Unmap a virtual address from an Sv48 page table.
///
/// Returns the physical address of the unmapped page, or `None` if
/// the VA was not mapped.
pub unsafe fn sv48_unmap(root_phys: usize, va: usize) -> Option<usize> {
    let (l0_phys, idx) = sv48_walk(root_phys, va, false)?;
    let l0 = &mut *(super::sv39::pa_to_kva(l0_phys) as *mut [PTE; 512]);
    let pte = l0[idx];
    if pte.is_valid() && pte.is_leaf() {
        l0[idx] = PTE::empty();
        Some(pte.phys_addr())
    } else {
        None
    }
}

/// Translate a virtual address to its physical address using Sv48.
pub unsafe fn sv48_virt_to_phys(root_phys: usize, va: usize) -> Option<usize> {
    let (l0_phys, idx) = sv48_walk(root_phys, va, false)?;
    let l0 = &*(super::sv39::pa_to_kva(l0_phys) as *const [PTE; 512]);
    let pte = l0[idx];
    if pte.is_valid() && pte.is_leaf() {
        Some(pte.phys_addr() | offset(va))
    } else {
        None
    }
}

/// Allocate and initialize a root (L3) page table for Sv48.
///
/// Returns the physical address of the allocated (zeroed) page,
/// or `None` on OOM.
pub fn sv48_alloc_root() -> Option<usize> {
    let page = unsafe { super::buddy::alloc_page()? };
    unsafe {
        let pt = &mut *(super::sv39::pa_to_kva(page) as *mut [PTE; 512]);
        for pte in pt.iter_mut() {
            *pte = PTE::empty();
        }
    }
    Some(page)
}

// ── Page Table Statistics ──────────────────────────────────────────

/// Count mapped pages in an Sv48 page table.
pub fn sv48_count_pages(root_phys: usize) -> usize {
    let mut count = 0usize;
    unsafe {
        let l3 = &*(super::sv39::pa_to_kva(root_phys) as *const [PTE; 512]);
        for v3 in 0..512 {
            let l3e = l3[v3];
            if !l3e.is_valid() || l3e.is_leaf() {
                continue;
            }
            let l2 = &*(super::sv39::pa_to_kva(l3e.phys_addr()) as *const [PTE; 512]);
            for v2 in 0..512 {
                let l2e = l2[v2];
                if !l2e.is_valid() {
                    continue;
                }
                if l2e.is_leaf() {
                    count += 512; // 1 GiB superpage = 512 x 2 MiB regions
                    continue;
                }
                let l1 = &*(super::sv39::pa_to_kva(l2e.phys_addr()) as *const [PTE; 512]);
                for v1 in 0..512 {
                    let l1e = l1[v1];
                    if !l1e.is_valid() {
                        continue;
                    }
                    if l1e.is_leaf() {
                        count += 512; // 2 MiB superpage = 512 x 4 KiB pages
                        continue;
                    }
                    let l0 = &*(super::sv39::pa_to_kva(l1e.phys_addr()) as *const [PTE; 512]);
                    for v0 in 0..512 {
                        if l0[v0].is_valid() && l0[v0].is_leaf() {
                            count += 1;
                        }
                    }
                }
            }
        }
    }
    count
}
