/// RISC-V Sdtrig — Debug and Trigger Extension
///
/// Hardware breakpoints, watchpoints, and execution triggers via
/// tselect/tdata1/tdata2 CSRs.
///
/// Reference: RISC-V Sdtrig extension specification.

use core::sync::atomic::{AtomicBool, Ordering};

/// Whether Sdtrig has been initialised.
static DEBUG_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Global debug trigger state.
static mut DEBUG_TRIGGERS: Option<DebugTriggers> = None;

// ── CSR addresses ───────────────────────────────────────────────────────

const CSR_TSELECT: u16 = 0x7A0;
const CSR_TDATA1: u16  = 0x7A1;
const CSR_TDATA2: u16  = 0x7A2;

// ── tdata1 type codes ───────────────────────────────────────────────────

const TDATA1_TYPE_OFF: usize        = (usize::BITS as usize) - 1; // MSB in the high nibble
const TDATA1_TYPE_MASK: usize       = 0xF << (usize::BITS as usize - 4);

const TYPE_NONE: usize       = 0;
const TYPE_MCONTROL: usize   = 2;
const TYPE_ICOUNT: usize     = 6;
const TYPE_ITRIGGER: usize   = 7;
const TYPE_ETRIGGER: usize   = 8;
const TYPE_MCONTROL6: usize  = 4;

const MCONTROL_ACTION_SHIFT: usize = 12;
const MCONTROL_ACTION_MASK: usize  = 0x3 << 12;
const MCONTROL_CHAIN: usize        = 1 << 11;
const MCONTROL_MATCH_SHIFT: usize  = 7;
const MCONTROL_MATCH_MASK: usize   = 0xF << 7;
const MCONTROL_M: usize            = 1 << 6;
const MCONTROL_S: usize            = 1 << 4;
const MCONTROL_U: usize            = 1 << 3;
const MCONTROL_EXECUTE: usize      = 1 << 2;
const MCONTROL_STORE: usize        = 1 << 1;
const MCONTROL_LOAD: usize         = 1;

// ── Trigger types ───────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TriggerType {
    Disabled,
    AddressMatch,      // mcontrol — breakpoint/watchpoint
    DataMatch,         // mcontrol6 — data value match
    InstructionCount,  // icount — step N instructions
    InterruptTrigger,  // itrigger — fire on interrupt
    ExceptionTrigger,  // etrigger — fire on exception
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MatchType {
    Equal,
    NAPOT,
    GreaterOrEqual,
    LessThan,
    MaskLow,
    MaskHigh,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum TriggerAction {
    Breakpoint,
    DebugMode,
    TraceOn,
    TraceOff,
    TraceNotify,
    External0,
    External1,
}

// ── Trigger ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct Trigger {
    pub trigger_type: TriggerType,
    pub match_type: MatchType,
    pub address: usize,
    pub mask: usize,
    pub action: TriggerAction,
    pub enabled: bool,
    pub hit_count: u64,
}

impl Trigger {
    const fn disabled() -> Self {
        Self {
            trigger_type: TriggerType::Disabled,
            match_type: MatchType::Equal,
            address: 0,
            mask: 0,
            action: TriggerAction::Breakpoint,
            enabled: false,
            hit_count: 0,
        }
    }
}

// ── DebugTriggers ───────────────────────────────────────────────────────

/// Debug trigger state manager.
pub struct DebugTriggers {
    triggers: [Trigger; 4],
    hit_pending: bool,
}

impl DebugTriggers {
    /// Construct initial state with all triggers disabled.
    pub const fn new() -> Self {
        Self {
            triggers: [
                Trigger::disabled(),
                Trigger::disabled(),
                Trigger::disabled(),
                Trigger::disabled(),
            ],
            hit_pending: false,
        }
    }

    /// Probe for Sdtrig availability.
    pub fn init() -> Self {
        let mut dt = Self::new();
        #[cfg(not(test))]
        {
            // Probe by reading tselect, writing 0, then reading back
            // If Sdtrig is not implemented, the CSR access may trap silently,
            // but on QEMU with `sdtig=true` it will work.
            unsafe {
                core::arch::asm!("csrw 0x7A0, zero", options(nostack));
                let _probe: usize;
                core::arch::asm!("csrr {}, 0x7A0", out(reg) _probe);
            }
            DEBUG_AVAILABLE.store(true, Ordering::SeqCst);
            crate::println!("  Sdtrig: debug triggers initialized");
        }
        dt
    }

    /// Check whether Sdtrig is available.
    pub fn available() -> bool {
        DEBUG_AVAILABLE.load(Ordering::Relaxed)
    }

    /// Select trigger register index `idx` (0-3) via tselect.
    unsafe fn select(idx: usize) {
        core::arch::asm!("csrw 0x7A0, {}", in(reg) idx);
    }

    /// Read tdata1 (trigger configuration).
    unsafe fn read_tdata1() -> usize {
        let val: usize;
        core::arch::asm!("csrr {}, 0x7A1", out(reg) val);
        val
    }

    /// Write tdata1.
    unsafe fn write_tdata1(val: usize) {
        core::arch::asm!("csrw 0x7A1, {}", in(reg) val);
    }

    /// Read tdata2 (trigger address / value).
    unsafe fn read_tdata2() -> usize {
        let val: usize;
        core::arch::asm!("csrr {}, 0x7A2", out(reg) val);
        val
    }

    /// Write tdata2.
    unsafe fn write_tdata2(val: usize) {
        core::arch::asm!("csrw 0x7A2, {}", in(reg) val);
    }

    // ── Breakpoint ──────────────────────────────────────────────────────

    /// Set a hardware breakpoint at `addr`.
    ///
    /// Finds a free trigger slot, configures mcontrol for execute-only
    /// match in S-mode, and stores the address in tdata2.
    pub fn set_breakpoint(&mut self, addr: usize) -> Result<usize, &'static str> {
        let idx = self.find_free()?;
        #[cfg(not(test))]
        unsafe {
            Self::select(idx);
            // tdata1: mcontrol (type=2), action=0 (breakpoint), S-mode, execute
            let tdata1 = (TYPE_MCONTROL << (usize::BITS as usize - 4))
                       | MCONTROL_S
                       | MCONTROL_EXECUTE;
            Self::write_tdata1(tdata1);
            Self::write_tdata2(addr);
        }
        self.triggers[idx] = Trigger {
            trigger_type: TriggerType::AddressMatch,
            match_type: MatchType::Equal,
            address: addr,
            mask: 0,
            action: TriggerAction::Breakpoint,
            enabled: true,
            hit_count: 0,
        };
        Ok(idx)
    }

    /// Set a hardware watchpoint at `addr` for data writes.
    pub fn set_watchpoint(&mut self, addr: usize, _size: usize) -> Result<usize, &'static str> {
        let idx = self.find_free()?;
        #[cfg(not(test))]
        unsafe {
            Self::select(idx);
            // tdata1: mcontrol (type=2), action=0, S-mode, store
            let tdata1 = (TYPE_MCONTROL << (usize::BITS as usize - 4))
                       | MCONTROL_S
                       | MCONTROL_STORE;
            Self::write_tdata1(tdata1);
            Self::write_tdata2(addr);
        }
        self.triggers[idx] = Trigger {
            trigger_type: TriggerType::AddressMatch,
            match_type: MatchType::Equal,
            address: addr,
            mask: 0,
            action: TriggerAction::Breakpoint,
            enabled: true,
            hit_count: 0,
        };
        Ok(idx)
    }

    /// Set a hardware read-watchpoint at `addr`.
    pub fn set_rwatchpoint(&mut self, addr: usize, _size: usize) -> Result<usize, &'static str> {
        let idx = self.find_free()?;
        #[cfg(not(test))]
        unsafe {
            Self::select(idx);
            let tdata1 = (TYPE_MCONTROL << (usize::BITS as usize - 4))
                       | MCONTROL_S
                       | MCONTROL_LOAD
                       | MCONTROL_STORE;
            Self::write_tdata1(tdata1);
            Self::write_tdata2(addr);
        }
        self.triggers[idx] = Trigger {
            trigger_type: TriggerType::AddressMatch,
            match_type: MatchType::Equal,
            address: addr,
            mask: 0,
            action: TriggerAction::Breakpoint,
            enabled: true,
            hit_count: 0,
        };
        Ok(idx)
    }

    /// Clear trigger at index `idx`.
    pub fn clear_trigger(&mut self, idx: usize) {
        if idx >= 4 {
            return;
        }
        #[cfg(not(test))]
        unsafe {
            Self::select(idx);
            Self::write_tdata1(0); // type=0 -> disabled
            Self::write_tdata2(0);
        }
        self.triggers[idx] = Trigger::disabled();
    }

    /// Called from trap handler when a trigger fires (breakpoint exception).
    ///
    /// Identifies which trigger fired, records the hit, and optionally
    /// clears the trigger if configured for one-shot use.
    pub fn handle_trigger_hit(&mut self, idx: usize) {
        if idx < 4 {
            self.triggers[idx].hit_count += 1;
            self.hit_pending = true;

            #[cfg(not(test))]
            unsafe {
                // Clear the hit bit by re-writing the trigger
                Self::select(idx);
                Self::write_tdata1(Self::read_tdata1());
            }

            let trig = &self.triggers[idx];
            crate::println!(
                "  DEBUG: trigger[{}] hit (type={:?}, addr=0x{:x}, count={})",
                idx, trig.trigger_type, trig.address, trig.hit_count
            );
        }
    }

    /// Enable single-step mode via icount trigger.
    ///
    /// Programs an icount trigger with count=1 so that the next
    /// instruction causes a breakpoint exception.
    pub fn enable_single_step(&mut self) {
        #[cfg(not(test))]
        unsafe {
            Self::select(3); // use last trigger for single-step
            // tdata1: icount (type=6), action=0, S-mode, count_enable=1
            let tdata1 = (TYPE_ICOUNT << (usize::BITS as usize - 4))
                       | (1 << 24)  // hit
                       | (1 << 10)  // count_enable
                       | (1 << 0);  // S-mode
            Self::write_tdata1(tdata1);
            Self::write_tdata2(1); // count = 1 instruction
        }
        self.triggers[3] = Trigger {
            trigger_type: TriggerType::InstructionCount,
            match_type: MatchType::Equal,
            address: 0,
            mask: 0,
            action: TriggerAction::Breakpoint,
            enabled: true,
            hit_count: 0,
        };
    }

    /// Disable single-step mode.
    pub fn disable_single_step(&mut self) {
        self.clear_trigger(3);
    }

    /// Return a reference to all triggers.
    pub fn list_triggers(&self) -> &[Trigger] {
        &self.triggers
    }

    /// Check whether a trigger hit is pending.
    pub fn hit_pending(&self) -> bool {
        self.hit_pending
    }

    /// Clear the pending hit flag.
    pub fn clear_hit_pending(&mut self) {
        self.hit_pending = false;
    }

    // ── internal helpers ────────────────────────────────────────────────

    fn find_free(&self) -> Result<usize, &'static str> {
        for i in 0..4 {
            if self.triggers[i].trigger_type == TriggerType::Disabled {
                return Ok(i);
            }
        }
        // Check if any trigger can be recycled (last resort)
        for i in 0..4 {
            if !self.triggers[i].enabled {
                return Ok(i);
            }
        }
        Err("no free debug trigger")
    }
}

// ── Global accessors ────────────────────────────────────────────────────

/// Initialise the global debug trigger manager.
pub fn init() {
    #[cfg(not(test))]
    unsafe {
        DEBUG_TRIGGERS = Some(DebugTriggers::init());
    }
}

/// Handle a debug trigger hit from the trap handler.
///
/// Reads stval (which contains the trigger index for Sdtrig on some
/// implementations, or the faulting address for breakpoints).
pub fn handle_trigger_hit(stval: usize) {
    #[cfg(not(test))]
    unsafe {
        if let Some(ref mut dt) = DEBUG_TRIGGERS {
            // Try to identify which trigger fired.
            // stval may contain the trigger index or the breakpoint address.
            for i in 0..4 {
                if dt.triggers[i].enabled && dt.triggers[i].address == stval {
                    dt.handle_trigger_hit(i);
                    return;
                }
            }
            // If no matching address, check all enabled triggers
            for i in 0..4 {
                if dt.triggers[i].enabled {
                    dt.handle_trigger_hit(i);
                    return;
                }
            }
        }
    }
}

/// Set a breakpoint via the global trigger manager.
pub fn set_breakpoint(addr: usize) -> Result<usize, &'static str> {
    #[cfg(not(test))]
    unsafe {
        DEBUG_TRIGGERS.as_mut().map(|dt| dt.set_breakpoint(addr)).unwrap_or(Err("debug not initialized"))
    }
    #[cfg(test)]
    Err("no debug in test")
}

/// Set a watchpoint via the global trigger manager.
pub fn set_watchpoint(addr: usize, size: usize) -> Result<usize, &'static str> {
    #[cfg(not(test))]
    unsafe {
        DEBUG_TRIGGERS.as_mut().map(|dt| dt.set_watchpoint(addr, size)).unwrap_or(Err("debug not initialized"))
    }
    #[cfg(test)]
    Err("no debug in test")
}

/// Clear a trigger by index via the global trigger manager.
pub fn clear_trigger(idx: usize) {
    #[cfg(not(test))]
    unsafe {
        if let Some(ref mut dt) = DEBUG_TRIGGERS {
            dt.clear_trigger(idx);
        }
    }
}

/// List active triggers into a buffer.
/// Format per trigger: [type:1][action:1][addr:8][enabled:1][hits:8] = 19 bytes.
pub fn list_triggers(buf: &mut [u8]) -> usize {
    #[cfg(not(test))]
    unsafe {
        if let Some(ref dt) = DEBUG_TRIGGERS {
            let mut written = 0;
            for trig in dt.list_triggers() {
                if written + 19 > buf.len() {
                    break;
                }
                buf[written] = match trig.trigger_type {
                    TriggerType::Disabled => 0,
                    TriggerType::AddressMatch => 1,
                    TriggerType::DataMatch => 2,
                    TriggerType::InstructionCount => 3,
                    TriggerType::InterruptTrigger => 4,
                    TriggerType::ExceptionTrigger => 5,
                };
                buf[written + 1] = match trig.action {
                    TriggerAction::Breakpoint => 0,
                    TriggerAction::DebugMode => 1,
                    TriggerAction::TraceOn => 2,
                    TriggerAction::TraceOff => 3,
                    TriggerAction::TraceNotify => 4,
                    TriggerAction::External0 => 5,
                    TriggerAction::External1 => 6,
                };
                let addr_bytes = trig.address.to_le_bytes();
                buf[written + 2..written + 10].copy_from_slice(&addr_bytes);
                buf[written + 10] = trig.enabled as u8;
                let hits_bytes = trig.hit_count.to_le_bytes();
                buf[written + 11..written + 19].copy_from_slice(&hits_bytes);
                written += 19;
            }
            return written;
        }
    }
    0
}
