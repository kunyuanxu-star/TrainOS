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
pub(crate) static PROCESSES: Mutex<Vec<Box<Process>>> = Mutex::new(Vec::new());

pub fn spawn(elf_data: &[u8], priority: u8) -> Option<u32> {
    let pid = {
        let mut next = NEXT_PID.lock();
        let pid = *next;
        *next += 1;
        pid
    };

    crate::console::puts("  spawn pid=");
    let mut n = pid as usize;
    let mut buf = [0u8; 10];
    let mut i = 10;
    loop { i -= 1; buf[i] = b'0' + (n % 10) as u8; n /= 10; if n == 0 { break; } }
    for j in i..10 { unsafe { core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize); } }
    crate::console::puts("\r\n");

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

/// Create a child process from a COW-shared page table.
/// This is the kernel side of fork().
pub fn fork_child(child_pt: usize, _parent_pt: usize, entry: usize, user_sp: usize, _satp_val: usize, priority: u8) -> Option<u32> {
    let child_pid = {
        let mut next = NEXT_PID.lock();
        let pid = *next;
        *next += 1;
        pid
    };

    // Allocate kernel stack for child
    let kstack_pa = buddy::alloc_page()?;
    let tf_sp = sv39::pa_to_kva(kstack_pa + PAGE_SIZE) - 280;

    // Compute child's own satp value
    let child_satp = sv39::make_satp(child_pt);

    // Create child thread with same entry point and user_sp as parent.
    // entry is already sepc + 4 (instruction after ecall).
    // Use same priority so both get scheduled (FIFO within priority level).
    let mut thread = Thread::new(child_pid, child_pid, priority, entry, tf_sp, child_satp);
    if let Some(ref mut tf) = thread.trap_frame {
        tf.user_sp = user_sp;
        tf.a0 = 0; // child returns 0 from fork
    }

    // Copy trap frame to stack
    unsafe {
        (tf_sp as *mut crate::trap::TrapFrame).write(thread.trap_frame.unwrap());
    }

    let mut proc = Box::new(Process::new(child_pid, priority, child_pt));
    proc.thread = Some(thread);
    let thread_ptr = proc.thread.as_mut().unwrap() as *mut Thread;

    let mut procs = PROCESSES.lock();
    procs.push(proc);

    crate::sched::enqueue_thread(thread_ptr);
    crate::console::puts("fork_child ok pid=");
    let mut n = child_pid as usize;
    let mut buf = [0u8; 10];
    let mut i = 10;
    loop { i -= 1; buf[i] = b'0' + (n % 10) as u8; n /= 10; if n == 0 { break; } }
    for j in i..10 { unsafe { core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize); } }
    crate::console::puts("\r\n");
    Some(child_pid)
}
