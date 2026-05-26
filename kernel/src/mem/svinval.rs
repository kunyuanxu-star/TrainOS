/// RISC-V Svinval — Fine-Grained TLB Invalidation
///
/// The Svinval extension provides more efficient TLB invalidation on
/// multi-core systems compared to the full `SFENCE.VMA` barrier:
///
///   - `SFENCE.W.INVAL`  — orders previous stores to be visible before
///                          subsequent SINVAL.VMA / HINVAL.VVMA.
///   - `SINVAL.VMA`      — invalidates TLB entries matching a given
///                          virtual address + ASID (no ordering).
///   - `SFENCE.INVAL.IR` — fence that makes the invalidations from
///                          SINVAL.VMA visible.
///
/// The recommended sequence for a given VA + ASID invalidation is:
///   1. SFENCE.W.INVAL
///   2. SINVAL.VMA rs1, rs2
///   3. SFENCE.INVAL.IR
///
/// On systems without Svinval, fall back to `SFENCE.VMA` (which
/// combines ordering + invalidation in one instruction).

use core::sync::atomic::{AtomicBool, Ordering};

// ── Runtime detection ─────────────────────────────────────────────────────

static SVINVAL_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Mark Svinval as available (called during boot if detected).
pub fn set_svinval_available() {
    SVINVAL_AVAILABLE.store(true, Ordering::Relaxed);
}

/// Check whether the Svinval extension is available.
pub fn svinval_available() -> bool {
    SVINVAL_AVAILABLE.load(Ordering::Relaxed)
}

// ── Single-address invalidation ───────────────────────────────────────────

/// Invalidate TLB entries for a single virtual address.
///
/// On Svinval-capable harts uses the three-instruction sequence
/// (SFENCE.W.INVAL; SINVAL.VMA; SFENCE.INVAL.IR).  Otherwise falls
/// back to `SFENCE.VMA` with a single-address operand.
pub fn tlb_inval_va(va: usize) {
    #[cfg(not(test))]
    unsafe {
        if svinval_available() {
            // Step 1: Order preceding stores
            core::arch::asm!("sfence.w.inval");
            // Step 2: Invalidate the TLB entry for `va` (all ASIDs)
            // SINVAL.VMA encodes as funct7=0x18, funct3=0x00, opcode=0x73
            core::arch::asm!(".insn r 0x73, 0x00, 0x18, x0, {va}, x0", va = in(reg) va);
            // Step 3: Make invalidation visible
            core::arch::asm!("sfence.inval.ir");
        } else {
            // Fallback to full SFENCE.VMA
            core::arch::asm!("sfence.vma {}", in(reg) va);
        }
    }
    #[cfg(test)]
    let _ = va;
}

/// Invalidate TLB entries for a virtual address + ASID pair.
///
/// Only invalidates entries matching both the virtual address and
/// the address-space identifier.
pub fn tlb_inval_va_asid(va: usize, asid: usize) {
    #[cfg(not(test))]
    unsafe {
        if svinval_available() {
            core::arch::asm!("sfence.w.inval");
            // SINVAL.VMA with rs1=va, rs2=asid
            core::arch::asm!(".insn r 0x73, 0x00, 0x18, x0, {va}, {asid}", va = in(reg) va, asid = in(reg) asid);
            core::arch::asm!("sfence.inval.ir");
        } else {
            core::arch::asm!("sfence.vma {}, {}", in(reg) va, in(reg) asid);
        }
    }
    #[cfg(test)]
    let _ = (va, asid);
}

/// Invalidate all TLB entries for a specific ASID.
///
/// Useful during process teardown or when recycling an ASID.
pub fn tlb_inval_asid(asid: usize) {
    #[cfg(not(test))]
    unsafe {
        if svinval_available() {
            core::arch::asm!("sfence.w.inval");
            // SINVAL.VMA with rs1=x0 (all VAs), rs2=asid
            core::arch::asm!(".insn r 0x73, 0x00, 0x18, x0, x0, {asid}", asid = in(reg) asid);
            core::arch::asm!("sfence.inval.ir");
        } else {
            core::arch::asm!("sfence.vma zero, {}", in(reg) asid);
        }
    }
    #[cfg(test)]
    let _ = asid;
}

/// Flush all TLB entries (global barrier).
pub fn tlb_flush_all() {
    #[cfg(not(test))]
    unsafe {
        core::arch::asm!("sfence.vma");
    }
}

// ─── Virtualised TLB invalidation (VS-stage) ──────────────────────────────
//
// For hypervisor usage (V23).  These call HINVAL.GVMA / HINVAL.VVMA
// when Svinval is available, otherwise fall back to `HFENCE.VVMA`.

/// Invalidate G-stage TLB entries for a guest physical address.
#[cfg(not(test))]
pub fn hinval_gvma(gpa: usize) {
    if svinval_available() {
        unsafe {
            core::arch::asm!("sfence.w.inval");
            // HINVAL.GVMA with rs1=gpa, rs2=x0 (all VMIDs)
            core::arch::asm!(".insn r 0x73, 0x00, 0x19, x0, {gpa}, x0", gpa = in(reg) gpa);
            core::arch::asm!("sfence.inval.ir");
        }
    } else {
        unsafe {
            core::arch::asm!("hfence.gvma {}, zero", in(reg) gpa);
        }
    }
}

/// Invalidate VS-stage TLB entries for a virtual address.
#[cfg(not(test))]
pub fn hinval_vvma(va: usize) {
    if svinval_available() {
        unsafe {
            core::arch::asm!("sfence.w.inval");
            // HINVAL.VVMA with rs1=va, rs2=x0 (all ASIDs)
            core::arch::asm!(".insn r 0x73, 0x00, 0x1A, x0, {va}, x0", va = in(reg) va);
            core::arch::asm!("sfence.inval.ir");
        }
    } else {
        unsafe {
            core::arch::asm!("hfence.vvma {}, zero", in(reg) va);
        }
    }
}
