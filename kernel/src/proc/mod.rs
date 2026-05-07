pub mod process;
pub mod thread;
pub mod switch;
pub mod elf;

use process::Process;
use thread::Thread;
use crate::mem::{buddy, sv39, layout::PAGE_SIZE};
use alloc::boxed::Box;
use spin::Mutex;
use alloc::vec::Vec;

static NEXT_PID: Mutex<u32> = Mutex::new(1);
static PROCESSES: Mutex<Vec<Box<Process>>> = Mutex::new(Vec::new());

pub fn spawn(elf_data: &[u8], priority: u8) -> Option<u32> {
    let pid = {
        let mut next = NEXT_PID.lock();
        let pid = *next;
        *next += 1;
        pid
    };

    // Allocate page table root for the process
    let root_pt = buddy::alloc_page()?;

    // Copy kernel L2 entries into the process page table so kernel
    // memory remains accessible after satp is switched to this PT.
    unsafe { sv39::copy_kernel_mappings(root_pt); }

    // Compute satp value for this process
    let satp_val = sv39::make_satp(root_pt);

    // Load ELF into the process address space
    let (entry, user_sp) = elf::load_elf(elf_data, root_pt)?;

    // Allocate kernel stack (physical page). Write trap frame onto its top.
    let kstack_pa = buddy::alloc_page()?;
    let tf_sp = sv39::pa_to_kva(kstack_pa + PAGE_SIZE) - 280;

    // Create thread. tf_sp points to where the trap frame sits on the stack.
    let mut thread = Thread::new(pid, pid, priority, entry, tf_sp, satp_val);
    if let Some(ref mut tf) = thread.trap_frame {
        tf.user_sp = user_sp;
    }
    // Copy the trap frame onto the kernel stack at tf_sp
    unsafe {
        (tf_sp as *mut crate::trap::TrapFrame).write(thread.trap_frame.unwrap());
    }

    let mut proc = Box::new(Process::new(pid, priority, root_pt));
    proc.thread = Some(thread);
    let thread_ptr: *mut Thread = proc.thread.as_mut().unwrap() as *mut Thread;

    let mut procs = PROCESSES.lock();
    procs.push(proc);

    crate::sched::enqueue_thread(thread_ptr);
    Some(pid)
}

pub fn init() {}
