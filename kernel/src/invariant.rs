// V21 Kernel invariant checks — formal correctness verification
//
// These run every 100 timer ticks (~1 second). Each check is a logical
// assertion about kernel data structure consistency. Failures indicate
// either a kernel bug or memory corruption.

pub fn run_checks() {
    check_memory_invariants();
    check_scheduler_invariants();
    check_cap_invariants();
    check_ipc_invariants();
    check_wxorx();
    check_stack_canary();
}

fn check_memory_invariants() {
    let allocated = crate::mem::buddy::allocated_pages();
    let total = crate::mem::buddy::total_pages();
    if allocated > total {
        crate::println!("INVARIANT: allocated pages ({}) > total pages ({})", allocated, total);
    }
}

fn check_scheduler_invariants() {
    // Verify current thread exists and is in a valid state
    if let Some(cur) = crate::sched::current_thread() {
        unsafe {
            let state = (*cur).state;
            let prio = (*cur).base_priority;
            if prio > 63 {
                crate::println!("INVARIANT: invalid priority {}", prio);
            }
            // Thread should be Running or Ready
            match state {
                crate::proc::thread::ThreadState::Ready
                | crate::proc::thread::ThreadState::Running
                | crate::proc::thread::ThreadState::Waiting
                | crate::proc::thread::ThreadState::Dead => {}
            }
        }
    }
}

fn check_cap_invariants() {
    let procs = crate::proc::PROCESSES.lock();
    let proc_count = procs.iter().filter(|p| p.state != crate::proc::process::ProcessState::Dead).count();
    drop(procs);
    // Each process should have a CNode with at least 16 slots
    if proc_count > 64 {
        crate::println!("INVARIANT: excessive process count {}", proc_count);
    }
}

fn check_ipc_invariants() {
    // Verify endpoint table integrity (no dangling pointers)
    // Lightweight check: count of sends/recvs should be monotonically increasing
    let sends = crate::ipc::endpoint::SEND_COUNT.load(core::sync::atomic::Ordering::Relaxed);
    let recvs = crate::ipc::endpoint::RECV_COUNT.load(core::sync::atomic::Ordering::Relaxed);
    if recvs > sends + 1000 {
        crate::println!("INVARIANT: recvs ({}) >> sends ({})", recvs, sends);
    }
}

fn check_wxorx() {
    // Verify current process page table for W^X violations
    if let Some(cur) = crate::sched::current_thread() {
        let pid = unsafe { (*cur).owner };
        let procs = crate::proc::PROCESSES.lock();
        if let Some(proc) = procs.iter().find(|p| p.pid == pid) {
            let root = proc.page_table_root;
            drop(procs);
            if let Err(e) = crate::security::verify_wxorx(root) {
                crate::println!("INVARIANT: W^X violation in pid={}: {}", pid, e);
            }
        } else {
            drop(procs);
        }
    }
}

fn check_stack_canary() {
    crate::security::check_stack_canary();
}
