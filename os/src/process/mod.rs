//! Process management module
//!
//! Manages tasks/processes and scheduling

pub mod task;
pub mod processor;
pub mod scheduler;
pub mod context;

use spin::Mutex;
use task::{TaskControlBlock, TaskId};
use scheduler::Scheduler;
use context::{TaskContext, TrapFrame};

/// Global task manager - const so can be used in static initialization
static TASK_MANAGER: Mutex<TaskManager> = Mutex::new(TaskManager::new());

/// Global scheduler - const so can be used in static initialization
static GLOBAL_SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

/// Per-CPU current task pointer (using a simple global for single-core initially)
static CURRENT_TASK: Mutex<Option<TaskControlBlock>> = Mutex::new(None);

/// Task Manager - manages all task control blocks
pub struct TaskManager {
    /// Maximum number of tasks
    max_tasks: usize,
    /// Task array (owned, not borrowed)
    tasks: [Option<TaskControlBlock>; 256],
}

impl TaskManager {
    /// Create a new task manager (const-compatible)
    pub const fn new() -> Self {
        Self {
            max_tasks: 256,
            tasks: [None; 256],
        }
    }

    /// Initialize the idle task at index 0
    pub fn init_idle_task(&mut self) {
        if self.tasks[0].is_none() {
            let mut task = TaskControlBlock::new(0);
            task.status = task::TaskStatus::Running;
            // Allocate kernel stack for idle task
            task.alloc_kernel_stack();
            self.tasks[0] = Some(task);
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
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the process management subsystem
pub fn init() {
    crate::println!("[process] Initializing process management...");

    // Initialize task manager with idle task
    let mut manager = TASK_MANAGER.lock();
    manager.init_idle_task();
    drop(manager);

    // Get idle task and set as current
    let manager = TASK_MANAGER.lock();
    if let Some(idle_task) = manager.get_task(0) {
        let mut current = CURRENT_TASK.lock();
        *current = Some(*idle_task);
    }
    drop(manager);

    crate::println!("[process] Task manager initialized");
    crate::println!("[process] OK");
}

/// Get the global task manager
pub fn get_task_manager() -> &'static Mutex<TaskManager> {
    &TASK_MANAGER
}

/// Get the global scheduler
pub fn get_scheduler() -> &'static Mutex<Scheduler> {
    &GLOBAL_SCHEDULER
}

/// Get current task
pub fn get_current_task() -> Option<TaskControlBlock> {
    *CURRENT_TASK.lock()
}

/// Set current task
pub fn set_current_task(task: TaskControlBlock) {
    *CURRENT_TASK.lock() = Some(task);
}

/// Schedule preemption - called from timer interrupt
pub fn schedule_preempt() {
    let mut scheduler = GLOBAL_SCHEDULER.lock();
    let mut current_task = CURRENT_TASK.lock();

    // If there's a current task, yield it
    if let Some(mut task) = current_task.take() {
        task.status = task::TaskStatus::Ready;
        let mut sched_task = scheduler::SchedTask::new(task);
        sched_task.tcb.status = task::TaskStatus::Ready;
        scheduler.yield_current();
    }

    // Check if we should preempt (time slice exhausted)
    if scheduler.on_tick() {
        // Time slice exhausted
        crate::println!("[scheduler] Time slice exhausted, switching tasks");
    }

    // Fetch next task
    if let Some(mut next) = scheduler.fetch_task() {
        next.tcb.status = task::TaskStatus::Running;
        scheduler.set_current(next);
        *current_task = Some(next.tcb);
    }

    drop(scheduler);
    drop(current_task);
}

/// Main scheduling function - select and run next task
pub fn schedule() {
    let mut scheduler = GLOBAL_SCHEDULER.lock();
    let mut current_task = CURRENT_TASK.lock();

    // Put current task back in queue if it's ready
    if let Some(mut task) = current_task.take() {
        if task.status == task::TaskStatus::Ready {
            task.status = task::TaskStatus::Ready;
            let mut sched_task = scheduler::SchedTask::new(task);
            scheduler.yield_current();
        }
    }

    // Fetch next task
    if let Some(mut next) = scheduler.fetch_task() {
        next.tcb.status = task::TaskStatus::Running;
        scheduler.set_current(next);
        *current_task = Some(next.tcb);
    }

    drop(scheduler);
    drop(current_task);
}

/// Create a new process (fork)
pub fn create_process(entry: usize, stack: usize, is_user: bool) -> Option<TaskId> {
    let mut scheduler = GLOBAL_SCHEDULER.lock();
    let task_id = scheduler.alloc_task_id();

    let mut tcb = TaskControlBlock::new(task_id.as_usize());

    // Allocate kernel stack
    tcb.alloc_kernel_stack();

    // Set up trap frame for the new task
    if is_user {
        // User mode task
        let mut tf = TrapFrame::new_user_entry(entry, stack, 0);
        // SPP = 0 (user mode), SPIE = 1, SIE = 0
        tf.sstatus = 0x00000020;
        tcb.trap_frame = core::ptr::null_mut(); // Will be set on first switch
        tcb.user_pc = entry;
        tcb.user_sp = stack;
    } else {
        // Kernel thread
        tcb.kernel_sp = tcb.kernel_sp - core::mem::size_of::<TrapFrame>();
    }

    tcb.status = task::TaskStatus::Ready;

    // Add to scheduler - it returns TaskId directly
    scheduler.add_task(tcb)
}

/// Initialize and run the first process
pub fn run_first_process() -> ! {
    crate::println!("[process] Starting init process...");

    // Create an idle task loop
    // For now, just run in kernel mode
    crate::println!();
    crate::println!("========================================");
    crate::println!("  trainOS is running!");
    crate::println!("========================================");

    // Test basic functionality
    test_basic_syscalls();

    // Enable interrupts and start the scheduler
    start_scheduler();

    // Should never reach here
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// Test basic syscalls
fn test_basic_syscalls() {
    crate::println!("[process] Testing basic syscalls...");

    // Test write
    let msg = b"Hello from trainOS kernel!\n";
    let _ret = crate::syscall::sys_write(1, msg.as_ptr() as usize, msg.len());

    // Test getpid
    let pid = crate::syscall::sys_getpid();
    crate::println!("[process] getpid returned");
    crate::println!("[process] Basic syscalls working!");

    // Test sched_yield
    let _ret = crate::syscall::sys_sched_yield();
    crate::println!("[process] sched_yield working!");
}

/// Start the scheduler and run tasks
fn start_scheduler() {
    crate::println!("[process] Starting scheduler...");

    // Create a simple init task
    if let Some(_tid) = create_process(0x00400000, 0x00400000 + 0x10000, true) {
        crate::println!("[process] Created init task");
    }

    // Set up timer for periodic scheduling
    // Timer is already set up in trap::init() via clint_init()

    crate::println!("[process] Scheduler started");
    crate::println!("[process] Timer interrupts enabled for preemption");
}
