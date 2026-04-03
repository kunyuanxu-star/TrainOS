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

    // Try to load and run the embedded user program
    crate::print!("[sched] Attempting to load user program...\r\n");

    // Embedded ELF binary
    static USER_ELF: &[u8] = include_bytes!("../../../os/bin/hello.bin");

    crate::print!("[sched] ELF size: ");
    crate::console::print_dec(USER_ELF.len());
    crate::println!(" bytes");

    // Validate ELF header
    if USER_ELF.len() < 64 {
        crate::println!("[sched] ELF too small");
        loop_idle();
        return;
    } else if USER_ELF[0..4] != [0x7F, b'E', b'L', b'F'] {
        crate::println!("[sched] Invalid ELF magic");
        loop_idle();
        return;
    }

    // Create user address space
    crate::print!("[sched] Creating user address space...\r\n");
    let mut user_space = match crate::memory::Sv39::UserAddressSpace::new() {
        Some(us) => us,
        None => {
            crate::println!("[sched] Failed to create user address space");
            loop_idle();
            return;
        }
    };
    crate::println!("[sched] User address space created");

    // Load ELF into user address space
    crate::print!("[sched] Loading ELF...\r\n");
    let entry_point: usize;
    let user_sp: usize;
    match crate::elf::load_elf(USER_ELF, &mut user_space) {
        Ok((ep, sp)) => {
            entry_point = ep;
            user_sp = sp;
            crate::print!("[sched] ELF loaded: entry=0x");
            crate::console::print_hex(ep);
            crate::print!(", sp=0x");
            crate::console::print_hex(sp);
            crate::println!("");
        }
        Err(e) => {
            crate::print!("[sched] ELF load failed: ");
            match e {
                crate::elf::ElfResult::InvalidFormat => { crate::println!("Invalid format"); }
                crate::elf::ElfResult::Unsupported => { crate::println!("Unsupported"); }
                crate::elf::ElfResult::LoadError => { crate::println!("Load error"); }
                _ => { crate::println!("Unknown error"); }
            }
            loop_idle();
            return;
        }
    }

    // Create a trap frame for user mode
    crate::print!("[sched] Creating trap frame...\r\n");
    let satp = user_space.get_satp();

    // Allocate a page for the trap frame on the kernel stack
    let trap_frame_ptr = allocate_kernel_trap_frame();
    if trap_frame_ptr.is_null() {
        crate::println!("[sched] Failed to allocate trap frame");
        loop_idle();
        return;
    }

    // Initialize trap frame for user mode entry
    unsafe {
        let tf = &mut *trap_frame_ptr;
        tf.ra = 0;
        tf.sp = user_sp;
        tf.gp = 0;
        tf.tp = 0;
        tf.t0 = 0;
        tf.t1 = 0;
        tf.t2 = 0;
        tf.s0 = 0;
        tf.s1 = 0;
        tf.a0 = 0;  // argc
        tf.a1 = 0;   // argv
        tf.a2 = 0;
        tf.a3 = 0;
        tf.a4 = 0;
        tf.a5 = 0;
        tf.a6 = 0;
        tf.a7 = 0;
        tf.s2 = 0;
        tf.s3 = 0;
        tf.s4 = 0;
        tf.s5 = 0;
        tf.s6 = 0;
        tf.s7 = 0;
        tf.s8 = 0;
        tf.s9 = 0;
        tf.s10 = 0;
        tf.s11 = 0;
        tf.t3 = 0;
        tf.t4 = 0;
        tf.t5 = 0;
        tf.t6 = 0;
        tf.sepc = entry_point;
        tf.sstatus = 0x00000020;  // SPP=0 (user), SPIE=1
    }

    crate::println!("[sched] Returning to user mode...");
    crate::print!("[sched] satp=0x");
    crate::console::print_hex(satp);
    crate::println!("");
    crate::print!("[sched] entry=0x");
    crate::console::print_hex(entry_point);
    crate::println!("");
    crate::print!("[sched] sp=0x");
    crate::console::print_hex(user_sp);
    crate::println!("");

    // Debug: print 'Z' and flush before return_to_user
    for c in b"Z" {
        crate::console::sbi_console_putchar_raw(*c as usize);
    }
    crate::console::console_flush();

    crate::println!("[sched] Calling return_to_user...");
    // Return to user mode
    // Note: This switches to the user page table and never returns
    unsafe {
        crate::process::context::return_to_user(
            trap_frame_ptr,
            satp,
            user_sp,
            entry_point
        );
    }
    // Should never reach here
    crate::println!("[sched] ERROR: return_to_user returned!");
    loop {}
}

/// Allocate a trap frame on the kernel stack
fn allocate_kernel_trap_frame() -> *mut crate::process::context::TrapFrame {
    use crate::process::context::TRAP_FRAME_SIZE;

    // Allocate a page for the trap frame
    let page = match crate::memory::allocator::alloc_page() {
        Some(p) => p,
        None => return core::ptr::null_mut(),
    };

    // Zero the page
    unsafe {
        core::ptr::write_bytes(page as *mut u8, 0, 4096);
    }

    (page as *mut crate::process::context::TrapFrame)
}

/// Idle loop when nothing else to do
fn loop_idle() {
    crate::print!("[sched] Entering idle loop\r\n");
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
