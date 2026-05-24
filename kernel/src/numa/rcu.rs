// V25: RCU (Read-Copy-Update) — single-writer grace period tracking.
//
// Readers bracket critical sections with rcu_read_lock() / rcu_read_unlock().
// A writer calls synchronize_rcu() to wait for all current readers to finish.
//
// This is a minimal implementation using per-CPU counters for the reader
// count and a spin-wait for the grace period.

#![allow(dead_code)]

use crate::numa::percpu::PerCpuCounter;
use core::sync::atomic::Ordering;

/// Global reader counter — one per hart, summed on read.
static RCU_READERS: PerCpuCounter = PerCpuCounter::new();

/// Enter an RCU read-side critical section.
pub fn rcu_read_lock() {
    RCU_READERS.inc();
    core::sync::atomic::fence(Ordering::Acquire);
}

/// Exit an RCU read-side critical section.
pub fn rcu_read_unlock() {
    core::sync::atomic::fence(Ordering::Release);
    RCU_READERS.dec();
}

/// Wait for all readers to finish (grace period).
///
/// In a single-writer scenario this blocks until every hart that was inside
/// an rcu_read_lock() has called rcu_read_unlock().
pub fn synchronize_rcu() {
    loop {
        core::sync::atomic::fence(Ordering::SeqCst);
        if RCU_READERS.read() == 0 {
            break;
        }
        core::hint::spin_loop();
    }
    // Ensure subsequent writes are visible after the grace period.
    core::sync::atomic::fence(Ordering::SeqCst);
}

/// Return the current reader count (for diagnostics).
pub fn rcu_reader_count() -> u64 {
    RCU_READERS.read()
}
