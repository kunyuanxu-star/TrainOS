/// RISC-V Sscofpmf — Supervisor Counter Overflow and Performance Monitoring
///
/// Provides hardware performance counters for profiling.
/// Cycle (0xC00) and instructions-retired (0xC02) counters are always
/// accessible from S-mode.  Configurable hpmcounter events (3-31) and
/// counter-overflow interrupt (cause 13) are available when Sscofpmf is
/// implemented.
///
/// Reference: RISC-V Sscofpmf extension specification.

use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Whether the PMU has been initialized on this hart.
static PMU_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Global performance monitor instance (one per system, shared across harts).
static mut PERF_MONITOR: Option<PerfMonitor> = None;

// ── Standard event codes (mhpmevent values) ─────────────────────────────

pub const EVENT_CYCLES: u64          = 0x00;
pub const EVENT_ICACHE_MISS: u64     = 0x01;
pub const EVENT_DCACHE_MISS: u64     = 0x03;
pub const EVENT_BRANCH_MISS: u64     = 0x05;
pub const EVENT_BRANCH_TAKEN: u64    = 0x04;
pub const EVENT_STORE_STALL: u64     = 0x08;
pub const EVENT_LOAD_STALL: u64      = 0x06;
pub const EVENT_L1D_MISS: u64        = 0x10;
pub const EVENT_L1I_MISS: u64        = 0x11;
pub const EVENT_TLB_MISS: u64        = 0x20;
pub const EVENT_CSR_READ: u64        = 0x80;
pub const EVENT_ITLB_MISS: u64       = 0x21;
pub const EVENT_DTLB_MISS: u64       = 0x22;
pub const EVENT_FDIV: u64            = 0x30;
pub const EVENT_FADD: u64            = 0x31;
pub const EVENT_FMUL: u64            = 0x32;

// ── CSR addresses ───────────────────────────────────────────────────────

const CSR_CYCLE: u16    = 0xC00;
const CSR_INSTRET: u16  = 0xC02;
const CSR_SCOUNTOVF: u16 = 0xDA0;

// ── PerfEventCounter ────────────────────────────────────────────────────

/// A single hardware performance event counter (hpmcounterN + mhpmeventN).
#[derive(Clone, Copy)]
pub struct PerfEventCounter {
    /// Hardware counter index (3-31).
    pub index: usize,
    /// Event selector written to mhpmevent[3-31] (best-effort from S-mode).
    pub event_id: u64,
    /// Last known counter value.
    pub current_count: u64,
    /// Number of times the counter has been sampled.
    pub sample_count: u64,
    /// Number of overflows recorded for this counter.
    pub overflow_count: u64,
    /// Whether this counter is enabled.
    pub enabled: bool,
    /// Whether overflow interrupts are enabled for this counter.
    pub overflow_intr: bool,
}

impl PerfEventCounter {
    const fn new(index: usize) -> Self {
        Self {
            index,
            event_id: 0,
            current_count: 0,
            sample_count: 0,
            overflow_count: 0,
            enabled: false,
            overflow_intr: false,
        }
    }
}

// ── PerfMonitor ─────────────────────────────────────────────────────────

/// Performance monitoring state machine.
pub struct PerfMonitor {
    /// Counter values at last snapshot (hart 0).
    pub cycles: AtomicU64,
    pub instructions: AtomicU64,
    /// Configurable event counters (hpmcounter 3-31).
    events: [PerfEventCounter; 29],
    /// Number of configured event counters.
    event_count: usize,
    /// Bitmask of counters that have overflow interrupt enabled.
    overflow_mask: AtomicU64,
    /// Bitmask of counters that have overflowed (latch).
    overflow_status: AtomicU64,
    /// Whether profiling is currently active.
    profiling_active: AtomicBool,
    /// Sampling period in cycles.
    pub sample_period: u64,
}

impl PerfMonitor {
    const EMPTY_EC: PerfEventCounter = PerfEventCounter::new(0);

    /// Construct a new performance monitor.
    pub const fn new() -> Self {
        Self {
            cycles: AtomicU64::new(0),
            instructions: AtomicU64::new(0),
            events: [
                Self::EMPTY_EC, Self::EMPTY_EC, Self::EMPTY_EC,
                Self::EMPTY_EC, Self::EMPTY_EC, Self::EMPTY_EC,
                Self::EMPTY_EC, Self::EMPTY_EC, Self::EMPTY_EC,
                Self::EMPTY_EC, Self::EMPTY_EC, Self::EMPTY_EC,
                Self::EMPTY_EC, Self::EMPTY_EC, Self::EMPTY_EC,
                Self::EMPTY_EC, Self::EMPTY_EC, Self::EMPTY_EC,
                Self::EMPTY_EC, Self::EMPTY_EC, Self::EMPTY_EC,
                Self::EMPTY_EC, Self::EMPTY_EC, Self::EMPTY_EC,
                Self::EMPTY_EC, Self::EMPTY_EC, Self::EMPTY_EC,
                Self::EMPTY_EC, Self::EMPTY_EC,
            ],
            event_count: 0,
            overflow_mask: AtomicU64::new(0),
            overflow_status: AtomicU64::new(0),
            profiling_active: AtomicBool::new(false),
            sample_period: 100_000,
        }
    }

    // ── Initialisation ──────────────────────────────────────────────────

    /// Probe for counter availability and initialise the monitor.
    pub fn init() -> Self {
        let mut pmu = Self::new();

        #[cfg(not(test))]
        {
            // Read baseline counter values
            pmu.cycles.store(Self::read_cycles(), Ordering::Relaxed);
            pmu.instructions.store(Self::read_instructions(), Ordering::Relaxed);

            // Probe a few hpmcounter CSRs to confirm availability
            for i in 3..=7 {
                let _ = Self::read_hpmcounter(i);
            }

            PMU_INITIALIZED.store(true, Ordering::SeqCst);
            crate::println!("  Sscofpmf: PMU initialized");
        }

        pmu
    }

    /// Check whether the PMU subsystem is available.
    pub fn available() -> bool {
        PMU_INITIALIZED.load(Ordering::Relaxed)
    }

    // ── CSR accessors (S-mode) ──────────────────────────────────────────

    /// Read the `cycle` CSR (0xC00) — always accessible from S-mode.
    #[inline]
    pub fn read_cycles() -> u64 {
        #[cfg(not(test))]
        unsafe {
            let val: u64;
            core::arch::asm!("csrr {}, 0xC00", out(reg) val);
            val
        }
        #[cfg(test)]
        0
    }

    /// Read the `instret` CSR (0xC02) — always accessible from S-mode.
    #[inline]
    pub fn read_instructions() -> u64 {
        #[cfg(not(test))]
        unsafe {
            let val: u64;
            core::arch::asm!("csrr {}, 0xC02", out(reg) val);
            val
        }
        #[cfg(test)]
        0
    }

    /// Read `hpmcounterN` (0xC03..0xC1F) — accessible from S-mode when
    /// `mcounteren` has the corresponding bit set.
    #[inline]
    pub fn read_hpmcounter(index: usize) -> u64 {
        #[cfg(not(test))]
        unsafe {
            match index {
                3  => { let v: u64; core::arch::asm!("csrr {}, 0xC03", out(reg) v); v }
                4  => { let v: u64; core::arch::asm!("csrr {}, 0xC04", out(reg) v); v }
                5  => { let v: u64; core::arch::asm!("csrr {}, 0xC05", out(reg) v); v }
                6  => { let v: u64; core::arch::asm!("csrr {}, 0xC06", out(reg) v); v }
                7  => { let v: u64; core::arch::asm!("csrr {}, 0xC07", out(reg) v); v }
                8  => { let v: u64; core::arch::asm!("csrr {}, 0xC08", out(reg) v); v }
                9  => { let v: u64; core::arch::asm!("csrr {}, 0xC09", out(reg) v); v }
                10 => { let v: u64; core::arch::asm!("csrr {}, 0xC0A", out(reg) v); v }
                11 => { let v: u64; core::arch::asm!("csrr {}, 0xC0B", out(reg) v); v }
                12 => { let v: u64; core::arch::asm!("csrr {}, 0xC0C", out(reg) v); v }
                13 => { let v: u64; core::arch::asm!("csrr {}, 0xC0D", out(reg) v); v }
                14 => { let v: u64; core::arch::asm!("csrr {}, 0xC0E", out(reg) v); v }
                15 => { let v: u64; core::arch::asm!("csrr {}, 0xC0F", out(reg) v); v }
                16 => { let v: u64; core::arch::asm!("csrr {}, 0xC10", out(reg) v); v }
                17 => { let v: u64; core::arch::asm!("csrr {}, 0xC11", out(reg) v); v }
                18 => { let v: u64; core::arch::asm!("csrr {}, 0xC12", out(reg) v); v }
                19 => { let v: u64; core::arch::asm!("csrr {}, 0xC13", out(reg) v); v }
                20 => { let v: u64; core::arch::asm!("csrr {}, 0xC14", out(reg) v); v }
                21 => { let v: u64; core::arch::asm!("csrr {}, 0xC15", out(reg) v); v }
                22 => { let v: u64; core::arch::asm!("csrr {}, 0xC16", out(reg) v); v }
                23 => { let v: u64; core::arch::asm!("csrr {}, 0xC17", out(reg) v); v }
                24 => { let v: u64; core::arch::asm!("csrr {}, 0xC18", out(reg) v); v }
                25 => { let v: u64; core::arch::asm!("csrr {}, 0xC19", out(reg) v); v }
                26 => { let v: u64; core::arch::asm!("csrr {}, 0xC1A", out(reg) v); v }
                27 => { let v: u64; core::arch::asm!("csrr {}, 0xC1B", out(reg) v); v }
                28 => { let v: u64; core::arch::asm!("csrr {}, 0xC1C", out(reg) v); v }
                29 => { let v: u64; core::arch::asm!("csrr {}, 0xC1D", out(reg) v); v }
                30 => { let v: u64; core::arch::asm!("csrr {}, 0xC1E", out(reg) v); v }
                31 => { let v: u64; core::arch::asm!("csrr {}, 0xC1F", out(reg) v); v }
                _  => 0,
            }
        }
        #[cfg(test)]
        0
    }

    /// Write `mhpmeventN` (0x323..0x33F) — M-mode CSR, best-effort from S-mode.
    /// On platforms without Sscofpmf the write may be silently ignored.
    #[inline]
    pub fn write_mhpmevent(index: usize, event_id: u64) {
        #[cfg(not(test))]
        unsafe {
            match index {
                3  => core::arch::asm!("csrw 0x323, {}", in(reg) event_id),
                4  => core::arch::asm!("csrw 0x324, {}", in(reg) event_id),
                5  => core::arch::asm!("csrw 0x325, {}", in(reg) event_id),
                6  => core::arch::asm!("csrw 0x326, {}", in(reg) event_id),
                7  => core::arch::asm!("csrw 0x327, {}", in(reg) event_id),
                8  => core::arch::asm!("csrw 0x328, {}", in(reg) event_id),
                9  => core::arch::asm!("csrw 0x329, {}", in(reg) event_id),
                10 => core::arch::asm!("csrw 0x32A, {}", in(reg) event_id),
                11 => core::arch::asm!("csrw 0x32B, {}", in(reg) event_id),
                12 => core::arch::asm!("csrw 0x32C, {}", in(reg) event_id),
                13 => core::arch::asm!("csrw 0x32D, {}", in(reg) event_id),
                14 => core::arch::asm!("csrw 0x32E, {}", in(reg) event_id),
                15 => core::arch::asm!("csrw 0x32F, {}", in(reg) event_id),
                16 => core::arch::asm!("csrw 0x330, {}", in(reg) event_id),
                17 => core::arch::asm!("csrw 0x331, {}", in(reg) event_id),
                18 => core::arch::asm!("csrw 0x332, {}", in(reg) event_id),
                19 => core::arch::asm!("csrw 0x333, {}", in(reg) event_id),
                20 => core::arch::asm!("csrw 0x334, {}", in(reg) event_id),
                21 => core::arch::asm!("csrw 0x335, {}", in(reg) event_id),
                22 => core::arch::asm!("csrw 0x336, {}", in(reg) event_id),
                23 => core::arch::asm!("csrw 0x337, {}", in(reg) event_id),
                24 => core::arch::asm!("csrw 0x338, {}", in(reg) event_id),
                25 => core::arch::asm!("csrw 0x339, {}", in(reg) event_id),
                26 => core::arch::asm!("csrw 0x33A, {}", in(reg) event_id),
                27 => core::arch::asm!("csrw 0x33B, {}", in(reg) event_id),
                28 => core::arch::asm!("csrw 0x33C, {}", in(reg) event_id),
                29 => core::arch::asm!("csrw 0x33D, {}", in(reg) event_id),
                30 => core::arch::asm!("csrw 0x33E, {}", in(reg) event_id),
                31 => core::arch::asm!("csrw 0x33F, {}", in(reg) event_id),
                _  => {}
            }
        }
    }

    /// Read `scountovf` (0xDA0) — returns the overflow status bitmask.
    #[inline]
    fn read_scountovf() -> u64 {
        #[cfg(not(test))]
        unsafe {
            let val: u64;
            core::arch::asm!("csrr {}, 0xDA0", out(reg) val);
            val
        }
        #[cfg(test)]
        0
    }

    /// Write `scountovf` (0xDA0) — clears the specified overflow bits.
    #[inline]
    fn write_scountovf(mask: u64) {
        #[cfg(not(test))]
        unsafe {
            core::arch::asm!("csrw 0xDA0, {}", in(reg) mask);
        }
    }

    // ── Profiling control ───────────────────────────────────────────────

    /// Start profiling on a set of events.
    pub fn start_profiling(&mut self, events: &[u64]) -> usize {
        let count = events.len().min(29);
        for i in 0..count {
            let idx = 3 + i;
            self.events[i] = PerfEventCounter {
                index: idx,
                event_id: events[i],
                current_count: Self::read_hpmcounter(idx),
                sample_count: 0,
                overflow_count: 0,
                enabled: true,
                overflow_intr: false,
            };
            Self::write_mhpmevent(idx, events[i]);
        }
        self.event_count = count;
        self.profiling_active.store(true, Ordering::SeqCst);
        count
    }

    /// Stop profiling and return the snapshot.
    pub fn stop_profiling(&mut self) -> PerfSnapshot {
        let snap = self.read_counters();
        self.profiling_active.store(false, Ordering::SeqCst);
        snap
    }

    /// Take a snapshot of all active counters.
    pub fn read_counters(&self) -> PerfSnapshot {
        let cycles = Self::read_cycles();
        let instructions = Self::read_instructions();
        let mut events = [(0u64, 0u64); 29];
        for i in 0..self.event_count {
            if self.events[i].enabled {
                events[i] = (self.events[i].event_id, Self::read_hpmcounter(self.events[i].index));
            }
        }
        PerfSnapshot {
            cycles,
            instructions,
            events,
            timestamp: cycles,
        }
    }

    /// Enable sampling at `period` cycles.
    pub fn enable_sampling(&mut self, period: u64) {
        self.sample_period = period;
        // Enable overflow for counter 3 by default (cycle-based sampling)
        if self.event_count > 0 {
            self.events[0].overflow_intr = true;
            self.overflow_mask.store(1 << 3, Ordering::SeqCst);
        }
    }

    /// Handle counter overflow interrupt (scause 13, interrupt=1).
    pub fn handle_overflow(&mut self) {
        #[cfg(not(test))]
        {
            let status = Self::read_scountovf();
            if status != 0 {
                self.overflow_status.fetch_or(status, Ordering::SeqCst);

                // Update counter values for overflowed counters
                for i in 0..self.event_count {
                    let bit = 1u64 << (self.events[i].index);
                    if status & bit != 0 {
                        self.events[i].overflow_count += 1;
                        self.events[i].current_count = Self::read_hpmcounter(self.events[i].index);
                    }
                }

                // Clear overflow bits
                Self::write_scountovf(status);
            }
        }
    }

    /// Reset all counters and profiling state.
    pub fn reset(&mut self) {
        self.cycles.store(0, Ordering::Relaxed);
        self.instructions.store(0, Ordering::Relaxed);
        for i in 0..self.event_count {
            self.events[i] = PerfEventCounter::new(3 + i);
        }
        self.event_count = 0;
        self.overflow_mask.store(0, Ordering::SeqCst);
        self.overflow_status.store(0, Ordering::SeqCst);
        self.profiling_active.store(false, Ordering::SeqCst);
    }

    /// Generate a performance summary from current counter values.
    pub fn summary(&self) -> PerfSummary {
        let cycles = Self::read_cycles();
        let instructions = Self::read_instructions();

        let ipc = if cycles > 0 {
            instructions as f32 / cycles as f32
        } else {
            0.0
        };

        let mut icache_misses = 0u64;
        let mut dcache_misses = 0u64;
        let mut branch_misses = 0u64;
        let mut tlb_misses = 0u64;

        for i in 0..self.event_count {
            let cnt = Self::read_hpmcounter(self.events[i].index);
            match self.events[i].event_id {
                EVENT_ICACHE_MISS => icache_misses = cnt,
                EVENT_DCACHE_MISS => dcache_misses = cnt,
                EVENT_BRANCH_MISS => branch_misses = cnt,
                EVENT_TLB_MISS => tlb_misses = cnt,
                _ => {}
            }
        }

        PerfSummary {
            cycles,
            instructions,
            ipc,
            icache_misses,
            dcache_misses,
            branch_misses,
            tlb_misses,
            cpu_utilization: 1.0,
        }
    }

    /// Number of configured event counters.
    pub fn event_count(&self) -> usize {
        self.event_count
    }

    /// Borrow event counters.
    pub fn events(&self) -> &[PerfEventCounter] {
        &self.events[..self.event_count]
    }
}

// ── Snapshot / Summary types ────────────────────────────────────────────

/// Point-in-time snapshot of all performance counters.
pub struct PerfSnapshot {
    pub cycles: u64,
    pub instructions: u64,
    pub events: [(u64, u64); 29],   // (event_id, count)
    pub timestamp: u64,
}

/// Human-readable performance summary.
pub struct PerfSummary {
    pub cycles: u64,
    pub instructions: u64,
    pub ipc: f32,
    pub icache_misses: u64,
    pub dcache_misses: u64,
    pub branch_misses: u64,
    pub tlb_misses: u64,
    pub cpu_utilization: f32,
}

// ── Global accessors (used by trap handler & syscalls) ──────────────────

/// Initialise the global PMU instance.
pub fn init() {
    #[cfg(not(test))]
    unsafe {
        PERF_MONITOR = Some(PerfMonitor::init());
    }
}

/// Handle counter overflow interrupt — called from trap dispatch.
pub fn handle_counter_overflow() {
    #[cfg(not(test))]
    unsafe {
        if let Some(ref mut pmu) = PERF_MONITOR {
            pmu.handle_overflow();
        }
    }
}

/// Read current performance snapshot via global monitor.
pub fn read_perf() -> Option<&'static PerfSnapshot> {
    #[cfg(not(test))]
    unsafe {
        // Return a stack-allocated snapshot is not possible directly.
        // We return None and let syscalls interact with the monitor directly.
        None
    }
    #[cfg(test)]
    None
}

/// Read performance summary via global monitor.
pub fn read_summary() -> Option<PerfSummary> {
    #[cfg(not(test))]
    unsafe {
        PERF_MONITOR.as_ref().map(|pmu| pmu.summary())
    }
    #[cfg(test)]
    None
}

/// Start profiling on the global monitor with the given event ids.
pub fn start_profiling(events: &[u64]) -> usize {
    #[cfg(not(test))]
    unsafe {
        PERF_MONITOR.as_mut().map(|pmu| pmu.start_profiling(events)).unwrap_or(0)
    }
    #[cfg(test)]
    0
}

/// Stop profiling on the global monitor.
pub fn stop_profiling() {
    #[cfg(not(test))]
    unsafe {
        if let Some(ref mut pmu) = PERF_MONITOR {
            pmu.stop_profiling();
        }
    }
}

/// Configure sampling period on the global monitor.
pub fn enable_sampling(period: u64) {
    #[cfg(not(test))]
    unsafe {
        if let Some(ref mut pmu) = PERF_MONITOR {
            pmu.enable_sampling(period);
        }
    }
}

// ── Syscall helpers ─────────────────────────────────────────────────────

/// Copy performance summary into a user-supplied buffer.
/// Format:
///   [0..7]  cycles
///   [8..15] instructions
///   [16..19] ipc (as f32 bits)
///   [20..27] icache_misses
///   [28..35] dcache_misses
///   [36..43] branch_misses
///   [44..51] tlb_misses
///   [52..55] cpu_utilization (as f32 bits)
///   Total: 56 bytes.
pub fn sys_perf_read(buf: &mut [u8]) -> usize {
    if buf.len() < 56 {
        return 0;
    }
    match read_summary() {
        Some(summary) => {
            let cycles_bytes = summary.cycles.to_le_bytes();
            let instructions_bytes = summary.instructions.to_le_bytes();
            let ipc_bytes = summary.ipc.to_bits().to_le_bytes();
            let icache_bytes = summary.icache_misses.to_le_bytes();
            let dcache_bytes = summary.dcache_misses.to_le_bytes();
            let branch_bytes = summary.branch_misses.to_le_bytes();
            let tlb_bytes = summary.tlb_misses.to_le_bytes();
            let cpu_bytes = summary.cpu_utilization.to_bits().to_le_bytes();

            buf[0..8].copy_from_slice(&cycles_bytes);
            buf[8..16].copy_from_slice(&instructions_bytes);
            buf[16..20].copy_from_slice(&ipc_bytes);
            buf[20..28].copy_from_slice(&icache_bytes);
            buf[28..36].copy_from_slice(&dcache_bytes);
            buf[36..44].copy_from_slice(&branch_bytes);
            buf[44..52].copy_from_slice(&tlb_bytes);
            buf[52..56].copy_from_slice(&cpu_bytes);
            56
        }
        None => 0,
    }
}
