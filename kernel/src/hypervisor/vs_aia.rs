/// RISC-V VS-AIA — Virtual Supervisor-level AIA
///
/// Allows guest VMs to use AIA (IMSIC + APLIC) through the hypervisor.
/// Each VM gets virtual IMSIC interrupt files per hart and a virtual APLIC
/// for wired interrupt delivery.
///
/// VS-level CSRs that must be emulated:
///   vsieh  (VS-mode IMSIC interrupt enable, 0x9C0)
///   vsiph  (VS-mode IMSIC interrupt pending, 0x9C2)
///   hvien  (Hypervisor virtual interrupt enable, 0x608)
///   hvip   (Hypervisor virtual interrupt pending, 0x609)
///   hidelegh (Hypervisor IRQ delivery config, 0x613)
///
/// Interrupt types:
///   VS-level external (cause 10 in VS-mode) — from virtual IMSIC
///   VS-level timer    (cause 5 in VS-mode)  — time compare
///   VS-level software  (cause 1 in VS-mode) — SBI IPI
///
/// Integration with V23 hypervisor:
///   - Trap handler in HS-mode checks for VS-AIA
///   - On guest IMSIC MMIO access, route to virtual IMSIC
///   - On guest APLIC MMIO access, route to virtual APLIC

use core::sync::atomic::{AtomicU64, Ordering};

/// Maximum number of VMs supported by VS-AIA.
const MAX_VS_AIA_VMS: usize = 8;

/// Maximum number of harts per VM.
const MAX_HARTS_PER_VM: usize = 4;

/// Number of interrupt words (256 bits = 4 × u64).
const INTR_WORDS: usize = 4;

/// Default MSI priority for virtual interrupts.
const DEFAULT_MSI_PRIORITY: u8 = 1;

// ── Virtual IMSIC ─────────────────────────────────────────────────────────

/// Per-hart virtual IMSIC interrupt file for a single VM.
///
/// Mirrors the hardware IMSIC: each hart in a VM has its own pending/enable
/// state.  The hypervisor injects MSIs by setting pending bits, and the
/// guest claims/completes interrupts by reading/writing the virtual file.
#[derive(Clone, Copy, Debug)]
pub struct VirtualImsic {
    /// Virtual interrupt pending bits (256 bits, one per interrupt ID).
    pub pending_bits: [u64; INTR_WORDS],
    /// Virtual interrupt enable bits (set via vsieh CSR writes).
    pub enable_bits: [u64; INTR_WORDS],
    /// External interrupt pending (summarized EIP for the hart).
    pub eip: bool,
}

impl VirtualImsic {
    const fn empty() -> Self {
        VirtualImsic {
            pending_bits: [0; INTR_WORDS],
            enable_bits: [0; INTR_WORDS],
            eip: false,
        }
    }

    /// Set a pending bit for a specific interrupt ID.
    fn set_pending(&mut self, irq: u32) {
        let word = (irq as usize) / 64;
        let bit = (irq as usize) % 64;
        if word < INTR_WORDS {
            self.pending_bits[word] |= 1 << bit;
            self.eip = self.has_pending_enabled();
        }
    }

    /// Clear a pending bit for a specific interrupt ID.
    fn clear_pending(&mut self, irq: u32) {
        let word = (irq as usize) / 64;
        let bit = (irq as usize) % 64;
        if word < INTR_WORDS {
            self.pending_bits[word] &= !(1 << bit);
            self.eip = self.has_pending_enabled();
        }
    }

    /// Check if there is any enabled-and-pending interrupt.
    fn has_pending_enabled(&self) -> bool {
        for w in 0..INTR_WORDS {
            if (self.pending_bits[w] & self.enable_bits[w]) != 0 {
                return true;
            }
        }
        false
    }

    /// Find the highest-priority pending enabled interrupt ID.
    /// Returns 0 if none.  Priority is determined by interrupt ID
    /// (lower ID = higher priority in RISC-V AIA convention).
    fn claim(&self) -> u32 {
        for w in 0..INTR_WORDS {
            let masked = self.pending_bits[w] & self.enable_bits[w];
            if masked != 0 {
                // Find the lowest set bit (highest priority)
                let bit = masked.trailing_zeros();
                let irq = (w as u32) * 64 + bit;
                return irq;
            }
        }
        0
    }
}

// ── Virtual APLIC ─────────────────────────────────────────────────────────

/// A virtual APLIC source entry — describes one wired interrupt source
/// for a VM's virtual APLIC.
#[derive(Clone, Copy, Debug)]
struct VirtualAplicSource {
    /// Interrupt priority (0-255).
    priority: u8,
    /// Target hart for MSI delivery.
    target_hart: u32,
    /// Whether this source is enabled.
    enabled: bool,
}

impl VirtualAplicSource {
    const fn empty() -> Self {
        VirtualAplicSource {
            priority: 0,
            target_hart: 0,
            enabled: false,
        }
    }
}

/// Maximum interrupt sources per virtual APLIC.
const VIRTUAL_APLIC_SOURCES: usize = 64;

// ── VS-AIA Controller ─────────────────────────────────────────────────────

/// VS-AIA controller managing virtual IMSIC and virtual APLIC for all VMs.
pub struct VsAiaController {
    /// Per-VM, per-hart virtual IMSIC state.
    vm_imsic: [[VirtualImsic; MAX_HARTS_PER_VM]; MAX_VS_AIA_VMS],
    /// Per-VM virtual APLIC source configuration.
    vm_aplic_sources: [[VirtualAplicSource; VIRTUAL_APLIC_SOURCES]; MAX_VS_AIA_VMS],
    /// Per-VM active count (how many vcpus have been created).
    vm_active_vcpus: [u32; MAX_VS_AIA_VMS],
}

impl VsAiaController {
    /// Create a new VS-AIA controller with all state zeroed.
    pub const fn new() -> Self {
        // Build an empty 8×4 array of VirtualImsic
        const EMPTY_IMSIC: VirtualImsic = VirtualImsic::empty();
        const EMPTY_IMSIC_ROW: [VirtualImsic; MAX_HARTS_PER_VM] = [
            EMPTY_IMSIC, EMPTY_IMSIC, EMPTY_IMSIC, EMPTY_IMSIC,
        ];
        const EMPTY_APLIC_SRC: VirtualAplicSource = VirtualAplicSource::empty();
        const EMPTY_APLIC_ROW: [VirtualAplicSource; VIRTUAL_APLIC_SOURCES] = [
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
            EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC, EMPTY_APLIC_SRC,
        ];

        VsAiaController {
            vm_imsic: [
                EMPTY_IMSIC_ROW, EMPTY_IMSIC_ROW, EMPTY_IMSIC_ROW, EMPTY_IMSIC_ROW,
                EMPTY_IMSIC_ROW, EMPTY_IMSIC_ROW, EMPTY_IMSIC_ROW, EMPTY_IMSIC_ROW,
            ],
            vm_aplic_sources: [
                EMPTY_APLIC_ROW, EMPTY_APLIC_ROW, EMPTY_APLIC_ROW, EMPTY_APLIC_ROW,
                EMPTY_APLIC_ROW, EMPTY_APLIC_ROW, EMPTY_APLIC_ROW, EMPTY_APLIC_ROW,
            ],
            vm_active_vcpus: [0; MAX_VS_AIA_VMS],
        }
    }

    /// Initialize the VS-AIA controller.
    pub fn init() -> Self {
        crate::println!("  V38c: VS-AIA initialized (virtual AIA for hypervisor)");
        VsAiaController::new()
    }

    /// Check if VS-AIA is available.
    /// Requires both H-extension (hypervisor) and AIA (APLIC+IMSIC).
    pub fn available() -> bool {
        // AIA must be available (the host uses APLIC+IMSIC)
        crate::trap::aia::InterruptController::is_aia_active()
        // H-extension is implicitly available if the hypervisor module is compiled in
    }

    /// Initialize virtual IMSIC state for a VM's vCPU.
    pub fn vm_create(&mut self, vm_id: usize) {
        if vm_id >= MAX_VS_AIA_VMS {
            return;
        }
        for hart in 0..MAX_HARTS_PER_VM {
            self.vm_imsic[vm_id][hart] = VirtualImsic::empty();
        }
        for src in 0..VIRTUAL_APLIC_SOURCES {
            self.vm_aplic_sources[vm_id][src] = VirtualAplicSource::empty();
        }
        self.vm_active_vcpus[vm_id] = 0;
    }

    /// Register a vCPU for a VM (increment active hart count).
    pub fn vm_add_vcpu(&mut self, vm_id: usize) {
        if vm_id < MAX_VS_AIA_VMS {
            self.vm_active_vcpus[vm_id] =
                self.vm_active_vcpus[vm_id].saturating_add(1).min(MAX_HARTS_PER_VM as u32);
        }
    }

    /// Inject an MSI interrupt into a guest VM.
    ///
    /// `vm_id` — VM index (0..7).
    /// `hart_id` — target hart within the VM (0..3).
    /// `irq` — interrupt ID (1..255, 0 is reserved).
    pub fn inject_msi(&mut self, vm_id: usize, hart_id: usize, irq: u32) {
        if vm_id >= MAX_VS_AIA_VMS || hart_id >= MAX_HARTS_PER_VM || irq == 0 {
            return;
        }
        self.vm_imsic[vm_id][hart_id].set_pending(irq);
        // Record in hvip-style bits: summarize for hypervisor
    }

    /// Inject a wired interrupt from the virtual APLIC.
    ///
    /// The virtual APLIC translates a wired interrupt into an MSI directed
    /// at the target hart configured for this source.
    pub fn inject_wired(&mut self, vm_id: usize, irq: u32, _priority: u8) {
        if vm_id >= MAX_VS_AIA_VMS || irq as usize >= VIRTUAL_APLIC_SOURCES || irq == 0 {
            return;
        }
        let source = &self.vm_aplic_sources[vm_id][irq as usize];
        if !source.enabled {
            return;
        }
        let target_hart = source.target_hart as usize;
        if target_hart < MAX_HARTS_PER_VM {
            self.vm_imsic[vm_id][target_hart].set_pending(irq);
        }
    }

    /// Handle VS-level external interrupt (cause 10 in VS-mode).
    ///
    /// Returns the claimed interrupt ID for the given VM and hart.
    /// The hypervisor should return to the guest after handling.
    pub fn handle_vs_external_interrupt(&self, vm_id: usize, hart_id: usize) -> u32 {
        if vm_id >= MAX_VS_AIA_VMS || hart_id >= MAX_HARTS_PER_VM {
            return 0;
        }
        self.vm_imsic[vm_id][hart_id].claim()
    }

    /// Read VS-level IMSIC claim register for guest interrupt handling.
    ///
    /// Guest reads the virtual IMSIC file at claim time.  Returns the
    /// highest-priority pending enabled interrupt, and clears its pending bit.
    pub fn vs_imsic_claim(&mut self, vm_id: usize, hart_id: usize) -> u32 {
        if vm_id >= MAX_VS_AIA_VMS || hart_id >= MAX_HARTS_PER_VM {
            return 0;
        }
        let imsic = &mut self.vm_imsic[vm_id][hart_id];
        let irq = imsic.claim();
        if irq != 0 {
            imsic.clear_pending(irq);
        }
        irq
    }

    /// Write VS-level IMSIC complete register.
    ///
    /// Guest writes back the interrupt ID after handling it.
    /// This re-enables delivery of new MSIs for this source.
    pub fn vs_imsic_complete(&mut self, vm_id: usize, _hart_id: usize, _irq: u32) {
        // In a real IMSIC, writing the complete register re-enables the
        // interrupt for delivery.  In our virtual model, the pending bit
        // is already cleared at claim time, so this is a no-op.
        let _ = vm_id;
    }

    /// Set guest interrupt enable via vsieh CSR write emulation.
    ///
    /// `bits` contains the interrupt enable mask for the given hart.
    /// Word 0 corresponds to interrupt IDs 0-63.
    pub fn vs_set_ie(&mut self, vm_id: usize, hart_id: usize, word: usize, bits: u64) {
        if vm_id >= MAX_VS_AIA_VMS || hart_id >= MAX_HARTS_PER_VM || word >= INTR_WORDS {
            return;
        }
        self.vm_imsic[vm_id][hart_id].enable_bits[word] = bits;
    }

    /// Get virtual interrupt pending summary for hvip CSR read.
    ///
    /// Summarizes pending interrupts across all harts for a VM.
    /// Used by the hypervisor to inject virtual interrupts.
    pub fn get_hvip(&self, vm_id: usize) -> u64 {
        if vm_id >= MAX_VS_AIA_VMS {
            return 0;
        }
        let mut hvip: u64 = 0;
        for hart in 0..MAX_HARTS_PER_VM {
            if self.vm_imsic[vm_id][hart].eip {
                hvip |= 1 << hart; // Bit per hart: external interrupt pending
            }
        }
        hvip
    }

    /// Set the priority of a virtual APLIC source for a VM.
    pub fn set_virtual_source_priority(&mut self, vm_id: usize, source: u32, priority: u8) {
        if vm_id >= MAX_VS_AIA_VMS || source as usize >= VIRTUAL_APLIC_SOURCES {
            return;
        }
        self.vm_aplic_sources[vm_id][source as usize].priority = priority;
    }

    /// Set the target hart for a virtual APLIC source.
    pub fn set_virtual_source_target(&mut self, vm_id: usize, source: u32, hart: u32) {
        if vm_id >= MAX_VS_AIA_VMS || source as usize >= VIRTUAL_APLIC_SOURCES {
            return;
        }
        self.vm_aplic_sources[vm_id][source as usize].target_hart = hart;
        self.vm_aplic_sources[vm_id][source as usize].enabled = true;
    }

    /// Check if any guest VM has pending external interrupts.
    pub fn has_pending_guest_interrupts(&self, vm_id: usize) -> bool {
        if vm_id >= MAX_VS_AIA_VMS {
            return false;
        }
        for hart in 0..MAX_HARTS_PER_VM {
            if self.vm_imsic[vm_id][hart].eip {
                return true;
            }
        }
        false
    }
}

// ── Global VS-AIA controller ──────────────────────────────────────────────

use spin::Mutex;

/// Global VS-AIA controller instance.
static VS_AIA: Mutex<Option<VsAiaController>> = Mutex::new(None);

/// Initialize the global VS-AIA controller.
pub fn vs_aia_init() {
    let controller = VsAiaController::init();
    *VS_AIA.lock() = Some(controller);
}

/// Inject an MSI into a guest VM (global API).
pub fn vs_inject_msi(vm_id: usize, hart_id: usize, irq: u32) {
    if let Some(ref mut ctrl) = *VS_AIA.lock() {
        ctrl.inject_msi(vm_id, hart_id, irq);
    }
}

/// Inject a wired interrupt from virtual APLIC (global API).
pub fn vs_inject_wired(vm_id: usize, irq: u32, priority: u8) {
    if let Some(ref mut ctrl) = *VS_AIA.lock() {
        ctrl.inject_wired(vm_id, irq, priority);
    }
}

/// Claim the highest-pending guest interrupt for a VM's hart (global API).
pub fn vs_imsic_claim(vm_id: usize, hart_id: usize) -> u32 {
    if let Some(ref mut ctrl) = *VS_AIA.lock() {
        ctrl.vs_imsic_claim(vm_id, hart_id)
    } else {
        0
    }
}

/// Complete a guest interrupt (global API).
pub fn vs_imsic_complete(vm_id: usize, hart_id: usize, irq: u32) {
    if let Some(ref mut ctrl) = *VS_AIA.lock() {
        ctrl.vs_imsic_complete(vm_id, hart_id, irq);
    }
}

/// Initialize per-VM VS-AIA state (called when a VM is created).
pub fn vs_aia_vm_create(vm_id: usize) {
    if let Some(ref mut ctrl) = *VS_AIA.lock() {
        ctrl.vm_create(vm_id);
    }
}

/// Register a vCPU for a VM.
pub fn vs_aia_vm_add_vcpu(vm_id: usize) {
    if let Some(ref mut ctrl) = *VS_AIA.lock() {
        ctrl.vm_add_vcpu(vm_id);
    }
}

/// Check if any guest external interrupt is pending.
pub fn vs_aia_has_pending(vm_id: usize) -> bool {
    VS_AIA.lock().as_ref().map(|c| c.has_pending_guest_interrupts(vm_id)).unwrap_or(false)
}

// ── VS-AIA CSR Emulation ──────────────────────────────────────────────────

/// Emulate VS-AIA CSR reads from the hypervisor.
///
/// Called when a guest VM accesses VS-level AIA CSRs.
/// Returns the value that should be presented to the guest.
pub fn vs_aia_csr_read(vm_id: usize, csr: usize, hart_id: usize) -> u64 {
    match csr {
        // vsieh (0x9C0): VS-mode IMSIC Interrupt Enable
        0x9C0 => {
            if let Some(ref ctrl) = *VS_AIA.lock() {
                if vm_id < MAX_VS_AIA_VMS && hart_id < MAX_HARTS_PER_VM {
                    // Return word 0 of enable bits (IRQs 0-63)
                    ctrl.vm_imsic[vm_id][hart_id].enable_bits[0]
                } else {
                    0
                }
            } else {
                0
            }
        }
        // vsiph (0x9C2): VS-mode IMSIC Interrupt Pending
        0x9C2 => {
            if let Some(ref ctrl) = *VS_AIA.lock() {
                if vm_id < MAX_VS_AIA_VMS && hart_id < MAX_HARTS_PER_VM {
                    ctrl.vm_imsic[vm_id][hart_id].pending_bits[0]
                } else {
                    0
                }
            } else {
                0
            }
        }
        // hvien (0x608): Hypervisor Virtual Interrupt Enable
        0x608 => {
            // Bit mask of which interrupt types are enabled for virtualization
            // bit 0: VSEI (VS-level external interrupt) enable
            // bit 1: VSTI (VS-level timer interrupt) enable
            // bit 2: VSSI (VS-level software interrupt) enable
            0x7 // Enable all three by default
        }
        // hvip (0x609): Hypervisor Virtual Interrupt Pending
        0x609 => {
            ctrl::get_hvip(vm_id)
        }
        // hidelegh (0x613): Hypervisor IRQ Delivery Config (guest)
        0x613 => {
            // Bit mask of which interrupts to delegate to guest
            0 // No delegation by default (hypervisor handles all)
        }
        _ => 0,
    }
}

/// Emulate VS-AIA CSR writes from the hypervisor.
pub fn vs_aia_csr_write(vm_id: usize, csr: usize, val: u64, hart_id: usize) {
    match csr {
        // vsieh: Write interrupt enable bits
        0x9C0 => {
            vs_set_ie(vm_id, hart_id, 0, val);
        }
        // hvien: Enable/disable virtual interrupts
        0x608 => {
            let _ = val; // Currently we always enable all three
        }
        // hvip: Clear pending virtual interrupts
        0x609 => {
            let _ = val; // Handled by claim/complete path
        }
        // hidelegh: Configure IRQ delegation
        0x613 => {
            let _ = val; // Currently no delegation
        }
        _ => {}
    }
}

/// Set guest interrupt enable (global wrapper for vsieh write).
pub fn vs_set_ie(vm_id: usize, hart_id: usize, word: usize, bits: u64) {
    if let Some(ref mut ctrl) = *VS_AIA.lock() {
        ctrl.vs_set_ie(vm_id, hart_id, word, bits);
    }
}

mod ctrl {
    /// Get hvip value summarizing pending guest external interrupts.
    pub(super) fn get_hvip(vm_id: usize) -> u64 {
        super::VS_AIA.lock().as_ref()
            .map(|c| c.get_hvip(vm_id))
            .unwrap_or(0)
    }
}
