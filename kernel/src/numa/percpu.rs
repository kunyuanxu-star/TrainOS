// V25: Per-CPU counters — avoid cache-line bouncing by giving each hart
// its own counter. Reading sums over all harts.

#![allow(dead_code)]

use core::sync::atomic::{AtomicU64, Ordering};

const MAX_HARTS: usize = 8;

/// A counter that maintains one value per CPU.
/// Increment/decrement touch only the local hart's cache line.
/// Reading sums all harts.
pub struct PerCpuCounter {
    counters: [AtomicU64; MAX_HARTS],
}

impl PerCpuCounter {
    pub const fn new() -> Self {
        const ZERO: AtomicU64 = AtomicU64::new(0);
        PerCpuCounter {
            counters: [ZERO; MAX_HARTS],
        }
    }

    /// Increment the local hart's counter.
    pub fn inc(&self) {
        let hart = crate::per_cpu::hart_id();
        self.counters[hart].fetch_add(1, Ordering::Relaxed);
    }

    /// Decrement the local hart's counter.
    pub fn dec(&self) {
        let hart = crate::per_cpu::hart_id();
        self.counters[hart].fetch_sub(1, Ordering::Relaxed);
    }

    /// Read the sum across all harts.
    pub fn read(&self) -> u64 {
        let mut sum = 0;
        for i in 0..MAX_HARTS {
            sum += self.counters[i].load(Ordering::Relaxed);
        }
        sum
    }

    /// Read the value for a specific hart.
    pub fn per_hart(&self, hart: usize) -> u64 {
        self.counters[hart].load(Ordering::Relaxed)
    }

    /// Reset all counters to zero.
    pub fn reset(&self) {
        for i in 0..MAX_HARTS {
            self.counters[i].store(0, Ordering::Relaxed);
        }
    }
}
