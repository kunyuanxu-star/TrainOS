/// RISC-V Vector Extension (RVV 1.0) Support — V36a
///
/// Implements lazy vector context switching, kernel-accessible vector
/// operations, and vector capability enforcement.
///
/// Hardware assumption: VLEN = 256 (QEMU 8.0+ default for `-cpu rv64,v=true`).
/// Each of the 32 vector registers (v0–v31) holds VLEN/8 = 32 bytes,
/// for a total register file size of 1024 bytes.

use core::sync::atomic::{AtomicU64, Ordering};

// ─── Vector Statistics ──────────────────────────────────────────────────────

/// Global vector extension usage statistics.
pub static VECTOR_STATS: VectorStats = VectorStats::new();

pub struct VectorStats {
    /// Number of tasks that have used vector instructions since boot.
    pub vector_tasks: AtomicU64,
    /// Total number of vector state saves (context switch out).
    pub vector_saves: AtomicU64,
    /// Total number of vector state restores (context switch in).
    pub vector_restores: AtomicU64,
    /// Number of lazy-activation traps (first vector instruction).
    pub vector_lazy_traps: AtomicU64,
    /// Hardware vector length in bytes (read once at init).
    pub vlen: AtomicU64,
}

impl VectorStats {
    pub const fn new() -> Self {
        VectorStats {
            vector_tasks: AtomicU64::new(0),
            vector_saves: AtomicU64::new(0),
            vector_restores: AtomicU64::new(0),
            vector_lazy_traps: AtomicU64::new(0),
            vlen: AtomicU64::new(256),
        }
    }

    pub fn record_save(&self) {
        self.vector_saves.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_restore(&self) {
        self.vector_restores.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_lazy_trap(&self) {
        self.vector_lazy_traps.fetch_add(1, Ordering::Relaxed);
    }
}

// ─── Vector State (per-thread) ──────────────────────────────────────────────

/// Per-thread vector register file and CSR state.
///
/// Layout (must match assembly offsets in save/restore routines):
///   [0..1024)   v_regs — 32 × 32 bytes (VLEN=256)
///   [1024]      vcsr    — vector control / status
///   [1032]      vstart  — vector start position
///   [1040]      vxsat   — fixed-point saturation flag
///   [1048]      vxrm    — fixed-point rounding mode
///   [1056]      vtype   — vector type (vill=1 means illegal)
///   [1064]      vl      — vector length
///   [1072]      dirty   — lazy-save flag (Rust-managed)
#[repr(C)]
pub struct VectorState {
    pub v_regs: [u8; 1024],
    pub vcsr: usize,
    pub vstart: usize,
    pub vxsat: usize,
    pub vxrm: usize,
    pub vtype: usize,
    pub vl: usize,
    pub dirty: bool,
}

impl VectorState {
    /// Create a clean (non-dirty) vector state with all registers zeroed.
    pub fn new() -> Self {
        VectorState {
            v_regs: [0u8; 1024],
            vcsr: 0,
            vstart: 0,
            vxsat: 0,
            vxrm: 0,
            vtype: 0,
            vl: 0,
            dirty: false,
        }
    }

    /// Check whether the V extension is available on this hardware
    /// by reading the `misa` CSR (via SBI or direct S-mode read).
    ///
    /// In S-mode we cannot directly read `misa`; instead we probe by
    /// attempting to set the VS field in sstatus and checking for an
    /// illegal-instruction trap.  If VS can be written, V is available.
    pub fn is_available() -> bool {
        unsafe {
            let sstatus: usize;
            core::arch::asm!("csrr {}, sstatus", out(reg) sstatus);
            // Try writing the current sstatus back — if the V-extension
            // is not present the VS field is hard-wired to 0.
            core::arch::asm!("csrw sstatus, {}", in(reg) sstatus);
            // Read sstatus back and check if VS bits can be set
            let sstatus2: usize;
            core::arch::asm!("csrr {}, sstatus", out(reg) sstatus2);
            // If VS field is writable, V extension is enabled
            // The VS field is bits [10:9] of sstatus.
            // A simple probe: try to set VS=1 (Initial)
            let probe = (sstatus & !(3usize << 9)) | (1usize << 9);
            core::arch::asm!("csrw sstatus, {}", in(reg) probe);
            let sstatus3: usize;
            core::arch::asm!("csrr {}, sstatus", out(reg) sstatus3);
            // Restore original sstatus
            core::arch::asm!("csrw sstatus, {}", in(reg) sstatus);
            (sstatus3 >> 9) & 3 == 1
        }
    }

    /// Mark this state as dirty (called on first vector instruction trap).
    pub fn mark_dirty(&mut self) {
        if !self.dirty {
            self.dirty = true;
            // Reset the CSR fields so a subsequent restore is well-defined.
            self.vcsr = 0;
            self.vstart = 0;
            self.vxsat = 0;
            self.vxrm = 0;
            self.vtype = 0;
            self.vl = 0;
        }
    }

    /// Save the hardware vector registers into this state block.
    /// Must be called with VS != Off (i.e. vector instructions enabled).
    ///
    /// # Safety
    /// - Must be called on the current HART (saves the *hardware* state).
    /// - Vector state must be currently active (VS >= Initial).
    pub unsafe fn save(&mut self) {
        if !self.dirty {
            return;
        }
        vector_state_save(self as *mut Self);
        // After save, the saved copy reflects all CSRs and register values.
    }

    /// Restore the hardware vector registers from this state block.
    ///
    /// # Safety
    /// - Must be called on the HART that will run the target thread.
    /// - VS should be Initial before calling (already enabled via sstatus).
    pub unsafe fn restore(&self) {
        if !self.dirty {
            return;
        }
        vector_state_restore(self as *const Self);
    }
}

extern "C" {
    /// Assembly routine: save vector state to the given VectorState.
    /// a0 = *mut VectorState
    fn vector_state_save(state: *mut VectorState);

    /// Assembly routine: restore vector state from the given VectorState.
    /// a0 = *const VectorState
    fn vector_state_restore(state: *const VectorState);
}

// ─── Vector Save/Restore Assembly ───────────────────────────────────────────

core::arch::global_asm!(
    ".section .text.vector, \"ax\", @progbits",

    // ═════════════════════════════════════════════════════════════════════
    // void vector_state_save(VectorState *state)
    //
    // Save all 32 vector registers (v0-v31) and vector CSRs.
    // Uses vsetvli with e8,m1 so that vl = VLMAX = VLEN/8.
    // Saves original vtype/vl before reconfiguring, then restores
    // them to the saved copy so the state is complete.
    // ═════════════════════════════════════════════════════════════════════
    ".globl vector_state_save",
    ".align 2",
    "vector_state_save:",
    // a0 = state pointer
    "    mv      t2, a0",
    // --- Save original vtype and vl ---
    "    csrr    t0, vtype",
    "    csrr    t1, vl",
    "    sd      t0, 1056(a0)",   // state->vtype
    "    sd      t1, 1064(a0)",   // state->vl
    // --- Reconfigure for e8,m1 (vl = VLEN/8 = 32 for VLEN=256) ---
    "    vsetvli x0, x0, e8,m1,ta",
    // --- Save v0..v31 ---
    "    vse8.v  v0,  0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v1,  0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v2,  0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v3,  0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v4,  0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v5,  0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v6,  0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v7,  0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v8,  0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v9,  0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v10, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v11, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v12, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v13, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v14, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v15, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v16, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v17, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v18, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v19, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v20, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v21, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v22, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v23, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v24, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v25, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v26, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v27, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v28, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v29, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v30, 0(t2)",  "    addi    t2, t2, 32",
    "    vse8.v  v31, 0(t2)",
    // --- Save vector CSRs ---
    "    csrr    t0, vcsr",
    "    sd      t0, 1024(a0)",   // state->vcsr
    "    csrr    t0, vstart",
    "    sd      t0, 1032(a0)",   // state->vstart
    "    csrr    t0, vxsat",
    "    sd      t0, 1040(a0)",   // state->vxsat
    "    csrr    t0, vxrm",
    "    sd      t0, 1048(a0)",   // state->vxrm
    "    ret",

    // ═════════════════════════════════════════════════════════════════════
    // void vector_state_restore(const VectorState *state)
    //
    // Restore all 32 vector registers (v0-v31) and vector CSRs.
    // CSRs are restored first so vtype/vl take effect for subsequent
    // instructions (the vector-load instructions run with e8,m1, then
    // the original vtype/vl are restored at the end).
    // ═════════════════════════════════════════════════════════════════════
    ".globl vector_state_restore",
    ".align 2",
    "vector_state_restore:",
    // a0 = state pointer
    // --- Restore CSRs (before touching vtype) ---
    "    ld      t0, 1024(a0)",   // state->vcsr
    "    csrw    vcsr, t0",
    "    ld      t0, 1032(a0)",   // state->vstart
    "    csrw    vstart, t0",
    "    ld      t0, 1040(a0)",   // state->vxsat
    "    csrw    vxsat, t0",
    "    ld      t0, 1048(a0)",   // state->vxrm
    "    csrw    vxrm, t0",
    // --- Reconfigure for e8,m1 for restore ---
    "    vsetvli x0, x0, e8,m1,ta",
    // --- Restore v0..v31 ---
    "    mv      t2, a0",
    "    vle8.v  v0,  0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v1,  0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v2,  0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v3,  0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v4,  0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v5,  0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v6,  0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v7,  0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v8,  0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v9,  0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v10, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v11, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v12, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v13, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v14, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v15, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v16, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v17, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v18, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v19, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v20, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v21, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v22, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v23, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v24, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v25, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v26, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v27, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v28, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v29, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v30, 0(t2)",  "    addi    t2, t2, 32",
    "    vle8.v  v31, 0(t2)",
    // --- Restore original vtype and vl ---
    "    ld      t0, 1056(a0)",   // state->vtype
    "    ld      t1, 1064(a0)",   // state->vl
    // Check vtype.vill — if set, vtype is illegal; skip vsetvl
    "    li      t2, 1",
    "    slli    t2, t2, 63",     // bit 63 = vill
    "    and     t2, t0, t2",
    "    bnez    t2, 1f",
    "    vsetvl  x0, t1, t0",
    "1:",
    "    ret",
);

// ─── Enable / Disable VS in sstatus ─────────────────────────────────────────

/// Enable vector extension for the current HART by setting VS=Initial (01)
/// in sstatus.  After this call, vector instructions will not trap.
///
/// # Safety
/// Should only be called after saving any previous vector state.
pub unsafe fn enable_vs() {
    let sstatus: usize;
    core::arch::asm!("csrr {}, sstatus", out(reg) sstatus);
    let vs = (sstatus & !(3usize << 9)) | (1usize << 9); // VS=Initial
    core::arch::asm!("csrw sstatus, {}", in(reg) vs);
}

/// Disable vector extension for the current HART by setting VS=Off (00)
/// in sstatus.  After this call, vector instructions will trap.
///
/// # Safety
/// Should only be called when no thread needs vector access.
pub unsafe fn disable_vs() {
    let sstatus: usize;
    core::arch::asm!("csrr {}, sstatus", out(reg) sstatus);
    let vs = sstatus & !(3usize << 9); // VS=Off
    core::arch::asm!("csrw sstatus, {}", in(reg) vs);
}

// ─── Kernel-Accessible Vector Operations ────────────────────────────────────

/// Fast memcpy using vector loads/stores (up to VLEN bits per iteration).
///
/// Uses a strip-mining loop: each iteration copies up to VLEN/8 bytes
/// (32 bytes for VLEN=256).  Falls back to byte copy for any remainder.
///
/// # Safety
/// Same as `core::ptr::copy_nonoverlapping`: dst and src must be valid,
/// properly aligned, and non-overlapping.
pub unsafe fn vector_memcpy(dst: *mut u8, src: *const u8, count: usize) {
    if count == 0 || dst.is_null() || src.is_null() {
        return;
    }

    let old_sstatus: usize;
    core::arch::asm!("csrr {}, sstatus", out(reg) old_sstatus);
    // Enable VS if not already enabled
    let vs = (old_sstatus >> 9) & 3;
    if vs == 0 {
        enable_vs();
    }

    let mut offset = 0usize;
    // Strip-mining loop: at most VLEN/8 = 32 bytes per iteration
    while offset < count {
        let remaining = count - offset;
        // vsetvli rd, rs1, e8,m1,ta  →  sets vl = min(remaining, VLEN/8)
        let vl: usize;
        core::arch::asm!(
            "vsetvli {rd}, {rs1}, e8,m1,ta",
            rd = out(reg) vl,
            rs1 = in(reg) remaining,
        );

        // vle8.v and vse8.v using v0 as scratch
        if vl > 0 {
            core::arch::asm!(
                "vle8.v v0, ({src})",
                "vse8.v v0, ({dst})",
                src = in(reg) src.add(offset),
                dst = in(reg) dst.add(offset),
                options(nostack, preserves_flags),
            );
        }
        offset += vl;
    }

    // Restore original sstatus (may disable VS if it was off)
    core::arch::asm!("csrw sstatus, {}", in(reg) old_sstatus);
}

/// Fast memset using vector stores.
///
/// # Safety
/// Same as `core::ptr::write_bytes`: dst must be valid and properly aligned.
pub unsafe fn vector_memset(dst: *mut u8, val: u8, count: usize) {
    if count == 0 || dst.is_null() {
        return;
    }

    let old_sstatus: usize;
    core::arch::asm!("csrr {}, sstatus", out(reg) old_sstatus);
    let vs = (old_sstatus >> 9) & 3;
    if vs == 0 {
        enable_vs();
    }

    // Broadcast val to all elements of v0
    // vmvs.vx v0, val  — not a real instruction, use vmv.v.x
    // Actually: vmv.v.x vd, rs1 — broadcast rs1 to all elements of vd
    core::arch::asm!("vmv.v.x v0, {}", in(reg) val as usize);

    let mut offset = 0usize;
    while offset < count {
        let remaining = count - offset;
        let vl: usize;
        core::arch::asm!(
            "vsetvli {rd}, {rs1}, e8,m1,ta",
            rd = out(reg) vl,
            rs1 = in(reg) remaining,
        );
        if vl > 0 {
            core::arch::asm!(
                "vse8.v v0, ({dst})",
                dst = in(reg) dst.add(offset),
                options(nostack, preserves_flags),
            );
        }
        offset += vl;
    }

    core::arch::asm!("csrw sstatus, {}", in(reg) old_sstatus);
}

/// Internet checksum (16-bit one's complement sum).
///
/// Uses byte-by-byte accumulation (scalar fallback).  A future optimized
/// version can use vector reductions (vredsum.vs) once QEMU and toolchain
/// support for vector inline asm reductions is confirmed.
///
/// Returns the standard 16-bit one's complement checksum (inverted).
pub fn vector_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    for &b in data {
        sum += b as u32;
    }
    // Fold 32-bit sum to 16 bits (one's complement)
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// Vector XOR for RAID / parity operations.
///
/// Computes `dst[i] ^= src[i]` for all i in 0..len using vector instructions.
///
/// # Safety
/// dst and src must be valid, non-overlapping slices of at least `len` bytes.
pub unsafe fn vector_xor(dst: &mut [u8], src: &[u8]) {
    let len = dst.len().min(src.len());
    if len == 0 {
        return;
    }

    let old_sstatus: usize;
    core::arch::asm!("csrr {}, sstatus", out(reg) old_sstatus);
    let vs = (old_sstatus >> 9) & 3;
    if vs == 0 {
        enable_vs();
    }

    let mut offset = 0usize;
    while offset < len {
        let remaining = len - offset;
        let vl: usize;
        core::arch::asm!(
            "vsetvli {rd}, {rs1}, e8,m1,ta",
            rd = out(reg) vl,
            rs1 = in(reg) remaining,
        );
        if vl > 0 {
            core::arch::asm!(
                "vle8.v v0, ({src})",
                "vle8.v v1, ({dst})",
                "vxor.vv v0, v0, v1",
                "vse8.v v0, ({dst})",
                src = in(reg) src.as_ptr().add(offset),
                dst = in(reg) dst.as_mut_ptr().add(offset),
                options(nostack, preserves_flags),
            );
        }
        offset += vl;
    }

    core::arch::asm!("csrw sstatus, {}", in(reg) old_sstatus);
}

// ─── Vector Initialization ──────────────────────────────────────────────────

/// Detect VLEN and set initial stats.  Called once at boot.
pub fn init_vector_support() {
    unsafe {
        if VectorState::is_available() {
            let vlenb: usize;
            // vlenb CSR (0xC22) returns VLEN/8
            core::arch::asm!("csrr {}, vlenb", out(reg) vlenb);
            VECTOR_STATS.vlen.store(vlenb as u64, Ordering::Relaxed);
            crate::println!("  RVV 1.0: vlen={} bits ({} bytes/reg, {} regs)",
                vlenb * 8, vlenb, 32);
        } else {
            crate::println!("  RVV 1.0: not available");
        }
    }
}
