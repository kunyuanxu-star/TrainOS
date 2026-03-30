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

/// Flag to request scheduler reschedule
pub static SCHEDULE_REQUESTED: Mutex<bool> = Mutex::new(false);

/// Request a schedule - called from sys_sched_yield
pub fn request_schedule() {
    *SCHEDULE_REQUESTED.lock() = true;
}

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
    crate::println!("[process] Init start");

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

    crate::println!("[process] Init OK");
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

    // Check if we should preempt (time slice exhausted)
    let should_preempt = scheduler.on_tick();

    // If there's a current task, save its state
    if let Some(ref mut task) = *current_task {
        task.status = if should_preempt {
            task::TaskStatus::Ready
        } else {
            task.status
        };

        // Save the trap frame to the current task
        if !trap_frame_ptr.is_null() {
            task.trap_frame = trap_frame_ptr;
        }

        // Put current task back in queue if it was running and should preempt
        if should_preempt {
            let mut sched_task = scheduler::SchedTask::new(*task);
            scheduler.yield_current();
        }
    }

    // Fetch next task
    if let Some(next) = scheduler.fetch_task() {
        let next_tcb = next.tcb;

        // Mark as running
        let mut new_sched_task = scheduler::SchedTask::new(next_tcb);
        new_sched_task.tcb.status = task::TaskStatus::Running;
        scheduler.set_current(new_sched_task);

        // Update current task
        *current_task = Some(next_tcb);

        crate::println!("[scheduler] Switched to task");
        // NOTE: We don't copy trap_frame here - the actual switch happens
        // when handle_trap returns via sret, which uses the trap_frame
        // that was set during the trap entry
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

/// Perform actual task switch from within trap handler
/// This is called after a syscall that requested a schedule (like sys_sched_yield)
/// trap_frame: the current trap frame (on kernel stack)
pub fn do_schedule(trap_frame: *mut context::TrapFrame) {
    let mut scheduler = GLOBAL_SCHEDULER.lock();
    let mut current_task = CURRENT_TASK.lock();

    // Save current task's trap frame
    if let Some(ref mut task) = *current_task {
        // Save the current trap frame state to the task's saved trap frame
        // The trap_frame points to the kernel stack where registers were saved
        if !trap_frame.is_null() {
            task.trap_frame = trap_frame;
        }
        // Mark as ready for next time
        task.status = task::TaskStatus::Ready;
    }

    // Fetch next task
    if let Some(next) = scheduler.fetch_task() {
        let next_tcb = next.tcb;

        // Copy next task's saved trap frame to current trap frame location
        // This way, when we return via sret, we restore next task's state
        if !next_tcb.trap_frame.is_null() && !trap_frame.is_null() {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    next_tcb.trap_frame,
                    trap_frame,
                    1
                );
            }
        }

        // Update current task
        *current_task = Some(next_tcb);

        crate::println!("[scheduler] Switched to task");
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

    // Set up for user or kernel task
    if is_user {
        // Create user address space with page table
        tcb.create_user_address_space();

        // Set up trap frame for user mode entry
        let user_sp = if stack != 0 { stack } else { tcb.user_sp };
        tcb.setup_trap_frame(entry, user_sp, 0);
    } else {
        // Kernel thread - set up trap frame for entry point
        // kernel_sp already points below the trap frame from alloc_kernel_stack
        tcb.setup_kernel_trap_frame(entry);
    }

    tcb.status = task::TaskStatus::Ready;

    // Add to scheduler - it returns TaskId directly
    scheduler.add_task(tcb)
}

/// Initialize and run the first process
pub fn run_first_process() -> ! {
    crate::println!("[run] Starting first process");

    // Start the scheduler
    start_scheduler();

    // Should never reach here
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// Idle task - runs when no other tasks are runnable
fn idle_task() {
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// Test task that cycles and yields
fn test_task() {
    static mut COUNT: usize = 0;
    loop {
        unsafe {
            COUNT += 1;
            if COUNT % 100 == 0 {
                crate::println!("[test] Task cycle");
            }
        }
        // Yield to allow scheduler to switch tasks
        crate::syscall::sys_sched_yield();
    }
}

/// Start the scheduler and run tasks
fn start_scheduler() {
    crate::println!("[sched] Starting scheduler");

    // Create idle task as the only task (kernel thread)
    if let Some(_tid) = create_process(idle_task as usize, 0x80020000, false) {
        crate::println!("[sched] Idle task created");
    }

    // Fetch the first task to run
    let first_task = {
        let mut scheduler = GLOBAL_SCHEDULER.lock();
        scheduler.fetch_task()
    };

    if let Some(sched_task) = first_task {
        let mut tcb = sched_task.tcb;

        // Set as current running task
        tcb.status = task::TaskStatus::Running;
        {
            let mut current = CURRENT_TASK.lock();
            *current = Some(tcb);
        }
        {
            let mut scheduler = GLOBAL_SCHEDULER.lock();
            scheduler.set_current(scheduler::SchedTask::new(tcb));
        }

        // Check if this is a user task or kernel thread
        if tcb.is_user_task {
            // For user tasks, use return_to_user to switch to user mode
            // This requires: trap_frame, satp, sp, pc
            unsafe {
                context::return_to_user(
                    tcb.trap_frame,
                    tcb.satp,
                    tcb.user_sp,
                    tcb.user_pc,
                );
            }
            // Should never reach here
            loop {}
        } else {
            // For kernel threads, use context_switch
            // Initialize TaskContext for first run:
            // ra = entry point (function to call)
            // sp = kernel stack top
            context::init_task_context(&mut tcb.ctx, tcb.user_pc, tcb.kernel_sp);

            // Create a dummy context for the boot code (we won't return to it)
            let mut boot_ctx: context::TaskContext = context::TaskContext::new(0, 0);

            // Perform the actual context switch
            unsafe {
                context::context_switch(&mut boot_ctx, &tcb.ctx);
            }

            // After context_switch returns, we're running in the new task
            // But we shouldn't reach here normally - the task runs until it yields
            loop {
                schedule();
            }
        }
    }

    // Should never reach here if a task was switched to
    loop {
        schedule();
    }
}
