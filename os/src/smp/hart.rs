//! HART (Hardware Thread) management
//!
//! HART = CPU core in RISC-V terminology

use spin::Mutex;

/// HART states
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HartState {
    /// HART is not present
    Unavailable,
    /// HART is powered down
    PoweredDown,
    /// HART is in boot/bootloader
    Booting,
    /// HART is running the kernel
    Running,
    /// HART is in sleep/wfi state
    Sleeping,
}

/// HART information
#[derive(Debug, Clone, Copy)]
pub struct Hart {
    /// HART ID (hartid)
    pub id: usize,
    /// Current state
    pub state: HartState,
    /// Boot PC (where to jump after waking)
    pub boot_pc: usize,
    /// Boot stack pointer
    pub boot_sp: usize,
    /// Number of context switches on this HART
    pub ctx_sw_count: usize,
}

impl Hart {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            state: HartState::Unavailable,
            boot_pc: 0,
            boot_sp: 0,
            ctx_sw_count: 0,
        }
    }
}

/// HART table - one entry per possible HART
const MAX_HARTS: usize = 8;
static HART_TABLE: Mutex<[Option<Hart>; MAX_HARTS]> = Mutex::new([None; MAX_HARTS]);

/// Register a HART as available
pub fn register_hart(hartid: usize, boot_pc: usize, boot_sp: usize) {
    let mut table = HART_TABLE.lock();
    if hartid < MAX_HARTS {
        let mut hart = Hart::new(hartid);
        hart.state = HartState::PoweredDown;
        hart.boot_pc = boot_pc;
        hart.boot_sp = boot_sp;
        table[hartid] = Some(hart);
    }
}

/// Boot a secondary HART (bring it into kernel)
pub fn boot_hart(hartid: usize) {
    let mut table = HART_TABLE.lock();
    if let Some(ref mut hart) = table[hartid] {
        if hart.state == HartState::PoweredDown {
            hart.state = HartState::Booting;
            // In a real implementation, we would:
            // 1. Set up the trap vector for this HART
            // 2. Send an IPI to wake it up
            // 3. Wait for it to reach Running state
            hart.state = HartState::Running;
        }
    }
}

/// Get HART state
pub fn get_hart_state(hartid: usize) -> Option<HartState> {
    let table = HART_TABLE.lock();
    table[hartid].map(|h| h.state)
}

/// Increment context switch count for a HART
pub fn inc_ctx_sw(hartid: usize) {
    let mut table = HART_TABLE.lock();
    if let Some(ref mut hart) = table[hartid] {
        hart.ctx_sw_count += 1;
    }
}

/// Check if all registered HARTs are running
pub fn all_harts_running() -> bool {
    let table = HART_TABLE.lock();
    for i in 0..MAX_HARTS {
        if let Some(ref hart) = table[i] {
            if hart.state != HartState::Running && hart.state != HartState::Unavailable {
                return false;
            }
        }
    }
    true
}
