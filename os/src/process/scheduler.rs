//! Scheduler - Multi-Level Feedback Queue (MLFQ)
//!
//! A preemptive scheduler with multiple priority queues
//! - 4 priority levels (0-3, lower is higher priority)
//! - Time slices: priority 0 = 20ms, 1 = 40ms, 2 = 80ms, 3 = 160ms
//! - Aging: tasks move to lower priority after waiting

use super::task::{TaskControlBlock, TaskId, TaskStatus};
use spin::Mutex;

/// Number of priority queues
const NUM_QUEUES: usize = 4;
/// Maximum tasks per queue
const MAX_TASKS_PER_QUEUE: usize = 16;
/// Time slice sizes in milliseconds for each priority level
const TIME_SLICES: [usize; NUM_QUEUES] = [20, 40, 80, 160];
/// Maximum ticks a task can stay in a queue before aging
const MAX_TICKS_BEFORE_AGING: usize = 8;

/// Task wrapper for scheduling with priority and time slice info
#[derive(Clone, Copy)]
pub struct SchedTask {
    pub tcb: TaskControlBlock,
    pub priority: usize,      // Current priority (0-3)
    pub time_slice: usize,    // Remaining time in current slice (ms)
    pub wait_ticks: usize,    // Ticks spent waiting in ready queue
}

impl SchedTask {
    pub fn new(tcb: TaskControlBlock) -> Self {
        Self {
            tcb,
            priority: 1,  // Default to priority 1
            time_slice: TIME_SLICES[1],
            wait_ticks: 0,
        }
    }

    /// Reset time slice for current priority
    pub fn reset_time_slice(&mut self) {
        self.time_slice = TIME_SLICES[self.priority];
    }

    /// Move task to lower priority (higher number)
    pub fn decrease_priority(&mut self) {
        if self.priority < NUM_QUEUES - 1 {
            self.priority += 1;
            self.reset_time_slice();
        }
    }

    /// Move task to higher priority (lower number) - aging
    pub fn increase_priority(&mut self) {
        if self.priority > 0 {
            self.priority -= 1;
            self.reset_time_slice();
        }
    }
}

/// A simple ring buffer queue for tasks
pub struct TaskQueue {
    tasks: [Option<SchedTask>; MAX_TASKS_PER_QUEUE],
    head: usize,
    tail: usize,
    count: usize,
}

impl TaskQueue {
    pub const fn new() -> Self {
        Self {
            tasks: [None; MAX_TASKS_PER_QUEUE],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    pub fn push(&mut self, task: SchedTask) -> bool {
        if self.count >= MAX_TASKS_PER_QUEUE {
            return false;
        }
        self.tasks[self.tail] = Some(task);
        self.tail = (self.tail + 1) % MAX_TASKS_PER_QUEUE;
        self.count += 1;
        true
    }

    pub fn pop(&mut self) -> Option<SchedTask> {
        if self.count == 0 {
            return None;
        }
        let task = self.tasks[self.head].take();
        self.head = (self.head + 1) % MAX_TASKS_PER_QUEUE;
        self.count -= 1;
        task
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn len(&self) -> usize {
        self.count
    }

    /// Iterate and modify each task (for aging)
    pub fn for_each<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut SchedTask),
    {
        let mut i = self.head;
        for _ in 0..self.count {
            if let Some(ref mut task) = self.tasks[i] {
                f(task);
            }
            i = (i + 1) % MAX_TASKS_PER_QUEUE;
        }
    }
}

/// Multi-Level Feedback Queue Scheduler
pub struct Scheduler {
    /// Priority queues
    queues: [TaskQueue; NUM_QUEUES],
    /// Current task executing on this CPU
    current: Option<SchedTask>,
    /// Total number of tasks
    task_count: usize,
    /// Next available task ID
    next_id: usize,
    /// Current tick (for accounting)
    current_tick: usize,
}

impl Scheduler {
    /// Create a new scheduler
    pub const fn new() -> Self {
        Self {
            queues: [
                TaskQueue::new(),
                TaskQueue::new(),
                TaskQueue::new(),
                TaskQueue::new(),
            ],
            current: None,
            task_count: 0,
            next_id: 1,  // ID 0 reserved for idle
            current_tick: 0,
        }
    }

    /// Add a new task to the ready queue
    pub fn add_task(&mut self, mut task: TaskControlBlock) -> Option<TaskId> {
        task.id = TaskId::new(self.next_id);
        self.next_id += 1;
        self.task_count += 1;

        task.status = TaskStatus::Ready;
        let sched_task = SchedTask::new(task);

        // Insert into appropriate priority queue (default priority 1)
        if self.queues[1].push(sched_task) {
            Some(TaskId::new(self.next_id - 1))
        } else {
            None
        }
    }

    /// Add task with specific priority (for fork with inherited priority)
    pub fn add_task_with_priority(&mut self, mut task: TaskControlBlock, priority: usize) -> Option<TaskId> {
        let pri = priority.min(NUM_QUEUES - 1);
        task.id = TaskId::new(self.next_id);
        self.next_id += 1;
        self.task_count += 1;

        task.status = TaskStatus::Ready;
        let mut sched_task = SchedTask::new(task);
        sched_task.priority = pri;
        sched_task.reset_time_slice();

        if self.queues[pri].push(sched_task) {
            Some(TaskId::new(self.next_id - 1))
        } else {
            None
        }
    }

    /// Get next task to run (MLFQ)
    pub fn fetch_task(&mut self) -> Option<SchedTask> {
        // First, do aging - move tasks that have waited too long
        self.do_aging();

        // Find highest priority non-empty queue
        for i in 0..NUM_QUEUES {
            if let Some(task) = self.queues[i].pop() {
                return Some(task);
            }
        }
        None
    }

    /// Perform aging - boost priority of tasks that have waited long
    fn do_aging(&mut self) {
        for q in &mut self.queues {
            q.for_each(|task| {
                task.wait_ticks += 1;
                if task.wait_ticks >= MAX_TICKS_BEFORE_AGING {
                    // Move to higher priority queue
                    if task.priority > 0 {
                        task.priority -= 1;
                        task.reset_time_slice();
                        task.wait_ticks = 0;
                    }
                }
            });
        }
    }

    /// Called on timer tick - decrement current task's time slice
    /// Returns true if preemption should occur
    pub fn on_tick(&mut self) -> bool {
        self.current_tick += 1;

        if let Some(ref mut current) = self.current {
            if current.time_slice > 0 {
                current.time_slice = current.time_slice.saturating_sub(1);
                if current.time_slice <= 0 {
                    // Time slice exhausted - preempt
                    current.decrease_priority();
                    return true;
                }
            }
        }
        false
    }

    /// Set the current running task
    pub fn set_current(&mut self, task: SchedTask) {
        self.current = Some(task);
    }

    /// Get current task
    pub fn get_current(&self) -> Option<&SchedTask> {
        self.current.as_ref()
    }

    /// Get current task mutable
    pub fn get_current_mut(&mut self) -> Option<&mut SchedTask> {
        self.current.as_mut()
    }

    /// Yield the CPU - put current task back in queue
    pub fn yield_current(&mut self) {
        if let Some(mut current) = self.current.take() {
            current.tcb.status = TaskStatus::Ready;
            current.reset_time_slice();
            current.wait_ticks = 0;
            let _ = self.queues[current.priority].push(current);
        }
    }

    /// Block the current task (waiting for I/O, etc.)
    pub fn block_current(&mut self) {
        if let Some(mut current) = self.current.take() {
            current.tcb.status = TaskStatus::Blocked;
            // Don't re-add to queue - blocked tasks need explicit wakeup
        }
    }

    /// Wake up a blocked task
    pub fn wake_task(&mut self, _task_id: TaskId) {
        // Search all queues and blocked list
        // For simplicity, blocked tasks are tracked in TaskManager
        // This is a placeholder - actual wakeup would be done by the blocker
    }

    /// Allocate a new task ID
    pub fn alloc_task_id(&mut self) -> TaskId {
        let id = self.next_id;
        self.next_id += 1;
        TaskId::new(id)
    }

    /// Get total ready task count
    pub fn ready_count(&self) -> usize {
        let mut count = 0;
        for q in &self.queues {
            count += q.len();
        }
        if self.current.is_some() {
            count += 1;
        }
        count
    }

    /// Get number of tasks in system
    pub fn task_count(&self) -> usize {
        self.task_count
    }
}

/// Global scheduler instance
static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

/// Get the global scheduler
pub fn get_scheduler() -> &'static Mutex<Scheduler> {
    &SCHEDULER
}
