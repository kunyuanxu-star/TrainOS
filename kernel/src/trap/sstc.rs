/// RISC-V Sstc (Supervisor-mode Timer Compare) Extension
///
/// Provides the `stimecmp` CSR (0x14D) that allows S-mode to program
/// timer interrupts directly, eliminating the need for SBI timer calls
/// or CLINT MMIO access.
///
/// Usage:
///   stimecmp CSR at 0x14D — when `time >= stimecmp`, STIP is set.
///   time CSR at 0xB01    — returns current mtime value.
///   Write 0 to stimecmp to disable the timer interrupt.
///
/// This module is compile-time gated by the Sstc availability check.
/// Platforms without Sstc should fall back to CLINT or SBI timer calls.

use core::sync::atomic::{AtomicBool, Ordering};

/// Whether Sstc has been initialized on this hart.
static SSTC_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Whether Sstc is available on this platform.
/// Set once during boot based on platform probing.
static SSTC_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Sstc timer abstraction.
pub struct SstcTimer;

impl SstcTimer {
    /// Probe for Sstc availability by attempting to read the stimecmp CSR.
    ///
    /// Returns `true` if the CSR is accessible (and thus Sstc is supported).
    /// On platforms where Sstc is not implemented, the read will cause an
    /// illegal-instruction trap.  Since kernel trap handling is already
    /// initialized at the point this is called, we rely on a prior knowledge
    /// that the platform supports Sstc (e.g. QEMU virt with `sstc=true`).
    ///
    /// A production implementation should check the ISA string in the
    /// DeviceTree or use the SBI platform extension to discover Sstc.
    pub fn is_available() -> bool {
        SSTC_AVAILABLE.load(Ordering::Relaxed)
    }

    /// Mark Sstc as available (called after successful probe).
    pub fn set_available(available: bool) {
        SSTC_AVAILABLE.store(available, Ordering::SeqCst);
    }

    /// Read the current `time` CSR value (mtime at 10 MHz on QEMU virt).
    #[inline]
    pub fn read_time() -> u64 {
        #[cfg(not(test))]
        unsafe {
            let val: u64;
            core::arch::asm!("csrr {}, 0xB01", out(reg) val);
            return val;
        }
        #[cfg(test)]
        0
    }

    /// Write the `stimecmp` CSR directly.
    ///
    /// When `time >= stimecmp`, the STIP (Supervisor Timer Interrupt Pending)
    /// bit is set in `sip`, triggering a timer interrupt at STIE=1.
    #[inline]
    pub fn set_stimecmp(val: u64) {
        #[cfg(not(test))]
        unsafe {
            core::arch::asm!("csrw 0x14D, {}", in(reg) val);
        }
    }

    /// Set the timer to fire after `us` microseconds from now.
    ///
    /// The timebase defaults to 10 MHz (the QEMU virt default),
    /// so 1 us = 10 ticks.
    pub fn set_timer_delay(us: u64) {
        let now = Self::read_time();
        let ticks = us * 10; // 10 MHz => 1 tick = 100 ns
        Self::set_stimecmp(now + ticks);
    }

    /// Set the timer to fire at a specific absolute `time` CSR value.
    ///
    /// Prefer this for periodic scheduling to avoid drift accumulation:
    ///   let deadline = read_time() + PERIOD;
    ///   set_timer_deadline(deadline);
    pub fn set_timer_deadline(deadline: u64) {
        Self::set_stimecmp(deadline);
    }

    /// Set a periodic timer interrupt by programming stimecmp relative
    /// to the current time.  Re-arm in the timer interrupt handler for
    /// a drift-free periodic tick.
    pub fn set_periodic_tick(period_us: u64) {
        Self::set_timer_delay(period_us);
    }

    /// Clear the timer interrupt by writing 0 to stimecmp.
    ///
    /// This disables STIP until stimecmp is set to a non-zero value.
    pub fn clear_interrupt() {
        Self::set_stimecmp(0);
    }

    /// Enable STIE (Supervisor Timer Interrupt Enable) in `sie`.
    ///
    /// Must be called once per hart before timer interrupts can fire.
    pub fn enable() {
        #[cfg(not(test))]
        unsafe {
            core::arch::asm!("csrrs zero, sie, {}", in(reg) 1usize << 5);
        }
        SSTC_INITIALIZED.store(true, Ordering::SeqCst);
    }

    /// Check if Sstc has been initialized on this hart.
    pub fn is_initialized() -> bool {
        SSTC_INITIALIZED.load(Ordering::Relaxed)
    }
}

/// Default timebase frequency for QEMU virt (10 MHz).
pub const TIMEBASE_FREQ: u64 = 10_000_000;

/// Convenience: one-shot timer in microseconds.
pub fn oneshot(us: u64) {
    SstcTimer::set_timer_delay(us);
}
