pub mod elf;
pub mod process;
pub mod switch;
pub mod thread;

use crate::cap::ops;
use crate::cap::types::{CapType, ResourceData, Slot};
use crate::mem::{buddy, layout::PAGE_SIZE, sv39};
use alloc::boxed::Box;
use alloc::vec::Vec;
use process::Process;
use spin::Mutex;
use thread::Thread;

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
    loop {
        i -= 1;
        buf[i] = b'0' + (n - (n / 10) * 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    for &b in buf[i..].iter() {
        unsafe { core::arch::asm!("ecall", in("a7") 1usize, in("a0") b as usize); }
    }
    crate::console::puts("\r\n");

    // Allocate page table root for the process
    let root_pt = buddy::alloc_page()?;

    // Copy kernel L2 entries into the process page table so kernel
    // memory remains accessible after satp is switched to this PT.
    unsafe {
        sv39::copy_kernel_mappings(root_pt);
    }

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

    // Create a CNode (capability space) for this process
    let cnode_id = ops::alloc_resource(
        CapType::CNode,
        ResourceData::CNode {
            slots: Mutex::new(alloc::vec::Vec::new()),
        },
    );
    // Pre-allocate 16 null slots in the CNode
    {
        let res = ops::get_resource(cnode_id).unwrap();
        if let ResourceData::CNode { ref slots } = &res.data {
            let mut s = slots.lock();
            for _ in 0..16 {
                s.push(Slot::null());
            }
        }
    }

    let mut proc = Box::new(Process::new(pid, priority, root_pt, cnode_id));
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
pub fn fork_child(
    child_pt: usize,
    _parent_pt: usize,
    entry: usize,
    user_sp: usize,
    _satp_val: usize,
    priority: u8,
) -> Option<u32> {
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

    // Create a new CNode for the child process with empty slots
    let cnode_id = ops::alloc_resource(
        CapType::CNode,
        ResourceData::CNode {
            slots: Mutex::new(alloc::vec::Vec::new()),
        },
    );
    {
        let res = ops::get_resource(cnode_id).unwrap();
        if let ResourceData::CNode { ref slots } = &res.data {
            let mut s = slots.lock();
            for _ in 0..16 {
                s.push(Slot::null());
            }
        }
    }

    let mut proc = Box::new(Process::new(child_pid, priority, child_pt, cnode_id));
    proc.thread = Some(thread);
    let thread_ptr = proc.thread.as_mut().unwrap() as *mut Thread;

    let mut procs = PROCESSES.lock();
    procs.push(proc);

    crate::sched::enqueue_thread(thread_ptr);
    crate::console::puts("fork_child ok pid=");
    let mut n = child_pid as usize;
    let mut buf = [0u8; 10];
    let mut i = 10;
    loop {
        i -= 1;
        buf[i] = b'0' + (n - (n / 10) * 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    for &b in buf[i..].iter() {
        unsafe { core::arch::asm!("ecall", in("a7") 1usize, in("a0") b as usize); }
    }
    crate::console::puts("\r\n");
    Some(child_pid)
}
