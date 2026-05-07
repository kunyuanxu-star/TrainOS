use crate::proc::thread::{Thread, ThreadState};
use alloc::boxed::Box;

const MAX_HARTS: usize = 4;

pub struct PerCpu {
    pub hart_id: usize,
    pub current: Option<*mut Thread>,
    pub idle: Option<*mut Thread>,
}

static mut PER_CPU: [PerCpu; MAX_HARTS] = [
    PerCpu { hart_id: 0, current: None, idle: None },
    PerCpu { hart_id: 1, current: None, idle: None },
    PerCpu { hart_id: 2, current: None, idle: None },
    PerCpu { hart_id: 3, current: None, idle: None },
];

pub fn this_cpu() -> &'static mut PerCpu {
    let hart = hart_id();
    unsafe { &mut PER_CPU[hart] }
}

pub fn this_cpu_ref() -> &'static PerCpu {
    let hart = hart_id();
    unsafe { &PER_CPU[hart] }
}

pub fn cpu(hart: usize) -> &'static PerCpu {
    unsafe { &PER_CPU[hart] }
}

pub fn hart_id() -> usize {
    let id: usize;
    unsafe { core::arch::asm!("mv {}, tp", out(reg) id); }
    id
}

pub fn init() {
    for hart in 0..MAX_HARTS {
        let idle = Box::new(Thread::new_idle());
        let idle_ptr = Box::into_raw(idle);
        unsafe {
            PER_CPU[hart].idle = Some(idle_ptr);
        }
    }
}

pub fn init_secondary() {
    let hart = hart_id();
    let idle = unsafe { PER_CPU[hart].idle.unwrap() };
    unsafe { (*idle).state = ThreadState::Running; }
    unsafe { PER_CPU[hart].current = Some(idle); }
}
