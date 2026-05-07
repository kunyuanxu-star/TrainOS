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
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TaskContext {
    pub ra: usize,
    pub sp: usize,
    pub s0: usize, pub s1: usize, pub s2: usize, pub s3: usize,
    pub s4: usize, pub s5: usize, pub s6: usize, pub s7: usize,
    pub s8: usize, pub s9: usize, pub s10: usize, pub s11: usize,
}

impl TaskContext {
    pub const fn empty() -> Self {
        TaskContext {
            ra: 0, sp: 0,
            s0: 0, s1: 0, s2: 0, s3: 0,
            s4: 0, s5: 0, s6: 0, s7: 0,
            s8: 0, s9: 0, s10: 0, s11: 0,
        }
    }
}

pub const KERNEL_STACK_SIZE: usize = 8192;

pub struct Thread {
    pub tid: u32,
    pub owner: u32,         // pid
    pub state: ThreadState,
    pub base_priority: u8,
    pub effective_priority: u8,
    pub task_ctx: TaskContext,
    pub trap_frame: Option<TrapFrame>,
    pub kernel_stack_top: usize,
    pub wait_target: Option<WaitTarget>,
}

impl Thread {
    pub fn new(tid: u32, owner: u32, priority: u8, entry: usize, tf_sp: usize, satp_val: usize) -> Self {
        let mut tf = TrapFrame::default();
        tf.sepc = entry;
        tf.sstatus = 1 << 5; // SPIE bit set (enable interrupts after sret)
        tf.satp = satp_val;

        let task_ctx = TaskContext {
            ra: user_trap_return as *const () as usize,
            sp: tf_sp, // points to trap frame on kernel stack
            ..TaskContext::empty()
        };

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
        }
    }

    pub fn new_idle() -> Self {
        Thread {
            tid: 0,
            owner: 0,
            state: ThreadState::Ready,
            base_priority: 0,
            effective_priority: 0,
            task_ctx: TaskContext::empty(),
            trap_frame: None,
            kernel_stack_top: 0,
            wait_target: None,
        }
    }
}

extern "C" {
    pub fn user_trap_return();
}
