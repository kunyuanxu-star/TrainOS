//! Processor management
//!
//! Manages the current running task on this CPU

use super::task::{TaskControlBlock, TaskId};
use spin::{Mutex, MutexGuard};

/// Per-CPU state
pub struct Processor {
    /// Currently running task
    current: Option<TaskId>,
    /// Idle task for when no tasks are runnable
    idle_task: TaskControlBlock,
}

impl Processor {
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task: TaskControlBlock::new(0),
        }
    }

    /// Get the current task ID
    pub fn current_id(&self) -> Option<TaskId> {
        self.current
    }

    /// Set the current task
    pub fn set_current(&mut self, id: TaskId) {
        self.current = Some(id);
    }

    /// Get a reference to the idle task
    pub fn idle_task(&self) -> &TaskControlBlock {
        &self.idle_task
    }

    /// Check if we have a current task
    pub fn has_current(&self) -> bool {
        self.current.is_some()
    }
}

/// Global processor instance - use Option to allow lazy initialization
static PROCESSOR: Mutex<Option<Processor>> = Mutex::new(None);

/// Get the global processor instance
pub fn get_processor() -> &'static Mutex<Option<Processor>> {
    &PROCESSOR
}

/// Get or initialize the processor
pub fn get_or_init_processor() -> MutexGuard<'static, Option<Processor>> {
    let mut guard = PROCESSOR.lock();
    if guard.is_none() {
        *guard = Some(Processor::new());
    }
    guard
}

/// Get current task ID
pub fn current_task_id() -> Option<TaskId> {
    get_or_init_processor().as_ref()?.current_id()
}
