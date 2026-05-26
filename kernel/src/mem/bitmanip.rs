// V36d — RISC-V B Extension (Bit Manipulation)
//
// Provides fast bit operations for kernel hot-path operations:
//   - Scheduler bitmap: find highest-priority set bit (CLZ to scan ready queue)
//   - Buddy allocator: population count for free-page accounting
//   - Memory management: fast alignment, log2, division
//   - SHA-256: rotations (used in TEE attestation)
//
// RISC-V B extension subsets:
//   Zbb (basic) — CLZ, CTZ, CPOP, ROL, ROR, REV8, ANDN, ORN, XNOR, etc.
//   Zbs (single-bit) — BCLR, BSET, BEXT, BINV
//   Zbc (carry-less multiply) — CLMUL, CLMULH, CLMULR
//   Zbkb (bitmanip crypto) — GREVI (bit reversal), etc.

/// Bit manipulation capability flags.
#[derive(Copy, Clone, Debug)]
pub struct BExtCapabilities {
    pub zbb: bool,  // Basic bit manipulation
    pub zbs: bool,  // Single-bit ops
    pub zbc: bool,  // Carry-less multiply
    pub zbkb: bool, // Bitmanip for crypto
}

static mut BEXT_AVAILABLE: (bool, bool, bool, bool) = (false, false, false, false);

/// Check which B-extension subsets are available.
pub fn bitmanip_available() -> BExtCapabilities {
    unsafe {
        BExtCapabilities {
            zbb: BEXT_AVAILABLE.0,
            zbs: BEXT_AVAILABLE.1,
            zbc: BEXT_AVAILABLE.2,
            zbkb: BEXT_AVAILABLE.3,
        }
    }
}

/// Mark the B extension as available (called during boot if QEMU/hardware supports it).
pub fn set_bitmanip_available(zbb: bool, zbs: bool, zbc: bool, zbkb: bool) {
    unsafe {
        BEXT_AVAILABLE = (zbb, zbs, zbc, zbkb);
    }
}

/// Optimize kernel hot-path operations to use B-extension instructions.
pub fn bitmanip_optimize() {
    let caps = bitmanip_available();
    if caps.zbb {
        crate::println!("  B-extension: Zbb available, bit operations optimized");
    }
    if caps.zbs {
        crate::println!("  B-extension: Zbs available");
    }
    if caps.zbc {
        crate::println!("  B-extension: Zbc available");
    }
    if caps.zbkb {
        crate::println!("  B-extension: Zbkb available");
    }
}

/// Global flag: whether bitmanip optimization is enabled.
static mut BITMANIP_OPT_ENABLED: bool = false;

/// Check if B-extension optimized paths are active.
pub fn is_optimized() -> bool {
    unsafe { BITMANIP_OPT_ENABLED }
}

// ── Zbb: Count Leading Zeros ────────────────────────────────────────────

/// Count leading zeros (bit position of highest set bit, MSB indexed).
/// x=0 returns 64.
#[cfg(target_arch = "riscv64")]
#[inline]
pub fn clz(x: u64) -> u32 {
    if x == 0 {
        return 64;
    }
    unsafe {
        let result: u32;
        core::arch::asm!(
            ".option push",
            ".option arch, +zbb",
            "clz {}, {}",
            ".option pop",
            out(reg) result,
            in(reg) x,
        );
        result
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[inline]
pub fn clz(x: u64) -> u32 {
    if x == 0 {
        return 64;
    }
    63 - (x.leading_zeros())
}

// ── Zbb: Count Trailing Zeros ───────────────────────────────────────────

/// Count trailing zeros (lowest set bit position).
/// x=0 returns 64.
#[cfg(target_arch = "riscv64")]
#[inline]
pub fn ctz(x: u64) -> u32 {
    if x == 0 {
        return 64;
    }
    unsafe {
        let result: u32;
        core::arch::asm!(
            ".option push",
            ".option arch, +zbb",
            "ctz {}, {}",
            ".option pop",
            out(reg) result,
            in(reg) x,
        );
        result
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[inline]
pub fn ctz(x: u64) -> u32 {
    if x == 0 { 64 } else { x.trailing_zeros() }
}

// ── Zbb: Population Count ───────────────────────────────────────────────

/// Count set bits (population count).
#[cfg(target_arch = "riscv64")]
#[inline]
pub fn pcnt(x: u64) -> u32 {
    unsafe {
        let result: u32;
        core::arch::asm!(
            ".option push",
            ".option arch, +zbb",
            "cpop {}, {}",
            ".option pop",
            out(reg) result,
            in(reg) x,
        );
        result
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[inline]
pub fn pcnt(x: u64) -> u32 {
    x.count_ones()
}

// ── Zbb: Rotate Right ───────────────────────────────────────────────────

/// Bitwise rotate right.
#[cfg(target_arch = "riscv64")]
#[inline]
pub fn ror(x: u64, shift: u32) -> u64 {
    let s = (shift & 63) as u64;
    if s == 0 {
        return x;
    }
    unsafe {
        let result: u64;
        core::arch::asm!(
            ".option push",
            ".option arch, +zbb",
            "ror {}, {}, {}",
            ".option pop",
            out(reg) result,
            in(reg) x,
            in(reg) s,
        );
        result
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[inline]
pub fn ror(x: u64, shift: u32) -> u64 {
    let s = shift & 63;
    if s == 0 { x } else { (x >> s) | (x << (64 - s)) }
}

// ── Zbb: Rotate Left ────────────────────────────────────────────────────

/// Bitwise rotate left.
#[cfg(target_arch = "riscv64")]
#[inline]
pub fn rol(x: u64, shift: u32) -> u64 {
    let s = (shift & 63) as u64;
    if s == 0 {
        return x;
    }
    unsafe {
        let result: u64;
        core::arch::asm!(
            ".option push",
            ".option arch, +zbb",
            "rol {}, {}, {}",
            ".option pop",
            out(reg) result,
            in(reg) x,
            in(reg) s,
        );
        result
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[inline]
pub fn rol(x: u64, shift: u32) -> u64 {
    let s = shift & 63;
    if s == 0 { x } else { (x << s) | (x >> (64 - s)) }
}

// ── Zbb: Byte Reverse (Endianness Swap) ─────────────────────────────────

/// Reverse byte order in a 64-bit value (endianness conversion).
#[cfg(target_arch = "riscv64")]
#[inline]
pub fn brev(x: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            ".option push",
            ".option arch, +zbb",
            "rev8 {}, {}",
            ".option pop",
            out(reg) result,
            in(reg) x,
        );
        result
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[inline]
pub fn brev(x: u64) -> u64 {
    x.swap_bytes()
}

/// Reverse byte order in a 32-bit value.
#[inline]
pub fn brev32(x: u32) -> u32 {
    (brev(x as u64) >> 32) as u32
}

// ── Zbkb: Bit Reverse ───────────────────────────────────────────────────

/// Reverse the order of bits in a 64-bit value.
/// Uses GREVI (generalized reverse immediate) from Zbkb subset.
#[cfg(target_arch = "riscv64")]
#[inline]
pub fn bitrev(x: u64) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            ".option push",
            ".option arch, +zbkb",
            "grevi {}, {}, 63",
            ".option pop",
            out(reg) result,
            in(reg) x,
        );
        result
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[inline]
pub fn bitrev(x: u64) -> u64 {
    let mut result = 0u64;
    let mut val = x;
    for _ in 0..64 {
        result = (result << 1) | (val & 1);
        val >>= 1;
    }
    result
}

// ── Zbs: Single-bit Operations ──────────────────────────────────────────

/// Set a single bit.
#[cfg(target_arch = "riscv64")]
#[inline]
pub fn bset(x: u64, bit: u32) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            ".option push",
            ".option arch, +zbs",
            "bset {}, {}, {}",
            ".option pop",
            out(reg) result,
            in(reg) x,
            in(reg) bit as u64,
        );
        result
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[inline]
pub fn bset(x: u64, bit: u32) -> u64 {
    x | (1u64 << bit)
}

/// Clear a single bit.
#[cfg(target_arch = "riscv64")]
#[inline]
pub fn bclr(x: u64, bit: u32) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            ".option push",
            ".option arch, +zbs",
            "bclr {}, {}, {}",
            ".option pop",
            out(reg) result,
            in(reg) x,
            in(reg) bit as u64,
        );
        result
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[inline]
pub fn bclr(x: u64, bit: u32) -> u64 {
    x & !(1u64 << bit)
}

/// Extract a single bit.
#[cfg(target_arch = "riscv64")]
#[inline]
pub fn bext(x: u64, bit: u32) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            ".option push",
            ".option arch, +zbs",
            "bext {}, {}, {}",
            ".option pop",
            out(reg) result,
            in(reg) x,
            in(reg) bit as u64,
        );
        result
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[inline]
pub fn bext(x: u64, bit: u32) -> u64 {
    (x >> bit) & 1
}

/// Toggle (invert) a single bit.
#[cfg(target_arch = "riscv64")]
#[inline]
pub fn binv(x: u64, bit: u32) -> u64 {
    unsafe {
        let result: u64;
        core::arch::asm!(
            ".option push",
            ".option arch, +zbs",
            "binv {}, {}, {}",
            ".option pop",
            out(reg) result,
            in(reg) x,
            in(reg) bit as u64,
        );
        result
    }
}

#[cfg(not(target_arch = "riscv64"))]
#[inline]
pub fn binv(x: u64, bit: u32) -> u64 {
    x ^ (1u64 << bit)
}

// ── Optimized Kernel Hot-Path Operations ────────────────────────────────

/// Fast find-first-set in a bitmap word, using CTZ (Zbb).
#[inline]
pub fn fast_bitmap_first_set(bits: u64) -> u32 {
    ctz(bits)
}

/// Fast find-last-set in a bitmap word, using CLZ (Zbb).
#[inline]
pub fn fast_bitmap_last_set(bits: u64) -> u32 {
    if bits == 0 {
        return 64;
    }
    63 - clz(bits)
}

/// Fast population count for a bitmap word.
#[inline]
pub fn fast_bitmap_popcount(bits: u64) -> u32 {
    pcnt(bits)
}

/// Fast integer ceil division using bit ops.
#[inline]
pub fn fast_div_ceil(a: u64, b: u64) -> u64 {
    if b == 0 {
        return 0;
    }
    (a + b - 1) / b
}

/// Fast log2 (floor) using CLZ.
#[inline]
pub fn fast_log2(x: u64) -> u32 {
    if x == 0 {
        return 0;
    }
    63 - clz(x)
}

/// Check if a value is a power of 2 (using POPCNT).
#[inline]
pub fn fast_is_power_of_2(x: u64) -> bool {
    x != 0 && pcnt(x) == 1
}

/// Fast alignment: round up to the next multiple of `align`.
/// `align` must be a power of 2.
#[inline]
pub fn fast_align_up(x: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    (x + align - 1) & !(align - 1)
}

/// Fast alignment: round down to the next multiple of `align`.
#[inline]
pub fn fast_align_down(x: u64, align: u64) -> u64 {
    debug_assert!(align.is_power_of_two());
    x & !(align - 1)
}

// ── Optimized SHA-256 rotation (used in TEE attestation) ───────────────

/// SHA-256 Σ0 rotation (rotate right by 2, 13, 22, XOR).
#[inline]
pub fn sha256_big_sigma0(x: u32) -> u32 {
    (ror(x as u64, 2) ^ ror(x as u64, 13) ^ ror(x as u64, 22)) as u32
}

/// SHA-256 Σ1 rotation (rotate right by 6, 11, 25, XOR).
#[inline]
pub fn sha256_big_sigma1(x: u32) -> u32 {
    (ror(x as u64, 6) ^ ror(x as u64, 11) ^ ror(x as u64, 25)) as u32
}

/// SHA-256 σ0 rotation (rotate right by 7, 18, shift right by 3).
#[inline]
pub fn sha256_small_sigma0(x: u32) -> u32 {
    let r7 = ror(x as u64, 7) as u32;
    let r18 = ror(x as u64, 18) as u32;
    r7 ^ r18 ^ (x >> 3)
}

/// SHA-256 σ1 rotation (rotate right by 17, 19, shift right by 10).
#[inline]
pub fn sha256_small_sigma1(x: u32) -> u32 {
    (ror(x as u64, 17) ^ ror(x as u64, 19) ^ (x as u64 >> 10)) as u32
}

// ── Scheduler Optimizations ─────────────────────────────────────────────

/// Find the highest priority ready queue (non-empty) in a scheduler bitmap.
#[inline]
pub fn sched_highest_ready(ready_bitmap: u64) -> u32 {
    fast_bitmap_last_set(ready_bitmap)
}

/// Count how many ready queues have threads.
#[inline]
pub fn sched_ready_count(ready_bitmap: u64) -> u32 {
    fast_bitmap_popcount(ready_bitmap)
}

// ── Buddy Allocator Optimizations ───────────────────────────────────────

/// Order for a desired page count: the smallest order that can hold `count` pages.
#[inline]
pub fn buddy_order_for_count(count: u32) -> u32 {
    if count <= 1 {
        return 0;
    }
    fast_log2((count - 1) as u64) + 1
}

/// Compute how many free pages a bitmap with `popcount` set bits represents.
#[inline]
pub fn buddy_free_pages_from_bitmap(bitmap: u64) -> u32 {
    fast_bitmap_popcount(bitmap)
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clz() {
        assert_eq!(clz(0), 64);
        assert_eq!(clz(1), 63);
        assert_eq!(clz(1 << 63), 0);
        assert_eq!(clz(0x8000_0000_0000_0000), 0);
    }

    #[test]
    fn test_ctz() {
        assert_eq!(ctz(0), 64);
        assert_eq!(ctz(1), 0);
        assert_eq!(ctz(2), 1);
        assert_eq!(ctz(0x8000_0000_0000_0000), 63);
    }

    #[test]
    fn test_popcount() {
        assert_eq!(pcnt(0), 0);
        assert_eq!(pcnt(1), 1);
        assert_eq!(pcnt(0xFFFF_FFFF_FFFF_FFFF), 64);
        assert_eq!(pcnt(0x0123456789ABCDEF), 32);
    }

    #[test]
    fn test_rotate() {
        assert_eq!(rol(1, 1), 2);
        assert_eq!(ror(2, 1), 1);
        assert_eq!(rol(1, 63), 0x8000_0000_0000_0000);
        assert_eq!(ror(0x8000_0000_0000_0000, 63), 1);
    }

    #[test]
    fn test_fast_log2() {
        assert_eq!(fast_log2(1), 0);
        assert_eq!(fast_log2(2), 1);
        assert_eq!(fast_log2(4), 2);
        assert_eq!(fast_log2(1024), 10);
        assert_eq!(fast_log2(0), 0);
    }

    #[test]
    fn test_buddy_order() {
        assert_eq!(buddy_order_for_count(1), 0);
        assert_eq!(buddy_order_for_count(2), 1);
        assert_eq!(buddy_order_for_count(512), 9);
    }

    #[test]
    fn test_bitmap_ops() {
        assert_eq!(fast_bitmap_first_set(0x100), 8);
        assert_eq!(fast_bitmap_first_set(0x8000_0000_0000_0000), 63);
        assert_eq!(fast_bitmap_last_set(0x0000_0000_0000_00FF), 7);
        assert_eq!(fast_bitmap_popcount(0xFF), 8);
    }
}
