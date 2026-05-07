use crate::proc::thread::{Thread, ThreadState};
use crate::proc::switch::context_switch;
use crate::sync::SpinLock;
use alloc::vec::Vec;

const NUM_PRIORITIES: usize = 64;

/// Simple FIFO queue backed by a Vec
struct ThreadQueue {
    items: Vec<*mut Thread>,
    head: usize,
}

impl ThreadQueue {
    const fn new() -> Self {
        ThreadQueue {
            items: Vec::new(),
            head: 0,
        }
    }

    fn push_back(&mut self, t: *mut Thread) {
        self.items.push(t);
    }

    fn pop_front(&mut self) -> Option<*mut Thread> {
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

    fn is_empty(&self) -> bool {
        self.head >= self.items.len()
    }
}

pub struct Scheduler {
    ready_queues: [ThreadQueue; NUM_PRIORITIES],
    priority_bitmap: u64,
    current: Option<*mut Thread>,
    pick_count: [u64; 4],  // picks per HART
}

impl Scheduler {
    pub const fn new() -> Self {
        const EMPTY: ThreadQueue = ThreadQueue::new();
        Scheduler {
            ready_queues: [EMPTY; NUM_PRIORITIES],
            priority_bitmap: 0,
            current: None,
            pick_count: [0; 4],
        }
    }

    pub fn enqueue(&mut self, thread: *mut Thread) {
        unsafe {
            let pri = (*thread).effective_priority as usize;
            if pri < NUM_PRIORITIES {
                (*thread).state = ThreadState::Ready;
                self.ready_queues[pri].push_back(thread);
                self.priority_bitmap |= 1u64 << pri;
            }
        }
    }

    fn highest_priority(&self) -> Option<usize> {
        if self.priority_bitmap == 0 { return None; }
        Some(63 - (self.priority_bitmap.leading_zeros() as usize))
    }

    fn dequeue_highest(&mut self) -> Option<*mut Thread> {
        let pri = self.highest_priority()?;
        let thread = self.ready_queues[pri].pop_front()?;
        if self.ready_queues[pri].is_empty() {
            self.priority_bitmap &= !(1u64 << pri);
        }
        unsafe { (*thread).state = ThreadState::Running; }
        self.pick_count[crate::per_cpu::hart_id()] += 1;
        Some(thread)
    }

    pub fn current(&self) -> Option<*mut Thread> {
        self.current
    }

    pub fn set_current(&mut self, thread: *mut Thread) {
        self.current = Some(thread);
    }
}

static SCHED_LOCK: SpinLock = SpinLock::new();
static mut SCHEDULER: Scheduler = Scheduler::new();

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
    unsafe { (*core::ptr::addr_of_mut!(SCHEDULER)).current = next; }
    SCHED_LOCK.unlock();

    // Context switch OUTSIDE the lock to avoid deadlocks
    unsafe {
        match (current_ptr, next) {
            (Some(old), Some(new)) => {
                context_switch(&mut (*old).task_ctx, &(*new).task_ctx);
            }
            (None, Some(new)) => {
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
    unsafe { (*core::ptr::addr_of_mut!(SCHEDULER)).enqueue(thread); }
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
        unsafe { (*core::ptr::addr_of_mut!(SCHEDULER)).current = Some(idle); }
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
            unsafe { core::arch::asm!("ecall", in("a7") 1usize, in("a0") c as usize); }
            crate::console::puts(" picks=");
            let n = SCHEDULER.pick_count[hart];
            let mut buf = [0u8; 10];
            let mut i = 10;
            let mut m = n;
            loop {
                i -= 1; buf[i] = b'0' + (m - (m / 10) * 10) as u8;
                m = m / 10; if m == 0 { break; }
            }
            for j in i..10 { unsafe { core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize); } }
            crate::console::puts("\r\n");
        }
    }
    SCHED_LOCK.unlock();
}
