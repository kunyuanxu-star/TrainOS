// V36d — RISC-V Pointer Masking (Ssnpm for S-mode, Smmpm for M-mode)
//
// Pointer masking allows the top bits of addresses to be ignored during
// address translation, enabling:
//   - Hardware memory tagging (MTE-like)
//   - Probabilistic memory safety (tagged pointers for use-after-free detection)
//   - Garbage collection barriers (for the V28 WASM runtime)
//   - Capability pointer tagging (V27 CHERI software capabilities)
//
// RISC-V spec: Ssnpm (S-supervisor pointer masking), Smmpm (M-machine pointer masking)
// Ratified as part of the RISC-V specification (version 1.0).
//
// PMLEN field in senvcfg (S-mode) / menvcfg (M-mode) determines how many
// upper bits are masked. Typical values:
//   PMLEN=0 — pointer masking disabled
//   PMLEN=1 — mask 1 bit  (bit 63)
//   PMLEN=7 — mask 7 bits (bits 63:57) — useful for 57-bit virtual address Sv57
//   PMLEN=4 — mask 4 bits (bits 63:60) — for Sv48 with 4-bit tag

/// Number of masked pointer bits (PMLEN).
/// For Sv39 we have 39-bit virtual addresses + 25 unused upper bits.
/// Masking the top 7 bits (PMLEN=7) leaves room for 7-bit tags.
pub const PTR_MASK_LENGTH_DEFAULT: u8 = 7;

/// Mask modes for pointer masking.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PtrMaskMode {
    /// Only mask in bare (untranslated) mode.
    BareOnly = 0,
    /// Mask in all virtual memory modes (Sv39/Sv48/Sv57).
    SvModes = 1,
}

/// Configuration for pointer masking.
#[derive(Clone, Copy, Debug)]
pub struct PtrMaskConfig {
    /// Number of masked bits (PMLEN, typically 7 for 7-bit tag).
    pub mask_length: u8,
    /// Where masking applies.
    pub mode: PtrMaskMode,
    /// Enable in M-mode (Smmpm) — affects menvcfg.
    pub machine_mode: bool,
}

impl PtrMaskConfig {
    /// Default configuration: 7-bit tag, Sv modes only, S-mode only.
    pub const fn default() -> Self {
        PtrMaskConfig {
            mask_length: PTR_MASK_LENGTH_DEFAULT,
            mode: PtrMaskMode::SvModes,
            machine_mode: false,
        }
    }

    /// Configuration for M-mode use (Smmpm).
    pub const fn machine() -> Self {
        PtrMaskConfig {
            mask_length: PTR_MASK_LENGTH_DEFAULT,
            mode: PtrMaskMode::SvModes,
            machine_mode: true,
        }
    }
}

/// Check whether pointer masking is available.
///
/// Reads the ISA string or uses a probe: writes senvcfg.PME, reads back,
/// and checks if the bit stuck.
pub fn ptr_mask_available() -> bool {
    unsafe {
        // Probe S-mode pointer masking:
        // Write 1 to senvcfg.PME (bit 3), read back. If it sticks, PM is supported.
        let mut senvcfg: usize;
        core::arch::asm!("csrr {}, senvcfg", out(reg) senvcfg);
        let original = senvcfg;
        senvcfg |= 1 << 3; // PME bit
        core::arch::asm!("csrw senvcfg, {}", in(reg) senvcfg);
        core::arch::asm!("csrr {}, senvcfg", out(reg) senvcfg);
        let supported = (senvcfg & (1 << 3)) != 0;
        // Restore
        core::arch::asm!("csrw senvcfg, {}", in(reg) original);
        supported
    }
}

/// Enable pointer masking with the given configuration.
pub fn enable_pointer_masking(cfg: PtrMaskConfig) {
    let pmlen_val = (cfg.mask_length as usize) & 0x7;
    let pme_bit = 1u8 << 3; // PME enable

    // Build PMM field (bits 32-35 for senvcfg, bits 32-35 for menvcfg)
    let pmm_field = (pmlen_val << 2) | (cfg.mode as usize);

    unsafe {
        if cfg.machine_mode {
            // Smmpm: configure menvcfg
            let mut menvcfg: usize;
            core::arch::asm!("csrr {}, menvcfg", out(reg) menvcfg);
            // Clear PMM field (bits 32-35) and PME (bit 3)
            menvcfg &= !(0x7usize << 32); // PMM
            menvcfg &= !(1 << 3);          // PME
            menvcfg |= (pmm_field << 30);  // PMM at bits 32-35
            menvcfg |= (pme_bit as usize) << 0; // PME at bit 3
            core::arch::asm!("csrw menvcfg, {}", in(reg) menvcfg);
        } else {
            // Ssnpm: configure senvcfg
            let mut senvcfg: usize;
            core::arch::asm!("csrr {}, senvcfg", out(reg) senvcfg);
            // Clear PMM field (bits 32-35) and PME (bit 3)
            senvcfg &= !(0x7usize << 32); // PMM
            senvcfg &= !(1 << 3);          // PME
            senvcfg |= (pmm_field << 30);  // PMM at bits 32-35
            senvcfg |= (pme_bit as usize) << 0; // PME at bit 3
            core::arch::asm!("csrw senvcfg, {}", in(reg) senvcfg);
        }
    }
}

/// Disable pointer masking.
pub fn disable_pointer_masking(machine_mode: bool) {
    unsafe {
        if machine_mode {
            let mut menvcfg: usize;
            core::arch::asm!("csrr {}, menvcfg", out(reg) menvcfg);
            menvcfg &= !(1 << 3); // Clear PME
            core::arch::asm!("csrw menvcfg, {}", in(reg) menvcfg);
        } else {
            let mut senvcfg: usize;
            core::arch::asm!("csrr {}, senvcfg", out(reg) senvcfg);
            senvcfg &= !(1 << 3); // Clear PME
            core::arch::asm!("csrw senvcfg, {}", in(reg) senvcfg);
        }
    }
}

/// Encode a hardware memory tag into the upper bits of a pointer.
///
/// With PMLEN=7, the top 7 bits (bits 63:57) carry the tag.
/// The tag is AND-ed with `((1 << PMLEN) - 1)` to stay within bounds.
///
/// # Safety
///
/// The tagged pointer must be used with pointer masking enabled,
/// otherwise it will address a different memory location.
pub fn tag_pointer(ptr: usize, tag: u8, pmlen: u8) -> usize {
    let mask_len = pmlen.min(PTR_MASK_LENGTH_DEFAULT) as usize;
    let tag_bits = (tag as usize) & ((1 << mask_len) - 1);
    let shift = 64 - mask_len;
    // Clear upper mask_len bits, then OR with tag
    let clear_mask = (1usize << shift) - 1;
    (ptr & clear_mask) | (tag_bits << shift)
}

/// Strip the pointer mask (clear upper bits), returning the canonical address.
///
/// With PMLEN=7, clears the top 7 bits (bits 63:57), leaving the remaining
/// 57 bits as the canonical address.
pub fn strip_tag(ptr: usize, pmlen: u8) -> usize {
    let mask_len = pmlen.min(PTR_MASK_LENGTH_DEFAULT) as usize;
    let shift = 64 - mask_len;
    // Keep only lower (64 - mask_len) bits
    let clear_mask = (1usize << shift) - 1;
    ptr & clear_mask
}

/// Extract the tag from a tagged pointer.
pub fn extract_tag(ptr: usize, pmlen: u8) -> u8 {
    let mask_len = pmlen.min(PTR_MASK_LENGTH_DEFAULT) as usize;
    let shift = 64 - mask_len;
    ((ptr >> shift) & ((1usize << mask_len) - 1)) as u8
}

/// Tag a pointer for capability use (7-bit tag for V27 CHERI software caps).
pub fn tag_with_cap(ptr: usize, cap_slot: u8) -> usize {
    tag_pointer(ptr, cap_slot, PTR_MASK_LENGTH_DEFAULT)
}

/// Tag a pointer with an allocation generation number for
/// use-after-free detection (V36d memory safety).
pub fn tag_with_generation(ptr: usize, generation: u8) -> usize {
    tag_pointer(ptr, generation, PTR_MASK_LENGTH_DEFAULT)
}

/// Verify that a tagged pointer's tag matches the expected value.
/// Returns `true` if the tag matches (pointer is valid).
pub fn verify_tag(ptr: usize, expected_tag: u8, pmlen: u8) -> bool {
    extract_tag(ptr, pmlen) == expected_tag
}

// ── V28 WASM GC barrier integration ──────────────────────────────────────

/// Tag for WASM GC objects: generation number for incremental GC barrier.
pub const WASM_GC_TAG_YOUNG: u8 = 0x01;
pub const WASM_GC_TAG_OLD: u8 = 0x02;
pub const WASM_GC_TAG_MARKED: u8 = 0x04;

/// Set the GC color tag on a WASM object pointer.
pub fn wasm_gc_tag(ptr: usize, color: u8) -> usize {
    tag_pointer(ptr, color, PTR_MASK_LENGTH_DEFAULT)
}

/// Check if a WASM object pointer has the given GC color.
pub fn wasm_gc_check(ptr: usize, color: u8) -> bool {
    verify_tag(ptr, color, PTR_MASK_LENGTH_DEFAULT)
}

// ── TEE / V27 capability integration ────────────────────────────────────

/// Maximum capability slot index for pointer-tagged CHERI software caps.
pub const CAP_TAG_MAX_SLOT: u8 = 0x7F; // 127 cap slots in 7-bit tag

/// Encode a capability slot number as a pointer tag.
pub fn cap_tag(ptr: usize, slot: u8) -> usize {
    tag_with_cap(ptr, slot & CAP_TAG_MAX_SLOT)
}

/// Extract the capability slot from a tagged pointer.
pub fn cap_extract(ptr: usize) -> u8 {
    extract_tag(ptr, PTR_MASK_LENGTH_DEFAULT)
}

// ── Tests (for Rust test harness) ───────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_roundtrip() {
        let ptr = 0x8000_0000usize;
        let tagged = tag_pointer(ptr, 0x5A, 7);
        assert_eq!(strip_tag(tagged, 7), ptr);
        assert_eq!(extract_tag(tagged, 7), 0x5A);
    }

    #[test]
    fn test_tag_masked() {
        let ptr = 0x8000_0000usize;
        // Tag value 0xFF should be masked to 0x7F with PMLEN=7
        let tagged = tag_pointer(ptr, 0xFF, 7);
        assert_eq!(extract_tag(tagged, 7), 0x7F);
    }

    #[test]
    fn test_cap_tag() {
        let ptr = 0x1000_0000usize;
        let tagged = cap_tag(ptr, 42);
        assert_eq!(cap_extract(tagged), 42);
        assert_eq!(strip_tag(tagged, PTR_MASK_LENGTH_DEFAULT), ptr);
    }

    #[test]
    fn test_verify_tag() {
        let ptr = 0x2000_0000usize;
        let tagged = tag_pointer(ptr, 0x3A, 7);
        assert!(verify_tag(tagged, 0x3A, 7));
        assert!(!verify_tag(tagged, 0x2A, 7));
    }
}
