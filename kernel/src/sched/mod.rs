use crate::proc::switch::context_switch;
use crate::proc::thread::{Thread, ThreadState};
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
    unsafe {
        (*core::ptr::addr_of_mut!(SCHEDULER)).current = next;
    }
    SCHED_LOCK.unlock();

    // Context switch OUTSIDE the lock to avoid deadlocks
    unsafe {
        match (current_ptr, next) {
            (Some(old), Some(new)) => {
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
