//! Task Control Block
//!
//! Represents a single task/thread in the system

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Ready,
    Running,
    Blocked,
    Exited,
}

/// Task ID (PID)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId(usize);

impl TaskId {
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    pub const fn as_usize(&self) -> usize {
        self.0
    }
}

/// Task control block
#[derive(Clone, Copy)]
pub struct TaskControlBlock {
    /// Task ID
    pub id: TaskId,
    /// Task status
    pub status: TaskStatus,
    /// Stack pointer
    pub sp: usize,
    /// Program counter
    pub pc: usize,
    /// Kernel stack pointer
    pub kernel_sp: usize,
    /// Physical address of the page table (satp)
    pub satp: usize,
    /// Parent task ID
    pub parent_id: Option<TaskId>,
    /// Exit code (if exited)
    pub exit_code: Option<i32>,
}

impl TaskControlBlock {
    pub const fn new(id: usize, pc: usize, sp: usize) -> Self {
        Self {
            id: TaskId::new(id),
            status: TaskStatus::Ready,
            sp,
            pc,
            kernel_sp: 0,
            satp: 0,
            parent_id: None,
            exit_code: None,
        }
    }

    pub fn set_running(&mut self) {
        self.status = TaskStatus::Running;
    }

    pub fn set_ready(&mut self) {
        self.status = TaskStatus::Ready;
    }

    pub fn set_blocked(&mut self) {
        self.status = TaskStatus::Blocked;
    }

    pub fn set_exited(&mut self, code: i32) {
        self.status = TaskStatus::Exited;
        self.exit_code = Some(code);
    }
}
