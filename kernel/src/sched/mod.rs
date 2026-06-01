use crate::mem::zicond::{cmov_u64, should_select_thread};
use crate::proc::switch::context_switch;
use crate::proc::thread::{PreemptMode, Thread, ThreadState};
use crate::sync::SpinLock;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

pub static CTX_SWITCH_COUNT: AtomicU64 = AtomicU64::new(0);

/// Simple FIFO queue backed by a Vec
pub(crate) struct ThreadQueue {
    items: Vec<*mut Thread>,
    head: usize,
}

// ThreadQueue is used behind a spin::Mutex, which requires Send.
// The kernel's single-address-space design with proper locking makes this safe.
unsafe impl Send for ThreadQueue {}

impl ThreadQueue {
    pub(crate) const fn new() -> Self {
        ThreadQueue {
            items: Vec::new(),
            head: 0,
        }
    }

    fn push_back(&mut self, t: *mut Thread) {
        self.items.push(t);
    }

    pub(crate) fn pop_front(&mut self) -> Option<*mut Thread> {
        if self.head >= self.items.len() {
            return None;
        }
        let t = self.items[self.head];
        self.head += 1;
        // Compact when head is large to avoid unbounded growth
        if self.head > 64 && self.head >= self.items.len() / 2 {
            self.items.drain(0..self.head);
            self.head = 0;
        }
        Some(t)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.head >= self.items.len()
    }

    pub(crate) fn iter(&self) -> core::slice::Iter<'_, *mut Thread> {
        self.items[self.head..].iter()
    }

    /// Insert a thread into the queue sorted by deadline (ascending).
    /// Used by EEVDF to maintain deadline order within a priority level.
    pub(crate) fn insert_sorted_by_deadline(&mut self, t: *mut Thread, deadline: u64) {
        let mut insert_pos = self.items.len();
        for i in self.head..self.items.len() {
            let d = unsafe { (*self.items[i]).deadline };
            if deadline < d {
                insert_pos = i;
                break;
            }
        }
        self.items.insert(insert_pos, t);
    }
}

pub struct Scheduler {
    pub(crate) current: Option<*mut Thread>,
    pick_count: [u64; 4], // picks per HART
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            current: None,
            pick_count: [0; 4],
        }
    }

    /// Enqueue a thread — delegates to the NUMA-aware scheduler.
    pub fn enqueue(&mut self, thread: *mut Thread) {
        crate::numa::enqueue_thread(thread);
    }

    /// Dequeue the highest-priority thread for the current hart's NUMA node.
    fn dequeue_highest(&mut self) -> Option<*mut Thread> {
        let hart = crate::per_cpu::hart_id();
        let thread = crate::numa::pick_next_for_hart(hart)?;
        self.pick_count[hart] += 1;
        Some(thread)
    }

    pub fn current(&self) -> Option<*mut Thread> {
        self.current
    }

    pub fn set_current(&mut self, thread: *mut Thread) {
        self.current = Some(thread);
    }
}

pub(crate) static SCHED_LOCK: SpinLock = SpinLock::new();
pub(crate) static mut SCHEDULER: Scheduler = Scheduler::new();

pub fn schedule() {
    SCHED_LOCK.lock();
    let current_ptr = unsafe { (*core::ptr::addr_of_mut!(SCHEDULER)).current };

    // Re-enqueue current if still Running
    if let Some(cur) = current_ptr {
        unsafe {
            let cur_state = (*cur).state;
            if cur_state == ThreadState::Running {
                (*core::ptr::addr_of_mut!(SCHEDULER)).enqueue(cur);
            }
        }
    }

    let next = unsafe { (*core::ptr::addr_of_mut!(SCHEDULER)).dequeue_highest() };
    // V35: Update last_cpu for CAS — record which CPU this thread ran on
    if let Some(n) = next {
        unsafe {
            (*n).last_cpu = crate::per_cpu::hart_id() as u8;
        }
    }
    unsafe {
        (*core::ptr::addr_of_mut!(SCHEDULER)).current = next;
    }
    SCHED_LOCK.unlock();

    // Context switch OUTSIDE the lock to avoid deadlocks
    unsafe {
        match (current_ptr, next) {
            (Some(old), Some(new)) => {
                // V36a: Save/restore vector context before register context switch
                crate::trap::switch_vector_context(old, new);
                context_switch(&mut (*old).task_ctx, &(*new).task_ctx);
                CTX_SWITCH_COUNT.fetch_add(1, Ordering::Relaxed);
            }
            (None, Some(new)) => {
                CTX_SWITCH_COUNT.fetch_add(1, Ordering::Relaxed);
                let ra = (*new).task_ctx.ra;
                let sp = (*new).task_ctx.sp;
                core::arch::asm!(
                    "mv sp, {sp}",
                    "jr {ra}",
                    sp = in(reg) sp,
                    ra = in(reg) ra,
                    options(noreturn),
                );
            }
            _ => {
                crate::console::puts("SCHED: no thread!\r\n");
                crate::idle_loop();
            }
        }
    }
}

pub fn enqueue_thread(thread: *mut Thread) {
    SCHED_LOCK.lock();
    unsafe {
        (*core::ptr::addr_of_mut!(SCHEDULER)).enqueue(thread);
    }
    SCHED_LOCK.unlock();
}

pub fn current_thread() -> Option<*mut Thread> {
    SCHED_LOCK.lock();
    let cur = unsafe { (*core::ptr::addr_of_mut!(SCHEDULER)).current };
    SCHED_LOCK.unlock();
    cur
}

pub fn start_scheduler(idle: *mut Thread) -> ! {
    {
        SCHED_LOCK.lock();
        unsafe {
            (*core::ptr::addr_of_mut!(SCHEDULER)).current = Some(idle);
        }
        SCHED_LOCK.unlock();
    }
    schedule();
    crate::console::puts("SCHED: schedule returned!\r\n");
    crate::idle_loop();
}

// ═══════════════════════════════════════════════════════════════════════════
// V35 — PREEMPT_LAZY (Deferred Preemption)
// ═══════════════════════════════════════════════════════════════════════════

/// Check whether the given thread should be preempted according to its
/// preemption mode.
///
/// Returns `true` when a reschedule is needed:
///  - `PreemptMode::Immediate` → always preempt
///  - `PreemptMode::Lazy`      → preempt only when `remaining_ticks == 0`
///                               (tick boundary)
///  - `PreemptMode::None`      → never preempt
pub fn check_preempt_lazy(current: *mut Thread) -> bool {
    unsafe {
        match (*current).preempt_mode {
            PreemptMode::Immediate => true,
            PreemptMode::Lazy => (*current).remaining_ticks == 0,
            PreemptMode::None => false,
        }
    }
}

/// Set the preemption mode for a thread (syscall backend).
pub fn set_preempt_mode(thread: *mut Thread, mode: PreemptMode) {
    unsafe {
        (*thread).preempt_mode = mode;
    }
}

/// Syscall: set preemption mode for a process.
/// `pid` = 0 means the calling thread, otherwise a specific PID.
/// `mode` = 0 (None), 1 (Lazy), 2 (Immediate).
pub fn sys_sched_setpreempt(pid: u32, mode: usize) -> Result<usize, &'static str> {
    let mode_enum = match mode {
        0 => PreemptMode::None,
        1 => PreemptMode::Lazy,
        2 => PreemptMode::Immediate,
        _ => return Err("invalid preempt mode"),
    };

    // Find the target thread.
    if pid == 0 {
        // Calling thread
        let current = current_thread().ok_or("no thread")?;
        set_preempt_mode(current, mode_enum);
        Ok(0)
    } else {
        // Find process by PID — iterate through processes to locate its thread.
        let procs = crate::proc::PROCESSES.lock();
        for p in procs.iter() {
            if p.pid == pid {
                if let Some(ref thread) = p.thread {
                    // Use a raw pointer to the thread inside the Process.
                    let thread_ptr = thread as *const Thread as *mut Thread;
                    set_preempt_mode(thread_ptr, mode_enum);
                    return Ok(0);
                }
                break;
            }
        }
        Err("pid not found")
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// V35 — Proxy Execution (Priority Inheritance via Proxy)
// ═══════════════════════════════════════════════════════════════════════════

/// Begin proxy execution: when thread `blocker` blocks on a lock held by
/// `holder`, we donate the blocker's scheduling context (priority, time slice)
/// to the holder so that it can make progress and release the lock quickly.
///
/// Caller must ensure both pointers are valid (non-null) and that `holder`
/// is the thread currently holding the contended lock.
pub fn proxy_start(blocker: *mut Thread, holder: *mut Thread) {
    unsafe {
        (*blocker).proxy_target = Some(holder);
        (*holder).proxy_donor = Some(blocker);
        // Transfer scheduling context: inherit priority
        let blk_prio = (*blocker).effective_priority;
        let hld_prio = (*holder).effective_priority;
        if blk_prio > hld_prio {
            (*holder).effective_priority = blk_prio;
        }
        // Donate remaining ticks so the holder does not get preempted early
        let blk_ticks = (*blocker).remaining_ticks;
        if blk_ticks > (*holder).remaining_ticks {
            (*holder).remaining_ticks = blk_ticks;
        }
    }
}

/// End proxy execution: the lock has been released by `holder`.
/// Restore the holder's original priority and clear proxy state.
pub fn proxy_end(holder: *mut Thread) {
    unsafe {
        if let Some(donor) = (*holder).proxy_donor {
            (*donor).proxy_target = None;
        }
        (*holder).proxy_donor = None;
        // Restore original priority and time slice
        (*holder).effective_priority = (*holder).base_priority;
    }
}

/// Resolve proxy chain during scheduling decisions.
/// If the current thread has a proxy_donor waiting, return `Some(current)`
/// so the scheduler can account for the inherited priority.
/// Returns `None` if there is no active proxy relationship.
pub fn resolve_proxy() -> Option<*mut Thread> {
    let current = current_thread()?;
    unsafe {
        if (*current).proxy_donor.is_some() {
            // This thread is acting as a proxy; its effective priority
            // already reflects the donor's priority.
            Some(current)
        } else {
            None
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// V35 — Time Slice Extension
// ═══════════════════════════════════════════════════════════════════════════

/// Number of timer ticks added per extension request (~50 ms at 10 ms/tick).
const SLICE_EXTENSION_TICKS: u64 = 5;
/// Maximum extensions per scheduling quantum (prevents starvation).
const SLICE_EXTENSION_MAX: u64 = 3;

/// Request a time slice extension for the calling thread.
/// Can be called from user-space (via rseq or similar) to prevent
/// preemption during a short critical section.
///
/// Only works when `slice_extension_enabled` is true for the thread
/// and the extension count has not exceeded `SLICE_EXTENSION_MAX`.
pub fn request_slice_extension() {
    let current = match current_thread() {
        Some(t) => t,
        None => return,
    };
    unsafe {
        if (*current).slice_extension_enabled
            && (*current).slice_extension_count < SLICE_EXTENSION_MAX
        {
            (*current).remaining_ticks =
                (*current).remaining_ticks.saturating_add(SLICE_EXTENSION_TICKS);
            (*current).slice_extension_count += 1;
        }
    }
}

/// Syscall: enable or disable time slice extension for the calling process.
pub fn sys_set_slice_ext(enable: bool) -> Result<usize, &'static str> {
    let current = current_thread().ok_or("no thread")?;
    unsafe {
        (*current).slice_extension_enabled = enable;
        if !enable {
            (*current).slice_extension_count = 0;
        }
    }
    Ok(0)
}

pub fn sched_stats() {
    SCHED_LOCK.lock();
    unsafe {
        for hart in 0..crate::per_cpu::hart_count() {
            crate::console::puts("  HART ");
            // print hart number
            let c = b'0' + hart as u8;
            core::arch::asm!("ecall", in("a7") 1usize, in("a0") c as usize);
            crate::console::puts(" picks=");
            let n = SCHEDULER.pick_count[hart];
            let mut buf = [0u8; 10];
            let mut i = 10;
            let mut m = n;
            loop {
                i -= 1;
                buf[i] = b'0' + (m - (m / 10) * 10) as u8;
                m /= 10;
                if m == 0 {
                    break;
                }
            }
            for &b in buf[i..].iter() {
                core::arch::asm!("ecall", in("a7") 1usize, in("a0") b as usize);
            }
            crate::console::puts("\r\n");
        }
    }
    SCHED_LOCK.unlock();
}

// ═══════════════════════════════════════════════════════════════════════════
// V38c — Zicond-Optimized Scheduler Hot Paths
// ═══════════════════════════════════════════════════════════════════════════
//
// These functions use branchless conditional move (Zicond) to accelerate
// scheduler comparisons that are executed on every scheduling decision.
// Eliminating branches avoids pipeline stalls from mispredictions.

/// Branchless comparison: return the thread with higher effective priority.
///
/// If priorities are equal, return the thread with the earlier deadline.
/// Uses Zicond to avoid conditional branches in the scheduler hot path.
pub fn pick_higher_priority_thread(a: *mut Thread, b: *mut Thread) -> *mut Thread {
    unsafe {
        let prio_a = (*a).effective_priority as u64;
        let prio_b = (*b).effective_priority as u64;
        let deadline_a = (*a).deadline;
        let deadline_b = (*b).deadline;

        // Use branchless selection: select 'a' if should_select_thread is true
        let select_a = should_select_thread(prio_a, prio_b, deadline_a, deadline_b);
        cmov_u64(select_a, a as u64, b as u64) as *mut Thread
    }
}

/// Branchless check if thread `a` has strictly higher priority than `b`.
///
/// Returns true if `a` has higher priority, or same priority but earlier deadline.
#[inline]
pub fn is_higher_priority(a: *mut Thread, b: *mut Thread) -> bool {
    unsafe {
        let prio_a = (*a).effective_priority as u64;
        let prio_b = (*b).effective_priority as u64;
        let deadline_a = (*a).deadline;
        let deadline_b = (*b).deadline;
        should_select_thread(prio_a, prio_b, deadline_a, deadline_b)
    }
}

/// Branchless clamp for thread priority to valid range [0, 63].
#[inline]
pub fn clamp_priority(prio: u64) -> u8 {
    crate::mem::zicond::clamp_u64(prio, 0, 63) as u8
}

/// Branchless compute time slice from weight.
/// Avoids branching on the weight > 0 check.
#[inline]
pub fn compute_slice_ticks(weight: u32) -> u64 {
    // slice = SLICE_TICKS * WEIGHT_MAX / max(weight, 1)
    let w = crate::mem::zicond::cmov_u64((weight as u64) < 1, 1, weight as u64);
    let slice_ticks: u64 = 1; // SLICE_TICKS
    let weight_max: u64 = 512; // WEIGHT_MAX
    slice_ticks * weight_max / w
}

/// Record scheduler Zicond optimization initialization.
pub fn sched_zicond_init() {
    if crate::mem::zicond::zicond_available() {
        crate::println!("  V38c: Scheduler hot paths optimized with Zicond");
    }
}
