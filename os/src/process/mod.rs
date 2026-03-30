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

/// Wrapper for trap frame pointer that implements Send
/// This is safe because we're running on a single-core system
/// and the trap frame is only accessed from the current CPU
pub struct TrapFramePtr(pub *mut context::TrapFrame);

unsafe impl Send for TrapFramePtr {}

/// Current trap frame pointer - set in trap handler, used by scheduler
pub static CURRENT_TRAP_FRAME: Mutex<TrapFramePtr> = Mutex::new(TrapFramePtr(core::ptr::null_mut()));

/// Kernel stack top for each task - used for context switching
pub static KERNEL_STACK_TOP: Mutex<Option<usize>> = Mutex::new(None);

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
            // Skip kernel stack allocation for now to get boot working
            // TODO: Fix kernel stack allocation
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
    crate::println!("[process] About to lock TASK_MANAGER...");

    // Initialize task manager with idle task
    let mut manager = TASK_MANAGER.lock();
    crate::println!("[process] TASK_MANAGER locked");
    manager.init_idle_task();
    drop(manager);

    crate::println!("[process] About to lock TASK_MANAGER again...");

    // Get idle task and set as current
    let manager = TASK_MANAGER.lock();
    if let Some(idle_task) = manager.get_task(0) {
        let mut current = CURRENT_TASK.lock();
        *current = Some(*idle_task);
        crate::println!("[process] Current task set");
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
/// This performs the actual context switch
pub fn schedule_preempt() {
    // Get the current trap frame (set by trap handler)
    let trap_frame_ptr = {
        let tf = CURRENT_TRAP_FRAME.lock();
        tf.0
    };

    if trap_frame_ptr.is_null() {
        // Not in a trap context, just yield
        schedule();
        return;
    }

    let mut scheduler = GLOBAL_SCHEDULER.lock();
    let mut current_task = CURRENT_TASK.lock();
    let mut kernel_stack_top = KERNEL_STACK_TOP.lock();

    // Check if we should preempt (time slice exhausted)
    let should_preempt = scheduler.on_tick();

    // Get the current task before we modify it
    let current_tid = current_task.as_ref().map(|t| t.id);

    // If there's a current task, save its state
    if let Some(ref mut task) = *current_task {
        task.status = if should_preempt {
            task::TaskStatus::Ready
        } else {
            task.status
        };

        // Save the trap frame to the current task
        // The trap_frame pointer is where the registers were saved on the stack
        // We need to copy it to a stable location in the task's kernel stack
        if !trap_frame_ptr.is_null() {
            task.trap_frame = trap_frame_ptr;
        }

        // Save kernel stack top
        *kernel_stack_top = Some(task.kernel_sp);

        // Put current task back in queue if it was running and should preempt
        if should_preempt {
            let mut sched_task = scheduler::SchedTask::new(*task);
            scheduler.yield_current();
        }
    }

    // Fetch next task
    if let Some(next) = scheduler.fetch_task() {
        // Set up the trap frame for the new task
        // We need to modify the current trap frame to restore the new task's state
        let next_tcb = next.tcb;

        // Copy the new task's trap frame to the current trap frame location
        if !next_tcb.trap_frame.is_null() && !trap_frame_ptr.is_null() {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    next_tcb.trap_frame,
                    trap_frame_ptr,
                    1
                );
            }
        }

        // Set the kernel stack top for the new task
        *kernel_stack_top = Some(next_tcb.kernel_sp);

        // Mark as running
        let mut new_sched_task = scheduler::SchedTask::new(next_tcb);
        new_sched_task.tcb.status = task::TaskStatus::Running;
        scheduler.set_current(new_sched_task);

        // Update current task
        *current_task = Some(next_tcb);

        crate::println!("[scheduler] Switched to task");
    }

    drop(scheduler);
    drop(current_task);
    drop(kernel_stack_top);
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

    // Allocate kernel stack (this also sets up trap_frame)
    tcb.alloc_kernel_stack();

    // Set up trap frame for the new task
    if is_user {
        // User mode task - initialize the trap frame at the reserved location
        // The trap_frame pointer was set by alloc_kernel_stack
        if !tcb.trap_frame.is_null() {
            let tf = TrapFrame::new_user_entry(entry, stack, 0);
            // SPP = 0 (user mode), SPIE = 1, SIE = 0
            let mut tf = tf;
            tf.sstatus = 0x00000020;
            unsafe {
                core::ptr::write(tcb.trap_frame, tf);
            }
        }
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

/// Test task that prints a message
fn test_task() {
    crate::println!("[test] Test task is running!");
    crate::println!("[test] This confirms scheduler is working");
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

    // Create a simple init task (kernel thread for now)
    // User tasks require proper address space setup which we don't have yet
    if let Some(_tid) = create_process(test_task as usize, 0x80020000, false) {
        crate::println!("[process] Created init task");
    }

    // Set up timer for periodic scheduling
    // Timer is already set up in trap::init() via clint_init()

    crate::println!("[process] Scheduler started");
    crate::println!("[process] Timer interrupts enabled for preemption");
}
