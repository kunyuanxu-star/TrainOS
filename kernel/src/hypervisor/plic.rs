// V23: Virtual PLIC for guest VMs
//
// Implements a simple Platform-Level Interrupt Controller for up to 8
// virtual machines, each with up to 64 interrupt sources.  This is a
// minimal functional PLIC that supports:
//
//   - Interrupt injection (pending bit set)
//   - Interrupt enable/disable (per-VM mask)
//   - Claim (find the highest-priority pending + enabled interrupt)
//   - Complete (acknowledge interrupt processing)
//
// Priority levels and preemption are not modelled — all interrupts are
// treated as equal.  If multiple interrupts are pending concurrently,
// the one with the lowest IRQ number is claimed first.

/// Maximum number of interrupt sources per VM.
const MAX_VM_IRQS: usize = 64;

/// Per-VM virtual PLIC state.
#[derive(Clone, Copy)]
struct VmPlic {
    /// Bitmap of pending interrupts (bit N = IRQ N is pending).
    pending: u64,
    /// Bitmap of enabled interrupts (bit N = IRQ N is enabled).
    enabled: u64,
    /// Interrupt threshold (not yet used; all interrupts pass threshold 0).
    threshold: u32,
}

/// Constant empty PLIC instance for array initialisation.
const EMPTY_VM_PLIC: VmPlic = VmPlic {
    pending: 0,
    enabled: 0,
    threshold: 0,
};

/// Global table of virtual PLIC instances, one per VM slot.
static mut VM_PLICS: [VmPlic; 8] = [EMPTY_VM_PLIC; 8];

/// Reset a VM's PLIC state back to all disabled / no pending interrupts.
///
/// Should be called during VM creation or teardown.
pub fn reset(vm_idx: usize) {
    if vm_idx < 8 {
        unsafe {
            VM_PLICS[vm_idx] = EMPTY_VM_PLIC;
        }
    }
}

/// Inject an interrupt into a guest VM.
///
/// Sets the pending bit for `irq` on VM `vm_idx`.  The interrupt will be
/// delivered when the VM next claims an interrupt (assuming it is enabled).
pub fn inject(vm_idx: usize, irq: u32) {
    if vm_idx >= 8 || irq >= MAX_VM_IRQS as u32 {
        return;
    }
    unsafe {
        VM_PLICS[vm_idx].pending |= 1u64 << irq;
    }
}

/// Claim the highest-priority pending + enabled interrupt for a guest VM.
///
/// Returns the IRQ number, or 0 if no interrupt is pending and enabled.
/// After claiming, the pending bit for that IRQ is cleared.
pub fn claim(vm_idx: usize) -> u32 {
    if vm_idx >= 8 {
        return 0;
    }
    unsafe {
        let plic = &mut VM_PLICS[vm_idx];
        let candidates = plic.pending & plic.enabled;
        if candidates == 0 {
            return 0;
        }
        // RISC-V PLIC convention: IRQ 0 is reserved (no interrupt), so
        // trailing_zeros gives us the smallest-numbered pending IRQ.
        let irq = candidates.trailing_zeros();
        plic.pending &= !(1u64 << irq);
        irq
    }
}

/// Mark an interrupt as completed for a guest VM.
///
/// In this simple implementation no per-IRQ tracking state is maintained,
/// so the function is a no-op.  It exists for API completeness and to
/// match the PLIC specification's claim/complete handshake.
pub fn complete(_vm_idx: usize, _irq: u32) {
    // No per-IRQ state to update.
}

/// Set the interrupt enable mask for a guest VM.
///
/// Only interrupts whose bit is set in `mask` will be delivered to the VM.
pub fn set_enable(vm_idx: usize, mask: u64) {
    if vm_idx < 8 {
        unsafe {
            VM_PLICS[vm_idx].enabled = mask;
        }
    }
}
