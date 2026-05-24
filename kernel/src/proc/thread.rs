use crate::trap::TrapFrame;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThreadState {
    Ready,
    Running,
    Waiting,
    Dead,
}

#[derive(Clone, Copy, Debug)]
pub enum WaitTarget {
    Endpoint(usize),
    CallReply(usize),
}

/// Saved callee-saved registers for context switch
/// The `satp` field stores the page table root for this thread.
/// context_switch saves/restores satp so that each thread runs with
/// its own page table after a context switch.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TaskContext {
    pub ra: usize,
    pub sp: usize,
    pub s0: usize,
    pub s1: usize,
    pub s2: usize,
    pub s3: usize,
    pub s4: usize,
    pub s5: usize,
    pub s6: usize,
    pub s7: usize,
    pub s8: usize,
    pub s9: usize,
    pub s10: usize,
    pub s11: usize,
    pub satp: usize,
}

impl TaskContext {
    pub const fn empty() -> Self {
        TaskContext {
            ra: 0,
            sp: 0,
            s0: 0,
            s1: 0,
            s2: 0,
            s3: 0,
            s4: 0,
            s5: 0,
            s6: 0,
            s7: 0,
            s8: 0,
            s9: 0,
            s10: 0,
            s11: 0,
            satp: 0,
        }
    }
}

pub const KERNEL_STACK_SIZE: usize = 8192;

#[repr(C)]
pub struct Thread {
    pub tid: u32,
    pub owner: u32, // pid
    pub state: ThreadState,
    pub base_priority: u8,
    pub effective_priority: u8,
    pub task_ctx: TaskContext,
    pub trap_frame: Option<TrapFrame>,
    pub kernel_stack_top: usize,
    pub wait_target: Option<WaitTarget>,
    // V25: NUMA and EEVDF scheduling fields
    pub node_id: u8,      // NUMA node affinity (0 = default)
    pub vruntime: u64,    // EEVDF virtual runtime (incremented per tick)
    pub deadline: u64,    // EEVDF deadline for sorted ready-queue insertion
    pub weight: u32,      // Scheduling weight (derived from priority, 8..512)
}

impl Thread {
    pub fn new(
        tid: u32,
        owner: u32,
        priority: u8,
        entry: usize,
        tf_sp: usize,
        satp_val: usize,
    ) -> Self {
        let mut tf = TrapFrame::default();
        tf.sepc = entry;
        // SPIE=1 (re-enable S-mode interrupts after sret),
        // SUM=1  (Supervisor can access User pages, needed for IPC payload copy)
        tf.sstatus = (1 << 5) | (1 << 18);
        tf.satp = satp_val;

        let task_ctx = TaskContext {
            ra: user_trap_return as *const () as usize,
            sp: tf_sp, // points to trap frame on kernel stack
            satp: satp_val,
            ..TaskContext::empty()
        };

        let w = (priority as u32 + 1) * 8; // map 0..63 to 8..512
        Thread {
            tid,
            owner,
            state: ThreadState::Ready,
            base_priority: priority,
            effective_priority: priority,
            task_ctx,
            trap_frame: Some(tf),
            kernel_stack_top: tf_sp + 280, // original top of kernel stack
            wait_target: None,
            // V25: NUMA / EEVDF defaults
            node_id: 0,
            vruntime: 0,
            deadline: 0,
            weight: w,
        }
    }

    pub fn new_idle() -> Self {
        // Read the current (kernel) satp for idle thread
        let kernel_satp: usize;
        unsafe {
            core::arch::asm!("csrr {}, satp", out(reg) kernel_satp);
        }
        let mut task_ctx = TaskContext::empty();
        task_ctx.satp = kernel_satp;

        Thread {
            tid: 0,
            owner: 0,
            state: ThreadState::Ready,
            base_priority: 0,
            effective_priority: 0,
            task_ctx,
            trap_frame: None,
            kernel_stack_top: 0,
            wait_target: None,
            node_id: 0,
            vruntime: 0,
            deadline: 0,
            weight: 8,
        }
    }
}

extern "C" {
    pub fn user_trap_return();
}
