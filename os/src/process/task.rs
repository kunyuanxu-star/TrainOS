//! Task Control Block
//!
//! Represents a single task/thread in the system

use crate::process::context::{TaskContext, TrapFrame};

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
    /// User stack pointer
    pub user_sp: usize,
    /// User program counter
    pub user_pc: usize,
    /// Kernel stack pointer
    pub kernel_sp: usize,
    /// Physical address of the page table (satp)
    pub satp: usize,
    /// Parent task ID
    pub parent_id: Option<TaskId>,
    /// Exit code (if exited)
    pub exit_code: Option<i32>,
    /// Task context for context switching
    pub ctx: TaskContext,
    /// Trap frame pointer (for returning to user mode)
    pub trap_frame: *mut TrapFrame,
    /// Kernel heap start
    pub heap_start: usize,
    /// Kernel heap end
    pub heap_end: usize,
}

impl TaskControlBlock {
    pub fn new(id: usize) -> Self {
        Self {
            id: TaskId::new(id),
            status: TaskStatus::Ready,
            user_sp: 0,
            user_pc: 0,
            kernel_sp: 0,
            satp: 0,
            parent_id: None,
            exit_code: None,
            ctx: TaskContext::new(0, 0),
            trap_frame: core::ptr::null_mut(),
            heap_start: 0,
            heap_end: 0,
        }
    }

    /// Create a new user task
    pub fn new_user_task(id: usize, pc: usize, sp: usize, satp: usize) -> Self {
        let mut task = Self::new(id);
        task.user_pc = pc;
        task.user_sp = sp;
        task.satp = satp;
        task.status = TaskStatus::Ready;
        task
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

    /// Allocate kernel stack for this task
    pub fn alloc_kernel_stack(&mut self) {
        // Allocate a page for kernel stack
        if let Some(addr) = crate::memory::allocator::alloc_page() {
            self.kernel_sp = addr + PAGE_SIZE;
        }
    }
}

/// Page size for stacks
const PAGE_SIZE: usize = 4096;

/// Task manager - manages all tasks in the system
pub struct TaskManager {
    /// Maximum number of tasks
    max_tasks: usize,
    /// Task array
    tasks: &'static mut [Option<TaskControlBlock>],
}

impl TaskManager {
    /// Create a new task manager
    pub fn new() -> Self {
        // This is a simplified version - in a real OS we would
        // allocate this from kernel heap
        static mut TASK_SPACE: [Option<TaskControlBlock>; 64] = [None; 64];
        Self {
            max_tasks: 64,
            tasks: unsafe { &mut TASK_SPACE },
        }
    }

    /// Add a new task
    pub fn add_task(&mut self, task: TaskControlBlock) -> Option<usize> {
        for i in 1..self.max_tasks {
            if self.tasks[i].is_none() {
                self.tasks[i] = Some(task);
                return Some(i);
            }
        }
        None
    }

    /// Get a task by ID
    pub fn get_task(&self, id: usize) -> Option<&TaskControlBlock> {
        if id < self.max_tasks {
            self.tasks[id].as_ref()
        } else {
            None
        }
    }

    /// Get a mutable task by ID
    pub fn get_task_mut(&mut self, id: usize) -> Option<&mut TaskControlBlock> {
        if id < self.max_tasks {
            self.tasks[id].as_mut()
        } else {
            None
        }
    }

    /// Remove a task
    pub fn remove_task(&mut self, id: usize) {
        if id < self.max_tasks {
            self.tasks[id] = None;
        }
    }

    /// Get the idle task (always runnable)
    pub fn get_idle_task(&self) -> Option<&TaskControlBlock> {
        self.tasks.get(0).and_then(|t| t.as_ref())
    }

    /// Get next ready task (simple round-robin)
    pub fn get_next_ready(&self) -> Option<&TaskControlBlock> {
        for i in 1..self.max_tasks {
            if let Some(ref task) = self.tasks[i] {
                if task.status == TaskStatus::Ready {
                    return Some(task);
                }
            }
        }
        None
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: TaskControlBlock is designed to be shared between threads via Mutex
// The trap_frame pointer is only accessed from the CPU that owns the task
unsafe impl Send for TaskControlBlock {}
unsafe impl Sync for TaskControlBlock {}
