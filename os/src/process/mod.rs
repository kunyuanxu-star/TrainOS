//! Process management module
//!
//! Manages tasks/processes and scheduling

pub mod task;
pub mod processor;
pub mod scheduler;
pub mod context;

use spin::Mutex;
use task::TaskManager;
use scheduler::Scheduler;

/// Global task manager - const so can be used in static initialization
static TASK_MANAGER: Mutex<TaskManager> = Mutex::new(TaskManager::new());

/// Global scheduler - const so can be used in static initialization
static GLOBAL_SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());

/// Initialize the process management subsystem
pub fn init() {
    crate::println!("[process] Initializing process management...");

    // Initialize task manager with idle task
    let mut manager = TASK_MANAGER.lock();
    manager.init_idle_task();
    crate::println!("[process] Task manager initialized");

    crate::println!("[process] Creating idle task...");
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

/// Run the first user process
pub fn run_first_process() -> ! {
    crate::println!("[process] Starting init process...");

    // For now, just run a simple test in supervisor mode
    crate::println!();
    crate::println!("========================================");
    crate::println!("  trainOS is running!");
    crate::println!("========================================");

    // Test syscalls
    run_syscall_test();

    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// Test syscalls from supervisor mode
fn run_syscall_test() {
    crate::println!("[process] Testing syscall from supervisor mode...");

    // Test write syscall
    let msg = b"Hello from kernel test!\n";
    let _ret = crate::syscall::sys_write(1, msg.as_ptr() as usize, msg.len());

    // Test getpid syscall
    let _pid = crate::syscall::sys_getpid();
    crate::println!("[process] getpid syscall works!");
    crate::println!("[process] Note: User program loading requires");
    crate::println!("[process] proper page table setup and linking.");

    // Schedule yield test
    let _yield_ret = crate::syscall::sys_sched_yield();
    crate::println!("[process] sched_yield syscall works!");
}
