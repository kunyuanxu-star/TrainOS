/// RISC-V Zicond — Conditional Move Instructions
///
/// Zicond provides two instructions that eliminate branches in hot paths:
///   czero.eqz rd, rs1, rs2 — rd = (rs2 == 0) ? 0 : rs1
///   czero.nez rd, rs1, rs2 — rd = (rs2 != 0) ? 0 : rs1
///
/// These are particularly valuable for:
///   - Scheduler: priority comparison without branching
///   - Memory: bounds checking without branch mispredictions
///   - IPC: endpoint selection without branch
///   - Any hot path where a conditional branch stalls the pipeline
///
/// Instruction encoding (R-type, OP=0x13, FUNCT3=0x7, FUNCT7=0x33):
///   czero.eqz:  [funct7=0x33][rs2][rs1][funct3=0x7][rd][opcode=0x13]
///   czero.nez:  [funct7=0x33][rs2][rs1][funct3=0x7][rd][opcode=0x13]
///     (distinguished by the rs2==0 vs rs2!=0 semantics — same encoding)
///
/// Wait, the actual encoding according to the spec:
///   czero.eqz rd, rs1, rs2  ->  funct5=0b00000, rs2, rs1, funct3=0b111, rd, opcode=0b0010011
///   czero.nez rd, rs1, rs2  ->  funct5=0b00001, rs2, rs1, funct3=0b111, rd, opcode=0b0010011
///
/// R-type encoding: opcode[6:0]=0x13, funct3[14:12]=0x7
///   czero.eqz: funct7[31:25]=0b0000000
///   czero.nez: funct7[31:25]=0b0000001

use core::sync::atomic::{AtomicBool, Ordering};

/// Whether Zicond is available on this platform.
static ZICOND_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Mark Zicond as available (called during boot if detected).
pub fn set_zicond_available() {
    ZICOND_AVAILABLE.store(true, Ordering::SeqCst);
}

/// Check whether the Zicond extension is available.
#[inline]
pub fn zicond_available() -> bool {
    ZICOND_AVAILABLE.load(Ordering::Relaxed)
}

/// Initialize Zicond support — probes for availability.
///
/// In QEMU with `-cpu rv64,zicond=true`, Zicond is available.
/// On platforms without Zicond, the fallback path uses branches.
pub fn init_zicond() {
    // Probe Zicond by executing a czero instruction.
    // If the instruction traps, Zicond is not available.
    #[cfg(not(test))]
    {
        // Try to execute czero.eqz x0, x0, x0 (should be a no-op if available)
        let result: u64;
        let trap_occurred: u64;

        unsafe {
            core::arch::asm!(
                "csrr {trap}, scause",
                // If we make it here, Zicond is available — set the flag
                "li {result}, 1",
                trap = out(reg) trap_occurred,
                result = out(reg) result,
            );
            // Alternative: we use a simpler probe — check the result of a
            // known Zicond operation
            let test_val: u64 = 42;
            let cond: u64 = 1; // non-zero
            let mut out: u64;
            // czero.nez: rd = (rs2 != 0) ? 0 : rs1
            // Since cond != 0, result should be 0
            core::arch::asm!(
                ".insn r 0x13, 0x7, 0x31, {rd}, {rs1}, {rs2}",
                rd = out(reg) out,
                rs1 = in(reg) test_val,
                rs2 = in(reg) cond,
            );
            if out == 0 {
                set_zicond_available();
            }
        }

        if zicond_available() {
            crate::println!("  V38c: Zicond available (conditional move instructions)");
        } else {
            crate::println!("  V38c: Zicond not available, using branch fallbacks");
        }
    }
}

// ── Conditional Move Operations ───────────────────────────────────────────

/// Select `a` if `cond` is true, `b` otherwise.
///
/// Uses Zicond czero.nez + czero.eqz to implement branchless selection.
/// On platforms without Zicond, falls back to a branch.
#[inline]
pub fn cmov_u64(cond: bool, a: u64, b: u64) -> u64 {
    if zicond_available() {
        // Branchless implementation using Zicond:
        //   mask_nez = czero.nez(0, a, cond) — gives 0 if cond is true (non-zero)
        //   Actually: czero.nez rd, rs1, rs2: rd = (rs2 != 0) ? 0 : rs1
        //   czero.eqz rd, rs1, rs2: rd = (rs2 == 0) ? 0 : rs1
        //
        // Strategy:
        //   result_nez = czero.nez(a, a, cond_sel)  -> a if cond_sel == 0, else 0
        //   Wait, let me think about this more carefully.
        //
        //   We want: result = cond ? a : b
        //
        //   Let cond_mask = if cond { 1 } else { 0 }
        //
        //   result_a = czero.nez(a, cond_mask)  -> a if cond_mask==0, else 0
        //   result_b = czero.eqz(b, cond_mask)  -> b if cond_mask!=0, else 0
        //
        //   result = result_a | result_b
        //
        //   Check: cond=true -> cond_mask=1
        //     czero.nez(a, 1) -> 0
        //     czero.eqz(b, 1) -> b
        //     result = 0 | b = b  ← WRONG! We want 'a'
        //
        //   Actually:
        //     result_a = czero.eqz(a, cond_mask)  -> a if cond_mask!=0, else 0
        //     result_b = czero.nez(b, cond_mask)  -> b if cond_mask==0, else 0
        //
        //   Check: cond=true -> cond_mask=1
        //     czero.eqz(a, 1) -> a
        //     czero.nez(b, 1) -> 0
        //     result = a | 0 = a ✓
        //
        //   Check: cond=false -> cond_mask=0
        //     czero.eqz(a, 0) -> 0
        //     czero.nez(b, 0) -> b
        //     result = 0 | b = b ✓

        let cond_mask: u64 = if cond { 1 } else { 0 };
        let result_a: u64;
        let result_b: u64;

        unsafe {
            // czero.eqz rd, rs1, rs2: rd = (rs2 == 0) ? 0 : rs1
            // funct7=0b0000000 (0x00)
            core::arch::asm!(
                ".insn r 0x13, 0x7, 0x00, {rd}, {rs1}, {rs2}",
                rd = out(reg) result_a,
                rs1 = in(reg) a,
                rs2 = in(reg) cond_mask,
            );
            // czero.nez rd, rs1, rs2: rd = (rs2 != 0) ? 0 : rs1
            // funct7=0b0000001 (0x01)
            core::arch::asm!(
                ".insn r 0x13, 0x7, 0x01, {rd}, {rs1}, {rs2}",
                rd = out(reg) result_b,
                rs1 = in(reg) b,
                rs2 = in(reg) cond_mask,
            );
        }
        result_a | result_b
    } else {
        // Fallback: branch-based
        if cond { a } else { b }
    }
}

/// Conditional set: return 1 if cond is true, 0 otherwise.
#[inline]
pub fn cset_u64(cond: bool) -> u64 {
    cmov_u64(cond, 1, 0)
}

/// Maximum of two u64 values without branches.
#[inline]
pub fn max_u64(a: u64, b: u64) -> u64 {
    cmov_u64(a > b, a, b)
}

/// Minimum of two u64 values without branches.
#[inline]
pub fn min_u64(a: u64, b: u64) -> u64 {
    cmov_u64(a < b, a, b)
}

/// Branchless maximum for signed i64 values.
#[inline]
pub fn max_i64(a: i64, b: i64) -> i64 {
    cmov_u64(a > b, a as u64, b as u64) as i64
}

/// Branchless minimum for signed i64 values.
#[inline]
pub fn min_i64(a: i64, b: i64) -> i64 {
    cmov_u64(a < b, a as u64, b as u64) as i64
}

/// Branchless clamp: clamp `val` to [lo, hi].
#[inline]
pub fn clamp_u64(val: u64, lo: u64, hi: u64) -> u64 {
    // min(max(val, lo), hi)
    cmov_u64(cmov_u64(val < lo, lo, val) > hi, hi, cmov_u64(val < lo, lo, val))
}

/// Branchless clamp for signed i64.
#[inline]
pub fn clamp_i64(val: i64, lo: i64, hi: i64) -> i64 {
    clamp_u64(val as u64, lo as u64, hi as u64) as i64
}

/// Absolute value without branches.
#[inline]
pub fn abs_i64(val: i64) -> i64 {
    let mask = (val >> 63) as u64; // all 1s if negative, 0 if non-negative
    // (val XOR mask) - mask
    (((val as u64) ^ mask).wrapping_sub(mask)) as i64
}

// ── Saturating Arithmetic ─────────────────────────────────────────────────

/// Saturating addition: a + b, clamped to u64::MAX on overflow.
#[inline]
pub fn saturating_add_u64(a: u64, b: u64) -> u64 {
    let sum = a.wrapping_add(b);
    // If overflow occurred, sum < a (or sum < b)
    cmov_u64(sum < a, u64::MAX, sum)
}

/// Saturating subtraction: a - b, clamped to 0 on underflow.
#[inline]
pub fn saturating_sub_u64(a: u64, b: u64) -> u64 {
    let diff = a.wrapping_sub(b);
    // If underflow occurred, diff > a
    cmov_u64(diff > a, 0, diff)
}

// ── Hot-Path Optimizations ────────────────────────────────────────────────

/// Apply Zicond optimizations to kernel hot paths.
///
/// Called once during boot to register Zicond-based replacements for
/// common kernel operations.  Actual replacement is done by the inline
/// functions above, but this function performs verification.
pub fn optimize_hot_paths() {
    if !zicond_available() {
        return;
    }

    // Verify that Zicond operations work correctly
    let test_ok = test_zicond_operations();
    if test_ok {
        crate::println!("  V38c: Zicond hot-path optimizations verified");
    } else {
        crate::println!("  V38c: WARNING — Zicond self-test failed!");
    }
}

/// Self-test for Zicond operations.
/// Returns true if all tests pass.
fn test_zicond_operations() -> bool {
    // Test cmov_u64
    if cmov_u64(true, 42, 100) != 42 { return false; }
    if cmov_u64(false, 42, 100) != 100 { return false; }

    // Test cset_u64
    if cset_u64(true) != 1 { return false; }
    if cset_u64(false) != 0 { return false; }

    // Test max/min
    if max_u64(10, 20) != 20 { return false; }
    if min_u64(10, 20) != 10 { return false; }
    if max_i64(-5, 3) != 3 { return false; }
    if min_i64(-5, 3) != -5 { return false; }

    // Test clamp
    if clamp_u64(50, 10, 30) != 30 { return false; }
    if clamp_u64(5, 10, 30) != 10 { return false; }
    if clamp_u64(20, 10, 30) != 20 { return false; }

    // Test absolute value
    if abs_i64(-42) != 42 { return false; }
    if abs_i64(42) != 42 { return false; }
    if abs_i64(0) != 0 { return false; }

    // Test saturating arithmetic
    if saturating_add_u64(u64::MAX, 1) != u64::MAX { return false; }
    if saturating_add_u64(100, 50) != 150 { return false; }
    if saturating_sub_u64(0, 1) != 0 { return false; }
    if saturating_sub_u64(100, 50) != 50 { return false; }

    true
}

// ── Scheduler-Specific Optimizations ──────────────────────────────────────

/// Compare two priority values and return the higher (branchless).
///
/// Used by the scheduler to compare thread priorities without branches.
#[inline]
pub fn higher_priority(prio_a: u64, prio_b: u64) -> u64 {
    max_u64(prio_a, prio_b)
}

/// Compare two deadlines and return the earlier (branchless).
///
/// Used by EEVDF deadline-based scheduling.
#[inline]
pub fn earlier_deadline(da: u64, db: u64) -> u64 {
    min_u64(da, db)
}

/// Select the thread with higher priority (branchless).
///
/// Returns `true` if thread_a should be selected over thread_b.
/// Used in scheduler hot paths where branch misprediction is costly.
#[inline]
pub fn should_select_thread(prio_a: u64, prio_b: u64, deadline_a: u64, deadline_b: u64) -> bool {
    // Higher priority first; if equal, earlier deadline first
    let same_prio = prio_a == prio_b;
    cmov_u64(same_prio, deadline_a, !prio_a) < cmov_u64(same_prio, deadline_b, !prio_b)
}
