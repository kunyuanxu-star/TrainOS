//! Scheduler
//!
//! Simple round-robin scheduler

use super::task::{TaskId, TaskControlBlock};
use spin::Mutex;

/// Task wrapper for scheduling
#[derive(Clone, Copy)]
pub struct SchedTask {
    pub tcb: TaskControlBlock,
}

impl SchedTask {
    pub fn new(tcb: TaskControlBlock) -> Self {
        Self { tcb }
    }
}

/// Simple FIFO scheduler
pub struct Scheduler {
    /// Ready queue of tasks (using fixed-size array for no_std)
    ready_queue: [Option<SchedTask>; 64],
    /// Number of tasks in queue
    queue_len: usize,
    /// Next task ID to allocate
    next_id: usize,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            ready_queue: [None; 64],
            queue_len: 0,
            next_id: 1,  // ID 0 is reserved for idle
        }
    }

    /// Add a task to the ready queue
    pub fn add_task(&mut self, task: SchedTask) {
        if self.queue_len < 64 {
            self.ready_queue[self.queue_len] = Some(task);
            self.queue_len += 1;
        }
    }

    /// Get the next task to run (FIFO)
    pub fn fetch_task(&mut self) -> Option<SchedTask> {
        if self.queue_len == 0 {
            None
        } else {
            let task = self.ready_queue[0].take();
            // Shift all tasks
            for i in 0..self.queue_len - 1 {
                self.ready_queue[i] = self.ready_queue[i + 1].take();
            }
            self.queue_len -= 1;
            task
        }
    }

    /// Get the next available task ID
    pub fn alloc_task_id(&mut self) -> TaskId {
        let id = self.next_id;
        self.next_id += 1;
        TaskId::new(id)
    }

    /// Number of ready tasks
    pub fn ready_count(&self) -> usize {
        self.queue_len
    }
}

/// Global scheduler instance - lazy initialized
static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

/// Get the global scheduler
pub fn get_scheduler() -> &'static Mutex<Scheduler> {
    &SCHEDULER
}
