/// RISC-V Smstateen — State Enable Extension
///
/// Controls access to performance counters, debug triggers, and other
/// extension state from less privileged modes.
///
/// From S-mode we control the `sstateen0-3` CSRs (0x10C-0x10F), which
/// gate U-mode access to the corresponding states.  The machine-mode
/// `mstateen0-3` CSRs gate S-mode access and are configured by M-mode
/// firmware (RustSBI / OpenSBI); we probe their effective values
/// via `sstateen` which is a subset.
///
/// Reference: RISC-V Smstateen extension specification.

use core::sync::atomic::{AtomicBool, Ordering};

/// Whether Smstateen has been probed and is available.
static SMSTATEEN_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Cached S-mode state enable values.
static mut STATE_ENABLE: Option<StateEnable> = None;

// ── sstateen CSR addresses (S-mode accessible) ──────────────────────────

const CSR_SSTATEEN0: u16 = 0x10C;
const CSR_SSTATEEN1: u16 = 0x10D;
const CSR_SSTATEEN2: u16 = 0x10E;
const CSR_SSTATEEN3: u16 = 0x10F;

// ── State enable bit positions ──────────────────────────────────────────

/// FCSR / F/D extension state.
pub const STATEEN_FCSR: u64   = 1 << 0;
/// Integer register state.
pub const STATEEN_IR: u64     = 1 << 1;
/// Custom state.
pub const STATEEN_CS: u64     = 1 << 2;
/// AIA IMSIC.
pub const STATEEN_IMSIC: u64  = 1 << 4;
/// AIA extension.
pub const STATEEN_AIA: u64    = 1 << 57;
/// Zkr entropy source.
pub const STATEEN_ZKR: u64    = 1 << 58;
/// Hardware performance counters (HPC).
pub const STATEEN_HPC: u64    = 1 << 60;
/// Debug triggers (Sdtrig).
pub const STATEEN_DTRIG: u64  = 1 << 62;
/// Vector extension (VS).
pub const STATEEN_V: u64      = 1 << 63;

/// Cached sstateen CSR values.
///
/// Only the `sstateen0-3` fields are writable from S-mode;
/// `mstateen` and `hstateen` fields are retained for documentation
/// but their values are fetched from the corresponding S-mode CSRs.
pub struct StateEnable {
    pub sstateen0: u64,
    pub sstateen1: u64,
    pub sstateen2: u64,
    pub sstateen3: u64,
}

impl StateEnable {
    /// Read current sstateen CSR values.
    pub fn init() -> Self {
        let mut se = Self {
            sstateen0: 0,
            sstateen1: 0,
            sstateen2: 0,
            sstateen3: 0,
        };
        #[cfg(not(test))]
        {
            se.sstateen0 = Self::read_sstateen(0);
            se.sstateen1 = Self::read_sstateen(1);
            se.sstateen2 = Self::read_sstateen(2);
            se.sstateen3 = Self::read_sstateen(3);
            SMSTATEEN_AVAILABLE.store(true, Ordering::SeqCst);
            crate::println!("  Smstateen: state enable initialized (sstateen0=0x{:x})", se.sstateen0);
        }
        se
    }

    /// Check if Smstateen is available.
    pub fn available() -> bool {
        SMSTATEEN_AVAILABLE.load(Ordering::Relaxed)
    }

    // ── CSR accessors ───────────────────────────────────────────────────

    /// Read `sstateenN` (0x10C + N).
    #[inline]
    fn read_sstateen(idx: usize) -> u64 {
        #[cfg(not(test))]
        unsafe {
            match idx {
                0 => { let v: u64; core::arch::asm!("csrr {}, 0x10C", out(reg) v); v }
                1 => { let v: u64; core::arch::asm!("csrr {}, 0x10D", out(reg) v); v }
                2 => { let v: u64; core::arch::asm!("csrr {}, 0x10E", out(reg) v); v }
                3 => { let v: u64; core::arch::asm!("csrr {}, 0x10F", out(reg) v); v }
                _ => 0,
            }
        }
        #[cfg(test)]
        0
    }

    /// Write `sstateenN` (0x10C + N).
    #[inline]
    fn write_sstateen(idx: usize, val: u64) {
        #[cfg(not(test))]
        unsafe {
            match idx {
                0 => core::arch::asm!("csrw 0x10C, {}", in(reg) val),
                1 => core::arch::asm!("csrw 0x10D, {}", in(reg) val),
                2 => core::arch::asm!("csrw 0x10E, {}", in(reg) val),
                3 => core::arch::asm!("csrw 0x10F, {}", in(reg) val),
                _ => {}
            }
        }
    }

    // ── Enable helpers ──────────────────────────────────────────────────

    /// Enable U-mode access to hardware performance counters (HPC).
    ///
    /// Without this, U-mode `rdcycle` / `rdinstret` / `rdhpmcounter`
    /// instructions raise an illegal-instruction exception.
    pub fn enable_hpc(&mut self) {
        self.sstateen0 |= STATEEN_HPC;
        #[cfg(not(test))]
        Self::write_sstateen(0, self.sstateen0);
    }

    /// Enable U-mode access to debug triggers.
    pub fn enable_triggers(&mut self) {
        self.sstateen0 |= STATEEN_DTRIG;
        #[cfg(not(test))]
        Self::write_sstateen(0, self.sstateen0);
    }

    /// Enable U-mode access to Zkr entropy source.
    pub fn enable_zkr(&mut self) {
        self.sstateen0 |= STATEEN_ZKR;
        #[cfg(not(test))]
        Self::write_sstateen(0, self.sstateen0);
    }

    /// Enable U-mode access to vector extension state.
    pub fn enable_vector(&mut self) {
        self.sstateen0 |= STATEEN_V;
        #[cfg(not(test))]
        Self::write_sstateen(0, self.sstateen0);
    }

    /// Enable U-mode access to AIA IMSIC.
    pub fn enable_aia(&mut self) {
        self.sstateen0 |= STATEEN_AIA;
        #[cfg(not(test))]
        Self::write_sstateen(0, self.sstateen0);
    }

    /// Enable U-mode access to FCSR (F/D extension).
    pub fn enable_fcsr(&mut self) {
        self.sstateen0 |= STATEEN_FCSR;
        #[cfg(not(test))]
        Self::write_sstateen(0, self.sstateen0);
    }

    /// Apply current sstateen values to hardware CSRs.
    pub fn apply(&self) {
        #[cfg(not(test))]
        {
            Self::write_sstateen(0, self.sstateen0);
            Self::write_sstateen(1, self.sstateen1);
            Self::write_sstateen(2, self.sstateen2);
            Self::write_sstateen(3, self.sstateen3);
        }
    }

    /// Check whether a specific state bit is accessible from S-mode by
    /// inspecting the sstateen value.
    pub fn is_accessible(&self, bit: u64) -> bool {
        (self.sstateen0 & bit) != 0
    }

    /// Disable a specific state for U-mode (clear the bit in sstateen0).
    pub fn disable(&mut self, bit: u64) {
        self.sstateen0 &= !bit;
        #[cfg(not(test))]
        Self::write_sstateen(0, self.sstateen0);
    }
}

// ── Global accessors ────────────────────────────────────────────────────

/// Initialise the global state enable manager.
///
/// On detection, enables HPC and debug trigger access for S-mode
/// (these require the M-mode firmware to have set the corresponding
/// `mstateen` bits; we assume they are enabled).
pub fn init() {
    #[cfg(not(test))]
    unsafe {
        STATE_ENABLE = Some(StateEnable::init());

        // Enable HPC and Debug triggers for U-mode by default.
        if let Some(ref mut se) = STATE_ENABLE {
            se.enable_hpc();
            se.enable_triggers();
            crate::println!("  Smstateen: HPC + debug triggers enabled for U-mode");
        }
    }
}

/// Return a mutable reference to the global StateEnable.
pub fn state_enable() -> Option<&'static mut StateEnable> {
    #[cfg(not(test))]
    unsafe {
        STATE_ENABLE.as_mut()
    }
    #[cfg(test)]
    None
}
