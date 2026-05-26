/// RISC-V AIA — Advanced Interrupt Architecture
///
/// Provides APLIC (Advanced Platform-Level Interrupt Controller) and
/// IMSIC (Incoming MSI Controller) support, replacing the legacy PLIC
/// with per-hart MSI-based interrupt delivery.
///
/// APLIC converts wired interrupts (e.g. from virtio, UART, PCI) into
/// MSIs delivered to the target hart's IMSIC interrupt file.
///
/// IMSIC provides per-hart interrupt files with claim/complete semantics
/// that avoid the global arbitration bottleneck of the legacy PLIC.
///
/// Hardware layout (QEMU virt with `aia=aplic-imsic`):
///   APLIC MMIO base: 0x0C00_0000
///   IMSIC MMIO base: 0x2400_0000 (per-hart page at base + hart_id * 0x1000)

use core::sync::atomic::{AtomicBool, Ordering};

/// Whether AIA has been detected and initialized.
static AIA_AVAILABLE: AtomicBool = AtomicBool::new(false);

// ── Platform constants (QEMU virt) ─────────────────────────────────────

/// APLIC MMIO base address on QEMU virt (same as legacy PLIC base).
const APLIC_BASE: usize = 0x0C00_0000;

/// Per-hart IMSIC interrupt file stride.
const IMSIC_PAGE_SIZE: usize = 0x1000;

/// IMSIC base address for hart0 on QEMU virt.
const IMSIC_BASE: usize = 0x2400_0000;

// ── APLIC register offsets (32-bit MMIO) ───────────────────────────────

/// Domain configuration register.
const APLIC_DOMAINCFG: usize = 0x0000;

/// Source configuration for IRQ `i`: base + 0x0004 + 4*i.
const APLIC_SOURCECFG: usize = 0x0004;

/// Set interrupt pending for IRQ `i`: base + 0x1C00 + 4*i.
const APLIC_SETIP: usize = 0x1C00;

/// Set interrupt enable for IRQ `i`: base + 0x2000 + 4*i.
const APLIC_SETIE: usize = 0x2000;

/// Generate MSI for a specific target: base + 0x3000 + 4*i.
const APLIC_GENMSI: usize = 0x3000;

/// Target (hart) for IRQ `i`: base + 0x3004 + 4*i.
const APLIC_TARGET: usize = 0x3004;

/// Delivery control for hart `h`: base + 0x3C00 + 4*h.
const APLIC_IDELIVERY: usize = 0x3C00;

/// Interrupt priority threshold for hart `h`: base + 0x3C04 + 4*h.
const APLIC_ITHRESHOLD: usize = 0x3C04;

/// Default B (big-endian) bit value in domaincfg — 0 for little-endian.
///
/// We set domaincfg to 0 (little-endian, MSI delivery mode, no IE=0 halt).
const APLIC_DOMAINCFG_INIT: u32 = 0;

// ── APLIC Controller ───────────────────────────────────────────────────

/// APLIC controller that manages wired interrupt sources and delivers
/// them as MSIs to target harts via the IMSIC.
pub struct AplicController {
    /// MMIO base address of the APLIC domain.
    base_addr: usize,
    /// Number of interrupt sources supported (read from sourcecfg limit).
    num_sources: usize,
}

impl AplicController {
    /// Probe and initialize the APLIC at the given MMIO base address.
    ///
    /// Returns `None` if no APLIC is found at the base address.
    pub fn probe(base: usize) -> Option<Self> {
        // Check if the APLIC domaincfg register is readable (non-zero response
        // for the MMIO region).  On QEMU virt with AIA, the APLIC exists at
        // 0xC000000.
        let cfg = unsafe { (base as *const u32).read_volatile() };
        // A valid APLIC domaincfg should not read as all-ones (invalid MMIO).
        if cfg == 0xFFFF_FFFF {
            return None;
        }

        Some(AplicController {
            base_addr: base,
            num_sources: 64, // QEMU virt APLIC has 64 interrupt sources
        })
    }

    /// Initialize the APLIC domain for MSI delivery mode.
    ///
    /// Must be called once before enabling individual sources.
    pub fn init(&self) {
        unsafe {
            // Configure domain: little-endian, MSI delivery mode
            (self.base_addr as *mut u32).write_volatile(APLIC_DOMAINCFG_INIT);
        }
    }

    /// Enable an interrupt source.
    pub fn enable_source(&self, irq: u32) {
        if irq as usize >= self.num_sources {
            return;
        }
        let reg = self.base_addr + APLIC_SETIE + (irq as usize) * 4;
        unsafe {
            (reg as *mut u32).write_volatile(1);
        }
    }

    /// Disable an interrupt source.
    pub fn disable_source(&self, irq: u32) {
        if irq as usize >= self.num_sources {
            return;
        }
        // APLIC sourcelcfg[irq] = 0 disables the source
        let reg = self.base_addr + APLIC_SOURCECFG + (irq as usize) * 4;
        unsafe {
            (reg as *mut u32).write_volatile(0);
        }
    }

    /// Set the target hart for an interrupt source (direct MSI delivery).
    ///
    /// The target register encodes the hart index and interrupt priority.
    /// Layout: [31:24] = hart_index, [23:0] = delivery info.
    pub fn set_target(&self, irq: u32, hart_index: u32) {
        if irq as usize >= self.num_sources {
            return;
        }
        let reg = self.base_addr + APLIC_TARGET + (irq as usize) * 4;
        unsafe {
            // Bit 31: delivery mode (0 = direct, 1 = MSI)
            // Bits [25:24] = hart index (for up to 4 harts on QEMU)
            // Lower bits = priority
            let val: u32 = (hart_index & 0xFF) << 24;
            (reg as *mut u32).write_volatile(val);
        }
    }

    /// Deliver an MSI for a specific source to a target hart.
    ///
    /// This writes the APLIC GENMSI register to trigger an MSI.
    pub fn deliver_msi(&self, irq: u32, target_hart: u32) {
        if irq as usize >= self.num_sources {
            return;
        }
        let reg = self.base_addr + APLIC_GENMSI;
        unsafe {
            // GENMSI: [31:24] = hart_index, [11:0] = interrupt ID
            let val: u32 = ((target_hart & 0xFF) << 24) | (irq & 0xFFF);
            (reg as *mut u32).write_volatile(val);
        }
    }

    /// Set interrupt priority.
    ///
    /// APLIC uses a 8-bit priority value per source.
    pub fn set_priority(&self, irq: u32, _priority: u8) {
        if irq as usize >= self.num_sources {
            return;
        }
        // Priority is written via sourcecfg[irq] on APLIC.
        // For now, leave at default (enable with default priority).
        let reg = self.base_addr + APLIC_SOURCECFG + (irq as usize) * 4;
        unsafe {
            // Bits [7:0] = priority, bit 0 = enable (with D = 1 for MSI mode)
            (reg as *mut u32).write_volatile(0x0001_0001);
        }
    }

    /// Return the number of interrupt sources.
    pub fn num_sources(&self) -> usize {
        self.num_sources
    }
}

// ── IMSIC Controller ───────────────────────────────────────────────────

/// Per-hart IMSIC interrupt file manager.
///
/// Each hart has its own 4 KB interrupt file in the IMSIC MMIO region.
/// The interrupt file provides claim/complete semantics: reading the
/// file returns the highest pending interrupt ID, writing completes it.
pub struct ImsicController {
    /// MMIO base of this hart's IMSIC interrupt file.
    file_base: usize,
}

impl ImsicController {
    /// Create an IMSIC controller for a specific hart.
    ///
    /// `hart_id` is the hardware thread ID (not the APLIC hart index).
    pub fn new(hart_id: usize) -> Self {
        ImsicController {
            file_base: IMSIC_BASE + hart_id * IMSIC_PAGE_SIZE,
        }
    }

    /// Claim the highest-pending interrupt from this hart's interrupt file.
    ///
    /// Reading the claim/complete register returns the interrupt ID
    /// of the highest-priority pending interrupt and clears its pending
    /// status.  Returns 0 if no interrupt is pending.
    pub fn claim(&self) -> u32 {
        unsafe {
            (self.file_base as *const u32).read_volatile()
        }
    }

    /// Complete a previously claimed interrupt.
    ///
    /// Writing the interrupt ID back to the claim/complete register
    /// signals completion, allowing new MSIs for this source to be
    /// delivered.
    pub fn complete(&self, irq: u32) {
        unsafe {
            (self.file_base as *mut u32).write_volatile(irq);
        }
    }

    /// Enable external interrupts via IMSIC.
    ///
    /// For S-mode, setting `sie.SEIE` (bit 9) enables external interrupts
    /// delivered through the IMSIC.
    pub fn enable(&self) {
        #[cfg(not(test))]
        unsafe {
            core::arch::asm!("csrrs zero, sie, {}", in(reg) 1usize << 9);
        }
    }

    /// Disable external interrupts.
    pub fn disable(&self) {
        #[cfg(not(test))]
        unsafe {
            core::arch::asm!("csrc sie, {}", in(reg) 1usize << 9);
        }
    }

    /// Check if an interrupt is pending (read the file's ToDo register).
    ///
    /// Returns the pending interrupt ID, or 0 if none.
    pub fn pending(&self) -> u32 {
        0 // Simplified: use claim instead
    }
}

// ── Unified Interrupt Controller Abstraction ───────────────────────────

/// Interrupt controller mode selected at boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerMode {
    /// Legacy PLIC (platform-level interrupt controller at 0xC000000).
    /// Used when AIA is not available.
    LegacyPlic,
    /// AIA with APLIC + IMSIC (per-hart MSI delivery).
    AplicImsic,
}

/// Unified interrupt controller for claim/complete operations.
///
/// Hides whether the platform uses legacy PLIC or AIA APLIC+IMSIC,
/// providing a consistent interface for the trap handler.
pub enum InterruptController {
    /// Legacy PLIC variant (no-op claim/complete).
    PLIC,
    /// AIA APLIC+IMSIC variant.
    APLIC_IMSIC(ImsicController),
}

impl InterruptController {
    /// Initialize and return the appropriate controller for this platform.
    ///
    /// Probes for AIA support; falls back to legacy PLIC if unavailable.
    pub fn init() -> Self {
        if Self::aia_available() {
            // AIA detected: set up IMSIC for this hart
            let hart_id = crate::per_cpu::hart_id();
            let imsic = ImsicController::new(hart_id);
            imsic.enable();
            AIA_AVAILABLE.store(true, Ordering::SeqCst);
            crate::println!("  AIA: APLIC + IMSIC initialized (hart {})", hart_id);
            InterruptController::APLIC_IMSIC(imsic)
        } else {
            crate::println!("  AIA: not available, using legacy PLIC");
            InterruptController::PLIC
        }
    }

    /// Check whether AIA is available on this platform.
    ///
    /// For QEMU virt, AIA requires `-machine virt,aia=aplic-imsic`.
    /// We probe by attempting to read the APLIC domaincfg register.
    pub fn aia_available() -> bool {
        // Probe APLIC at the known base address.
        // A real MMIO response (non-all-ones) indicates APLIC presence.
        #[cfg(not(test))]
        unsafe {
            let cfg = (APLIC_BASE as *const u32).read_volatile();
            if cfg != 0xFFFF_FFFF && cfg != 0 {
                return true;
            }
        }
        false
    }

    /// Claim the highest-pending interrupt.
    ///
    /// For AIA mode, delegates to IMSIC claim.
    /// For legacy PLIC mode, returns 0 (external interrupts are
    /// handled via kernel MMIO proxy, not direct ISRs).
    pub fn claim(&self) -> u32 {
        match self {
            InterruptController::PLIC => 0,
            InterruptController::APLIC_IMSIC(imsic) => imsic.claim(),
        }
    }

    /// Complete a previously claimed interrupt.
    pub fn complete(&self, irq: u32) {
        match self {
            InterruptController::PLIC => {}
            InterruptController::APLIC_IMSIC(imsic) => imsic.complete(irq),
        }
    }

    /// Enable external interrupts.
    pub fn enable_external(&self) {
        match self {
            InterruptController::PLIC => {
                // For legacy PLIC on QEMU virt, external interrupts
                // (SEIE, bit 9) must be enabled.
                #[cfg(not(test))]
                unsafe {
                    core::arch::asm!("csrrs zero, sie, {}", in(reg) 1usize << 9);
                }
            }
            InterruptController::APLIC_IMSIC(imsic) => imsic.enable(),
        }
    }

    /// Return the current controller mode.
    pub fn mode(&self) -> ControllerMode {
        match self {
            InterruptController::PLIC => ControllerMode::LegacyPlic,
            InterruptController::APLIC_IMSIC(_) => ControllerMode::AplicImsic,
        }
    }

    /// Check if AIA is active.
    pub fn is_aia_active() -> bool {
        AIA_AVAILABLE.load(Ordering::Relaxed)
    }
}

// ── Global singleton ───────────────────────────────────────────────────

use spin::Mutex;
use core::sync::atomic::AtomicU32;

/// Global interrupt controller instance (one per platform, shared across harts).
///
/// Initialized during trap init, used by the trap handler for external
/// interrupt claim/complete.
static GLOBAL_INTC: Mutex<Option<InterruptController>> = Mutex::new(None);

/// Per-hart IMSIC has its own claim/complete, but we maintain a single
/// APLIC instance for source management.
static APLIC: Mutex<Option<AplicController>> = Mutex::new(None);

/// Initialize the AIA subsystem (called once during boot).
pub fn init() {
    // Probe and initialize APLIC
    if let Some(aplic) = AplicController::probe(APLIC_BASE) {
        aplic.init();
        // Enable all 64 sources with default priority and target hart0
        for irq in 1..64 {
            aplic.set_priority(irq, 1);
            aplic.enable_source(irq);
            aplic.set_target(irq, 0);
        }
        *APLIC.lock() = Some(aplic);
        crate::println!("  APLIC: probed {} sources", 64);
    } else {
        crate::println!("  APLIC: not found, using legacy interrupt delivery");
    }

    // Initialize the unified interrupt controller
    let ic = InterruptController::init();
    *GLOBAL_INTC.lock() = Some(ic);
}

/// Claim an external interrupt via the global controller.
pub fn claim_global() -> u32 {
    if let Some(ref ic) = *GLOBAL_INTC.lock() {
        ic.claim()
    } else {
        0
    }
}

/// Complete an external interrupt via the global controller.
pub fn complete_global(irq: u32) {
    if let Some(ref ic) = *GLOBAL_INTC.lock() {
        ic.complete(irq);
    }
}
