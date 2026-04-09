//! Per-CPU structures
//!
//! Each CPU core has its own local data structures

use spin::Mutex;

/// Maximum number of CPUs supported
pub const MAX_CPUS: usize = 8;

/// Per-CPU data structure
/// Each CPU core has its own instance
#[derive(Copy, Clone)]
pub struct PerCpu {
    /// CPU core ID (HART ID)
    pub hartid: usize,
    /// Current task ID
    pub current_task: usize,
    /// Kernel stack pointer for this CPU
    pub kernel_sp: usize,
    /// Scheduler state for this CPU
    pub scheduler_state: usize,
    /// Number of interrupts handled
    pub irq_count: usize,
}

impl PerCpu {
    pub const fn new(hartid: usize) -> Self {
        Self {
            hartid,
            current_task: 0,
            kernel_sp: 0,
            scheduler_state: 0,
            irq_count: 0,
        }
    }
}

/// Per-CPU mutex for each core
static PER_CPU_MUTEX: [Mutex<PerCpu>; MAX_CPUS] = [
    Mutex::new(PerCpu::new(0)),
    Mutex::new(PerCpu::new(1)),
    Mutex::new(PerCpu::new(2)),
    Mutex::new(PerCpu::new(3)),
    Mutex::new(PerCpu::new(4)),
    Mutex::new(PerCpu::new(5)),
    Mutex::new(PerCpu::new(6)),
    Mutex::new(PerCpu::new(7)),
];

/// Initialize per-CPU data structures
pub fn init_per_cpu() {
    // Output 'C' using inline asm
    unsafe {
        core::arch::asm!(
            "li a7, 1",
            "li a0, 67",  // 'C'
            "ecall"
        );
    }
}

/// Get per-CPU mutex for a specific CPU
pub fn get_per_cpu(cpu: usize) -> Option<&'static Mutex<PerCpu>> {
    if cpu < MAX_CPUS {
        Some(&PER_CPU_MUTEX[cpu])
    } else {
        None
    }
}

/// Get current CPU's per-CPU data
pub fn get_current_cpu() -> &'static Mutex<PerCpu> {
    // Read HART ID from tp register
    let hart_id = get_hart_id();
    &PER_CPU_MUTEX[hart_id.min(MAX_CPUS - 1)]
}

/// Increment interrupt count for current CPU
pub fn increment_irq_count() {
    let per_cpu = get_current_cpu();
    let mut data = per_cpu.lock();
    data.irq_count += 1;
}

/// Per-CPU initialization for SMP
/// Called when a secondary HART starts up
pub fn smp_percpu_init() {
    // Get HART ID from tp register
    let hart_id = get_hart_id();

    // Set this HART's per-CPU data
    // In a full implementation, we would configure per-CPU timers, local interrupts, etc.
    crate::print!("[cpu] Per-CPU init for HART ");
    crate::console::print_dec(hart_id);
    crate::println!("");
}

/// Get current HART ID from tp register
pub fn get_hart_id() -> usize {
    let hart_id: usize;
    unsafe {
        core::arch::asm!("mv {0}, tp", out(reg) hart_id);
    }
    hart_id
}

/// Get current task ID on this CPU
pub fn get_current_task() -> usize {
    get_current_cpu().lock().current_task
}

/// Set current task ID on this CPU
pub fn set_current_task(task_id: usize) {
    get_current_cpu().lock().current_task = task_id;
}
