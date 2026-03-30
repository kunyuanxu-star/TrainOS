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
    Zombie,  // Task exited but not yet reaped by parent
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
    /// Is this a user task (vs kernel thread)?
    pub is_user_task: bool,
    /// Children list (PIDs of child processes)
    pub children: [Option<usize>; 16],
    /// Number of children
    pub child_count: usize,
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
            heap_start: crate::syscall::memory::INITIAL_BRK,
            heap_end: crate::syscall::memory::INITIAL_BRK,
            is_user_task: false,
            children: [None; 16],
            child_count: 0,
        }
    }

    /// Create a new user task
    pub fn new_user_task(id: usize, pc: usize, sp: usize, satp: usize) -> Self {
        let mut task = Self::new(id);
        task.user_pc = pc;
        task.user_sp = sp;
        task.satp = satp;
        task.status = TaskStatus::Ready;
        task.is_user_task = true;
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

    pub fn set_zombie(&mut self, code: i32) {
        self.status = TaskStatus::Zombie;
        self.exit_code = Some(code);
    }

    pub fn set_exited(&mut self, code: i32) {
        self.status = TaskStatus::Exited;
        self.exit_code = Some(code);
    }

    /// Allocate kernel stack for this task
    /// Also reserves space for the trap frame at the top of the stack
    pub fn alloc_kernel_stack(&mut self) {
        // Allocate a page for kernel stack
        if let Some(addr) = crate::memory::allocator::alloc_page() {
            // The kernel stack grows down, trap frame is at the top
            let stack_top = addr + PAGE_SIZE;
            self.kernel_sp = stack_top;

            // Reserve space for trap frame at top of kernel stack
            // We'll use the top portion for the trap frame
            let trap_frame_size = core::mem::size_of::<crate::process::context::TrapFrame>();
            self.trap_frame = (stack_top - trap_frame_size) as *mut TrapFrame;

            // Initialize the trap frame to zero
            unsafe {
                core::ptr::write_bytes(self.trap_frame as *mut u8, 0, trap_frame_size);
            }
        }
    }

    /// Create a new user address space for this task
    pub fn create_user_address_space(&mut self) -> bool {
        if let Some(mut user_space) = crate::memory::Sv39::UserAddressSpace::new() {
            // Set up user stack
            if let Ok(stack_top) = user_space.setup_user_stack() {
                self.user_sp = stack_top;
            }

            // Set the SATP for this address space
            self.satp = user_space.get_satp();
            self.is_user_task = true;
            true
        } else {
            false
        }
    }

    /// Set up the trap frame for user mode entry
    pub fn setup_trap_frame(&mut self, entry: usize, sp: usize, arg0: usize) {
        if !self.trap_frame.is_null() {
            let mut tf = crate::process::context::TrapFrame::new_user_entry(entry, sp, arg0);
            // SPP = 0 (user mode), SPIE = 1, SIE = 0
            tf.sstatus = 0x00000020;
            unsafe {
                core::ptr::write(self.trap_frame, tf);
            }
            self.user_pc = entry;
            self.user_sp = sp;
        }
    }

    /// Add a child process ID
    pub fn add_child(&mut self, pid: usize) {
        if self.child_count < 16 {
            self.children[self.child_count] = Some(pid);
            self.child_count += 1;
        }
    }

    /// Remove a child (when it exits)
    pub fn remove_child(&mut self, pid: usize) {
        for i in 0..self.child_count {
            if self.children[i] == Some(pid) {
                self.children[i] = None;
                break;
            }
        }
    }

    /// Get parent PID
    pub fn get_parent_pid(&self) -> Option<usize> {
        self.parent_id.map(|id| id.as_usize())
    }
}

/// Page size for stacks
pub const PAGE_SIZE: usize = 4096;

/// Task manager - manages all tasks in the system
pub struct TaskManager {
    /// Maximum number of tasks
    max_tasks: usize,
    /// Task array (owned, not borrowed)
    tasks: [Option<TaskControlBlock>; 64],
}

impl TaskManager {
    /// Create a new task manager (const-compatible)
    pub const fn new() -> Self {
        Self {
            max_tasks: 64,
            tasks: [None; 64],
        }
    }

    /// Initialize the idle task at index 0
    pub fn init_idle_task(&mut self) {
        if self.tasks[0].is_none() {
            self.tasks[0] = Some(TaskControlBlock::new(0));
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
