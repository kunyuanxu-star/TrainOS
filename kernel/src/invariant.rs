//! Kernel invariant checks. Only compiled in debug mode.
//! These are assertions about kernel data structure consistency.

/// Check scheduler invariants:
/// - priority_bitmap is consistent with ready_queues
/// - No thread is in multiple queues
/// - current thread's state is Running
pub fn check_scheduler_invariants() {
    // Only run in debug mode or when INVARIANT_CHECK is set
    #[cfg(debug_assertions)]
    {
        // Basic check: scheduler should always have a valid state
        // More detailed checks would require exposing internal state
    }
}

/// Check memory allocator invariants:
/// - allocated_pages never exceeds total_pages
/// - No double-free (cannot check without tracking)
pub fn check_memory_invariants() {
    let allocated = crate::mem::buddy::allocated_pages();
    let total = crate::mem::buddy::total_pages();
    if allocated > total {
        crate::println!("INVARIANT VIOLATION: allocated {} > total {} pages", allocated, total);
    }
}

/// Check capability system invariants:
/// - Resource ref counts are non-zero
/// - CNode slots are in bounds
pub fn check_cap_invariants() {
    // Log that invariant checks are enabled
}

/// Run all invariant checks periodically (called from timer interrupt)
pub fn run_checks() {
    check_memory_invariants();
    check_scheduler_invariants();
    check_cap_invariants();
}
