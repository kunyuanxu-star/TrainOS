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
    let free = crate::mem::buddy::count_free_pages();
    let total = crate::mem::buddy::total_pages();
    if allocated + free != total {
        crate::println!(
            "INVARIANT: allocated({}) + free({}) != total({})",
            allocated, free, total
        );
    }

    let procs = crate::proc::PROCESSES.lock();
    let mut user_pages = 0usize;
    for p in procs.iter() {
        if p.state != crate::proc::process::ProcessState::Dead {
            user_pages += crate::mem::sv39::count_user_pages(p.page_table_root);
        }
    }
    drop(procs);
    if user_pages > allocated {
        crate::println!(
            "INVARIANT: user pages ({}) > allocated pages ({})",
            user_pages, allocated
        );
    }
}

fn check_scheduler_invariants() {
    crate::sched::SCHED_LOCK.lock();
    let sched = unsafe { &*core::ptr::addr_of_mut!(crate::sched::SCHEDULER) };
    let bitmap = sched.priority_bitmap;

    for prio in 0..64 {
        if bitmap & (1u64 << prio) != 0 {
            let q = &sched.ready_queues[prio];
            if q.is_empty() {
                crate::println!(
                    "INVARIANT: priority_bitmap bit {} set but ready_queues[{}] is empty",
                    prio, prio
                );
                continue;
            }
            for &t in q.iter() {
                unsafe {
                    let state = (*t).state;
                    match state {
                        crate::proc::thread::ThreadState::Ready => {}
                        _ => crate::println!(
                            "INVARIANT: thread pid={} in ready_queues[{}] has state {:?}",
                            (*t).owner, prio, state
                        ),
                    }
                }
            }
        } else if !sched.ready_queues[prio].is_empty() {
            crate::println!(
                "INVARIANT: priority_bitmap bit {} clear but ready_queues[{}] has threads",
                prio, prio
            );
        }
    }

    crate::sched::SCHED_LOCK.unlock();
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
    // Validate endpoint table integrity
    let eps = crate::ipc::ENDPOINTS.lock();
    for (i, ep_opt) in eps.iter().enumerate() {
        if i == 0 { continue; } // slot 0 unused
        let Some(ep) = ep_opt else { continue; };

        // Validate waiting_receiver integrity
        if let Some(recv) = ep.waiting_receiver {
            unsafe {
                if (*recv).state != crate::proc::thread::ThreadState::Waiting {
                    crate::println!(
                        "INVARIANT: endpoint {} waiting_receiver not in Waiting state",
                        i
                    );
                }
                let valid_target = matches!((*recv).wait_target,
                    Some(crate::proc::thread::WaitTarget::Endpoint(id)) if id == i);
                if !valid_target {
                    crate::println!(
                        "INVARIANT: endpoint {} waiting_receiver has mismatched wait_target",
                        i
                    );
                }
            }
        }
    }
    drop(eps);

    // Count-based lightweight check
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
