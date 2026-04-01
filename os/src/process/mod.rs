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
    let mut scheduler = GLOBAL_SCHEDULER.lock();
    let current_tcb_opt = CURRENT_TASK.lock().take();

    // Check if we should preempt (time slice exhausted)
    let should_preempt = scheduler.on_tick();

    if !should_preempt {
        // Put current task back if we aren't preempting
        if let Some(t) = current_tcb_opt {
            *CURRENT_TASK.lock() = Some(t);
        }
        return;
    }

    if current_tcb_opt.is_none() {
        return;
    }
    let mut current_tcb = current_tcb_opt.unwrap();

    // IMPORTANT: Save the context pointer BEFORE moving current_tcb to scheduler
    // This is the bug fix - we need the pointer while current_tcb is still valid
    let saved_ctx_ptr = &mut current_tcb.ctx as *mut context::TaskContext;

    // Fetch next task
    let next_opt = scheduler.fetch_task();
    if next_opt.is_none() {
        // No next task, put current back
        *CURRENT_TASK.lock() = Some(current_tcb);
        return;
    }

    let mut next_tcb = next_opt.unwrap().tcb;

    // Initialize next task's context if it's new (never run before, ctx.ra == 0)
    if next_tcb.ctx.ra == 0 {
        context::init_task_context(&mut next_tcb.ctx, next_tcb.user_pc, next_tcb.kernel_sp);
    }

    // Save current task's context and put it back in scheduler queue
    current_tcb.status = task::TaskStatus::Ready;
    let sched_task = scheduler::SchedTask::new(current_tcb);
    scheduler.yield_current_with_task(sched_task);

    // Update scheduler's current task
    let mut new_sched_task = scheduler::SchedTask::new(next_tcb);
    new_sched_task.tcb.status = task::TaskStatus::Running;
    scheduler.set_current(new_sched_task);

    // Update current task
    *CURRENT_TASK.lock() = Some(next_tcb);

    drop(scheduler);

    // Perform actual context switch - save current (via saved_ctx_ptr), load next
    unsafe {
        context::context_switch(saved_ctx_ptr, &next_tcb.ctx);
    }
}

/// Main scheduling function - select and run next task
pub fn schedule() {
    let mut scheduler = GLOBAL_SCHEDULER.lock();
    let mut current_task = CURRENT_TASK.lock();

    // Put current task back in queue if it's ready
    if let Some(mut task) = current_task.take() {
        if task.status == task::TaskStatus::Ready {
            task.status = task::TaskStatus::Ready;
            let _sched_task = scheduler::SchedTask::new(task);
            scheduler.yield_current();
        }
    }

    // Fetch next task
    if let Some(mut next) = scheduler.fetch_task() {
        crate::print!("[sched] switch\r\n");
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

    // Take the current task
    let mut current_tcb_opt = CURRENT_TASK.lock().take();

    if current_tcb_opt.is_none() {
        return;
    }

    let mut current_tcb = current_tcb_opt.unwrap();

    // Mark current as ready and put back in scheduler
    current_tcb.status = task::TaskStatus::Ready;
    let sched_task = scheduler::SchedTask::new(current_tcb);
    scheduler.yield_current_with_task(sched_task);

    // Fetch next task from scheduler
    let next_opt = scheduler.fetch_task();
    if next_opt.is_none() {
        // No next task - should not happen if idle task is always runnable
        // Put current back
        if let Some(curr) = scheduler.get_current_mut() {
            curr.tcb.status = task::TaskStatus::Running;
            *CURRENT_TASK.lock() = Some(curr.tcb);
        }
        return;
    }

    let mut next_tcb = next_opt.unwrap().tcb;
    next_tcb.status = task::TaskStatus::Running;

    // Initialize next task's context if it's new (never run before, ctx.ra == 0)
    if next_tcb.ctx.ra == 0 {
        context::init_task_context(&mut next_tcb.ctx, next_tcb.user_pc, next_tcb.kernel_sp);
    }

    // Set next task as current
    *CURRENT_TASK.lock() = Some(next_tcb);
    scheduler.set_current(scheduler::SchedTask::new(next_tcb));

    // Save current task's context pointer (before we lose current_tcb)
    let saved_ctx_ptr = &mut current_tcb.ctx as *mut context::TaskContext;

    drop(scheduler);

    // Perform actual context switch - saves current state to saved_ctx_ptr, loads next
    unsafe {
        context::context_switch(saved_ctx_ptr, &next_tcb.ctx);
    }

    // When context_switch returns, we're back in the previous task
    // The previous task's state was saved to its TCB's ctx field
    // Mark it as ready so it can be scheduled again
    if let Some(ref mut prev_tcb) = *CURRENT_TASK.lock() {
        prev_tcb.status = task::TaskStatus::Ready;
    }
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
/// This task just burns CPU since timer interrupt preemption doesn't work in QEMU
fn idle_task() {
    let mut counter: usize = 0;
    loop {
        counter += 1;
        if counter % 1000 == 0 {
            crate::print!("[idle] idle task running\r\n");
        }
    }
}

/// Test task that cycles
fn test_task() {
    static mut COUNT: usize = 0;
    loop {
        unsafe {
            COUNT += 1;
            if COUNT % 100 == 0 {
                crate::print!("[test] Task cycle\r\n");
            }
        }
    }
}

/// Start the scheduler and run the first user process
fn start_scheduler() {
    crate::println!("[sched] Starting scheduler");

    // Embedded ELF binary for testing
    static HELLO_ELF: &[u8] = include_bytes!("../../../target/riscv64gc-unknown-none-elf/release/hello");

    // Create user address space
    crate::println!("[sched] Creating user address space");
    let mut user_space = match crate::memory::Sv39::UserAddressSpace::new() {
        Some(us) => {
            crate::println!("[sched] User address space created");
            us
        },
        None => {
            crate::println!("[sched] Failed to create user address space");
            loop {}
        }
    };

    // Load ELF
    crate::println!("[sched] Loading ELF");
    let (entry_point, user_sp) = match crate::elf::load_elf(HELLO_ELF, &mut user_space) {
        Ok(result) => {
            crate::println!("[sched] ELF loaded successfully");
            result
        },
        Err(e) => {
            crate::println!("[sched] ELF loading failed");
            loop {}
        }
    };

    // Allocate kernel stack for this process
    crate::println!("[sched] Allocating kernel stack");
    let kernel_stack_page = match crate::memory::allocator::alloc_page() {
        Some(addr) => addr,
        None => {
            crate::println!("[sched] Failed to allocate kernel stack");
            loop {}
        }
    };
    let kernel_sp = kernel_stack_page + 4096;  // Top of kernel stack page

    // Set up trap frame at top of kernel stack
    let trap_frame_size = core::mem::size_of::<crate::process::context::TrapFrame>();
    let trap_frame_ptr = (kernel_sp - trap_frame_size) as *mut crate::process::context::TrapFrame;

    // Initialize trap frame for user mode entry
    unsafe {
        let mut tf = crate::process::context::TrapFrame::new_user_entry(entry_point, user_sp, 0);
        // Set sstatus: SPP=0 (user mode), SPIE=1, SIE=0
        tf.sstatus = 0x00000020;
        core::ptr::write(trap_frame_ptr, tf);
    }

    // Get SATP from user space
    let satp = user_space.get_satp();
    crate::println!("[sched] satp");

    // Set sscratch to point to kernel trap frame
    // This is needed so that when a trap occurs, the CPU can find the kernel stack
    unsafe {
        core::arch::asm!("csrw sscratch, {0}", in(reg) trap_frame_ptr as usize);
    }

    crate::println!("[sched] Returning to user mode");

    // Return to user mode - this never returns
    unsafe {
        crate::print!("[sched] About to call return_to_user\n");
        crate::process::context::return_to_user(
            trap_frame_ptr,
            satp,
            user_sp,
            entry_point
        );
        crate::print!("[sched] return_to_user returned - should not see this\n");
    }

    // Should never reach here
    loop {}
}
