// V23: Paravirtual timer for guest VMs
//
// Provides a virtualized timer interface for up to 8 guest VMs.  Each VM
// has a time offset (captured at VM initialization) and a compare register.
// When the virtual time reaches or exceeds the compare value, the hypervisor
// injects a timer interrupt into the guest.
//
// The virtual time is derived from the host tick count
// (`crate::trap::TICK_COUNT`) minus a per-VM offset, so guests observe time
// advancing at the same rate as the host.

/// Per-VM offset subtracted from the host tick count to produce virtual time.
static mut VM_TIMER_OFFSETS: [u64; 8] = [0u64; 8];

/// Per-VM compare register.  When `virtual_time <= VM_TIMER_CMP[i]` the
/// hypervisor should inject a timer interrupt into VM `i`.  Initialized to
/// `u64::MAX` so that no interrupt fires before the guest sets a compare
/// value.
static mut VM_TIMER_CMP: [u64; 8] = [u64::MAX; 8];

/// Initialise the paravirtual timer for a guest VM.
///
/// This MUST be called once during VM creation (before the VM starts) to
/// capture the host tick offset that will be used to compute the guest's
/// virtual time.
///
/// # Panics
///
/// Panics if `vm_idx >= 8`.
pub fn init_vm_timer(vm_idx: usize) {
    assert!(vm_idx < 8, "VM timer index out of bounds");
    unsafe {
        VM_TIMER_OFFSETS[vm_idx] = crate::trap::TICK_COUNT as u64;
        VM_TIMER_CMP[vm_idx] = u64::MAX;
    }
}

/// Read the current virtual time for the given VM.
///
/// The virtual time is the host tick count minus a per-VM offset captured
/// at VM creation time.  Returns 0 if `vm_idx` is out of bounds.
pub fn read_time(vm_idx: usize) -> u64 {
    if vm_idx >= 8 {
        return 0;
    }
    let host_ticks = unsafe { crate::trap::TICK_COUNT as u64 };
    let offset = unsafe { VM_TIMER_OFFSETS[vm_idx] };
    host_ticks.wrapping_sub(offset)
}

/// Set the timer compare register for a guest VM.
///
/// If the compare value is <= the current virtual time, the timer interrupt
/// is injected immediately.  Otherwise the interrupt will fire once the
/// virtual time reaches the compare value (detected by `check_timers`).
///
/// # Panics
///
/// Panics if `vm_idx >= 8`.
pub fn set_timer_cmp(vm_idx: usize, cmp: u64) {
    assert!(vm_idx < 8, "VM timer index out of bounds");
    unsafe {
        VM_TIMER_CMP[vm_idx] = cmp;
    }
    let now = read_time(vm_idx);
    if cmp <= now {
        crate::println!(
            "PV timer: injecting interrupt to VM {} (cmp={}, now={})",
            vm_idx,
            cmp,
            now,
        );
        unsafe {
            VM_TIMER_CMP[vm_idx] = u64::MAX;
        }
    }
}

/// Check all VM timers and inject interrupts where the compare value has
/// been reached or exceeded.
///
/// This function should be called periodically from the host timer interrupt
/// handler (or from a dedicated hypervisor scheduling tick).
pub fn check_timers() {
    for i in 0..8 {
        let now = read_time(i);
        let cmp = unsafe { VM_TIMER_CMP[i] };
        if cmp <= now && cmp != u64::MAX {
            crate::println!(
                "PV timer: injecting interrupt to VM {} (cmp={}, now={})",
                i,
                cmp,
                now,
            );
            unsafe {
                VM_TIMER_CMP[i] = u64::MAX;
            }
        }
    }
}
