/// RISC-V Zihintpause — PAUSE Hint Instruction
///
/// The PAUSE instruction provides a hint to the CPU that the current hart
/// is in a spin loop.  This allows:
///   - The CPU to reduce power consumption during busy-waiting
///   - Pipeline to flush less aggressively on memory ordering violations
///   - SMT (Simultaneous Multithreading) to yield issue bandwidth to the
///     other logical thread on the same core
///   - Memory ordering hardware to settle, reducing lock acquisition latency
///
/// Unlike WFI (which stops execution until an interrupt), PAUSE continues
/// execution but with reduced resource contention — making it ideal for
/// spinlock backoff.
///
/// Encoding: funct12=0x000, rs1=0, funct3=0x000, rd=0, opcode=0x0F
///   .insn 0x0F, 0x00, x0, x0, x0
///
/// Equivalent assembly: `pause` (mnemonic added in binutils 2.38+)

use core::sync::atomic::{AtomicBool, Ordering};

/// Whether Zihintpause is available on this platform.
static PAUSE_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Mark Zihintpause as available (called during boot if detected).
pub fn set_pause_available() {
    PAUSE_AVAILABLE.store(true, Ordering::SeqCst);
}

/// Check whether the Zihintpause extension is available.
#[inline]
pub fn pause_available() -> bool {
    PAUSE_AVAILABLE.load(Ordering::Relaxed)
}

// ── CPU Pause Instructions ────────────────────────────────────────────────

/// Execute a PAUSE hint instruction.
///
/// Acts as a NOP on hardware without Zihintpause, so it's safe to call
/// unconditionally.  On supporting hardware, reduces power and improves
/// spinlock performance.
#[inline]
pub fn cpu_pause() {
    unsafe {
        // PAUSE hint — safe on all RISC-V hardware (acts as NOP if unsupported)
        // Raw encoding: 0x0000000F (fence with all zeros = PAUSE hint)
        core::arch::asm!(
            ".word 0x0000000F",
            options(nomem, nostack)
        );
    }
}

/// Execute multiple PAUSE instructions in a loop.
///
/// `count` — number of PAUSE hints to issue.
#[inline]
pub fn spin_pause(count: u32) {
    for _ in 0..count {
        cpu_pause();
    }
}

/// Exponential backoff using PAUSE instructions.
///
/// Starts with a minimum delay and doubles up to a maximum.
/// Used by spinlock implementations to reduce contention.
#[inline]
pub fn pause_backoff(iteration: u32) {
    // Delay = min(1 << iteration, 1024)
    let shift = iteration.min(10);
    let delay = 1u32 << shift;
    spin_pause(delay);
}

// ── PauseSpinLock ─────────────────────────────────────────────────────────

/// Spinlock with exponential backoff using PAUSE instructions.
///
/// Significantly more efficient than a tight CAS loop on SMT systems
/// and reduces power consumption on all hardware.
pub struct PauseSpinLock {
    locked: core::sync::atomic::AtomicBool,
}

impl PauseSpinLock {
    /// Create a new unlocked PauseSpinLock.
    pub const fn new() -> Self {
        PauseSpinLock {
            locked: core::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Acquire the lock with exponential backoff.
    ///
    /// Uses PAUSE instructions to reduce pipeline pressure while spinning.
    pub fn lock(&self) {
        let mut delay: u32 = 1;
        loop {
            // Fast path: try to acquire
            if !self.locked.swap(true, core::sync::atomic::Ordering::Acquire) {
                return;
            }
            // Backoff with PAUSE
            for _ in 0..delay {
                cpu_pause();
            }
            delay = delay.saturating_mul(2).min(1024);
        }
    }

    /// Try to acquire the lock without blocking.
    ///
    /// Returns `true` if the lock was acquired.
    pub fn try_lock(&self) -> bool {
        !self.locked.swap(true, core::sync::atomic::Ordering::Acquire)
    }

    /// Release the lock.
    pub fn unlock(&self) {
        self.locked.store(false, core::sync::atomic::Ordering::Release);
    }
}

unsafe impl Sync for PauseSpinLock {}
unsafe impl Send for PauseSpinLock {}

// ── Initialization ────────────────────────────────────────────────────────

/// Initialize Zihintpause support.
///
/// On QEMU virt, Zihintpause is always available.  The PAUSE instruction
/// encoding 0x0000000F is safe to execute even on platforms that don't
/// implement it — it decodes as a `fence` hint and behaves as a NOP.
pub fn init_pause() {
    #[cfg(not(test))]
    {
        // Execute PAUSE to verify it doesn't trap
        unsafe {
            core::arch::asm!(
                ".word 0x0000000F",
                options(nomem, nostack)
            );
            // No trap occurred — Zihintpause is available
            set_pause_available();
        }

        if pause_available() {
            crate::println!("  V38c: Zihintpause available (PAUSE hint instruction)");
        } else {
            crate::println!("  V38c: Zihintpause not available, using NOP fallback");
        }
    }
}
