use crate::proc::thread::{Thread, ThreadState};
use crate::proc::switch::context_switch;
use alloc::vec::Vec;
use spin::Mutex;

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
}

impl Scheduler {
    pub const fn new() -> Self {
        const EMPTY: ThreadQueue = ThreadQueue::new();
        Scheduler {
            ready_queues: [EMPTY; NUM_PRIORITIES],
            priority_bitmap: 0,
            current: None,
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
        Some(thread)
    }

    pub fn current(&self) -> Option<*mut Thread> {
        self.current
    }

    pub fn set_current(&mut self, thread: *mut Thread) {
        self.current = Some(thread);
    }
}

// Safe because Scheduler is always accessed behind a Mutex
unsafe impl Send for Scheduler {}

static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

/// Main schedule function: switch from current to next ready thread.
pub fn schedule() {
    let mut sched = SCHEDULER.lock();
    let current_ptr = sched.current;

    // Re-enqueue current if still Running
    if let Some(cur) = current_ptr {
        unsafe {
            let cur_state = (*cur).state;
            if cur_state == ThreadState::Running {
                sched.enqueue(cur);
            }
        }
    }

    let next = sched.dequeue_highest();

    sched.current = next;
    drop(sched);

    unsafe {
        match (current_ptr, next) {
            (Some(old), Some(new)) => {
                context_switch(&mut (*old).task_ctx, &(*new).task_ctx);
            }
            (None, Some(new)) => {
                // First schedule: no old thread, just jump to new
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
                // No thread to run -- should not happen
                crate::console::puts("SCHED: no thread!\r\n");
                crate::idle_loop();
            }
        }
    }
}

pub fn enqueue_thread(thread: *mut Thread) {
    SCHEDULER.lock().enqueue(thread);
}

pub fn current_thread() -> Option<*mut Thread> {
    SCHEDULER.lock().current
}

/// Start scheduler with idle thread as initial current.
pub fn start_scheduler(idle: *mut Thread) -> ! {
    {
        let mut sched = SCHEDULER.lock();
        sched.current = Some(idle);
    }
    // Immediately switch to the highest-priority ready thread.
    schedule();
    // Should never reach here.
    crate::console::puts("SCHED: schedule returned unexpectedly!\r\n");
    crate::idle_loop();
}
