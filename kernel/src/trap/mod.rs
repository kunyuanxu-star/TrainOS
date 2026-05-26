pub mod asm;
pub mod sstc;
pub mod aia;

use core::sync::atomic::{AtomicBool, Ordering};

/// Whether Sstc is the active timer source (vs CLINT fallback).
static SSTC_ACTIVE: AtomicBool = AtomicBool::new(false);

/// TrapFrame: register state saved on trap entry.
/// Layout must match asm.rs offsets exactly.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TrapFrame {
    pub ra: usize,      // 0*8
    pub gp: usize,      // 1*8
    pub tp: usize,      // 2*8
    pub t0: usize,      // 3*8 (user_sp at save time)
    pub t1: usize,      // 4*8
    pub t2: usize,      // 5*8
    pub s0: usize,      // 6*8
    pub s1: usize,      // 7*8
    pub a0: usize,      // 8*8
    pub a1: usize,      // 9*8
    pub a2: usize,      // 10*8
    pub a3: usize,      // 11*8
    pub a4: usize,      // 12*8
    pub a5: usize,      // 13*8
    pub a6: usize,      // 14*8
    pub a7: usize,      // 15*8
    pub s2: usize,      // 16*8
    pub s3: usize,      // 17*8
    pub s4: usize,      // 18*8
    pub s5: usize,      // 19*8
    pub s6: usize,      // 20*8
    pub s7: usize,      // 21*8
    pub s8: usize,      // 22*8
    pub s9: usize,      // 23*8
    pub s10: usize,     // 24*8
    pub s11: usize,     // 25*8
    pub t3: usize,      // 26*8
    pub t4: usize,      // 27*8
    pub t5: usize,      // 28*8
    pub t6: usize,      // 29*8
    pub sepc: usize,    // 30*8
    pub sstatus: usize, // 31*8
    pub satp: usize,    // 32*8
    pub user_sp: usize, // 33*8
    pub stval: usize,   // 34*8
}

/// Initialize trap handling: set stvec to __trap_entry.
pub fn init() {
    extern "C" {
        fn __trap_entry();
    }
    unsafe {
        core::arch::asm!("csrw stvec, {}", in(reg) __trap_entry as *const () as usize);
    }
}

/// Enable timer interrupts (sie.STIE).
pub fn enable_timer_interrupt() {
    unsafe {
        core::arch::asm!("csrrs zero, sie, {}", in(reg) 0x20usize);
    }
}

// ── Timer subsystem: Sstc with CLINT fallback ──────────────────────────

/// Initialize the timer subsystem.
///
/// Probes for Sstc availability.  If Sstc is present, uses the stimecmp
/// CSR directly (no SBI or CLINT needed).  Falls back to legacy CLINT
/// MMIO timer when Sstc is not available.
///
/// Called once during boot (hart 0 only).
pub fn timer_init() {
    // Probe Sstc by reading the stimecmp CSR (0x14D).
    // On platforms without Sstc, this would trap. On our target
    // (QEMU virt with recent firmware), Sstc is available.
    #[cfg(not(test))]
    {
        let mut _probe_val: usize;
        unsafe {
            core::arch::asm!("csrr {}, 0x14D", out(reg) _probe_val);
        }
        // CSR accessible — Sstc is available.
        SSTC_ACTIVE.store(true, Ordering::SeqCst);
        sstc::SstcTimer::set_available(true);
        sstc::SstcTimer::enable();
        sstc::SstcTimer::set_timer_delay(TICK_MS as u64 * 1000);
        crate::println!("  Sstc timer initialized (stimecmp CSR)");
    }
}

/// Mark Sstc as unavailable and fall back to CLINT timer.
/// Call this when the Sstc probe fails (e.g., on platforms without Sstc).
pub fn use_clint_fallback() {
    SSTC_ACTIVE.store(false, Ordering::SeqCst);
    sstc::SstcTimer::set_available(false);
    clint_init();
    crate::println!("  CLINT timer initialized (Sstc not available)");
}

// ── CLINT timer (legacy fallback) ───────────────────────────────────────

const CLINT_BASE: usize = 0x0200_0000;
fn clint_mtimecmp_offset() -> usize {
    let hart = crate::per_cpu::hart_id();
    CLINT_BASE + 0x4000 + hart * 8
}
const CLINT_MTIME: usize = CLINT_BASE + 0xBFF8;
const TIMEBASE_FREQ: usize = 10_000_000; // 10MHz
const TICK_MS: usize = 10;
const TICK_TICKS: usize = (TICK_MS * TIMEBASE_FREQ) / 1000;

unsafe fn mtime() -> u64 {
    let ptr = CLINT_MTIME as *const u64;
    ptr.read_volatile()
}

unsafe fn set_mtimecmp(val: u64) {
    let offset = clint_mtimecmp_offset();
    (offset as *mut u64).write_volatile(val);
}

/// Arm the next CLINT timer tick (fallback path).
fn clint_set_next_timer() {
    unsafe {
        let current = mtime();
        set_mtimecmp(current + TICK_TICKS as u64);
    }
}

/// Initialize the CLINT timer (fallback path).
fn clint_init() {
    clint_set_next_timer();
}

/// Arm the next timer tick using whichever source is active.
fn set_next_timer_tick() {
    if SSTC_ACTIVE.load(Ordering::Relaxed) {
        sstc::SstcTimer::set_periodic_tick(TICK_MS as u64 * 1000);
    } else {
        clint_set_next_timer();
    }
}

/// Trap dispatch — called from assembly with TrapFrame pointer in a0.
#[no_mangle]
extern "C" fn handle_trap(tf: &mut TrapFrame) {
    let scause: usize;
    unsafe {
        core::arch::asm!("csrr {}, scause", out(reg) scause);
    }

    let cause = scause & !(1usize << 63);
    let is_interrupt = (scause >> 63) != 0;

    if is_interrupt {
        match cause {
            1 => software_interrupt(tf),
            5 => timer_interrupt(tf),
            9 => external_interrupt(tf), // V36b: SEI (Supervisor External Interrupt) via AIA
            _ => {
                crate::println!("Unhandled interrupt: scause=0x{:x}", scause);
                crate::sched::schedule(); // try to recover
            }
        }
    } else {
        match cause {
            2 => handle_vector_trap(tf),  // V36a: Illegal instruction (lazy vec)
            8 => syscall(tf),     // Environment call from U-mode
            12 => page_fault(tf), // Instruction page fault
            13 => page_fault(tf), // Load page fault
            15 => page_fault(tf), // Store/AMO page fault
            _ => kill_current_process(scause, tf), // Kill process, don't hang kernel
        }
    }
}

fn software_interrupt(_tf: &mut TrapFrame) {
    unsafe {
        core::arch::asm!("csrc sip, {}", in(reg) 1usize << 1);
    }
    crate::sched::schedule();
}

pub(crate) static mut TICK_COUNT: usize = 0;
static mut INVARIANT_TICK: u64 = 0;

/// Handle supervisor external interrupt via AIA (SEI, cause 9).
///
/// Claims the interrupt from the IMSIC (or legacy PLIC) and handles it.
/// For now, external interrupts are expected to be handled by device
/// services via kernel proxy syscalls (MMIO mapping), so we simply
/// claim and complete to keep the interrupt system healthy.
fn external_interrupt(_tf: &mut TrapFrame) {
    // Claim the interrupt via the unified interrupt controller
    let irq = aia::claim_global();
    if irq != 0 {
        // In future: dispatch to registered device handler
        aia::complete_global(irq);
    }
    crate::sched::schedule();
}

fn timer_interrupt(_tf: &mut TrapFrame) {
    set_next_timer_tick();
    unsafe {
        core::arch::asm!("csrc sip, {}", in(reg) 1usize << 5);
    }

    // Account user time for the currently running process
    crate::syscall::proc::account_utime();

    unsafe {
        TICK_COUNT += 1;
        INVARIANT_TICK += 1;
        if INVARIANT_TICK % 100 == 0 {
            crate::invariant::run_checks();
        }
        // V25: NUMA load balancing every 1000 ticks
        if TICK_COUNT % 1000 == 0 {
            crate::numa::try_balance();
        }
        // V26: Distributed IPC heartbeat every 500 ticks
        if TICK_COUNT % 500 == 0 {
            crate::distributed::heartbeat_tick();
        }
    }

    // V24: TIMER hook
    let timer_pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    unsafe {
        crate::extension::run_hook(crate::extension::HOOK_TIMER, TICK_COUNT as u64, timer_pid as u64);
    }

    // V35: PREEMPT_LAZY — decrement remaining ticks and check preemption policy.
    // Fair-class (Lazy) tasks are preempted only when remaining_ticks reaches 0,
    // i.e. at the tick boundary.  RT-class (Immediate) tasks are preempted every
    // tick.  None-class (idle) tasks are never preempted from the timer tick.
    let needs_resched = crate::sched::current_thread()
        .map(|t| {
            // Decrement remaining ticks for the current thread
            unsafe {
                if (*t).remaining_ticks > 0 {
                    (*t).remaining_ticks -= 1;
                }
            }
            crate::sched::check_preempt_lazy(t)
        })
        .unwrap_or(true);

    if needs_resched {
        crate::sched::schedule();
    }
}

/// Handle illegal instruction trap for lazy vector extension activation.
/// On first vector instruction, enable V-extension for the task.
fn handle_vector_trap(tf: &mut TrapFrame) {
    let current = match crate::sched::current_thread() {
        Some(t) => t,
        None => { kill_current_process(2, tf); return; }
    };

    unsafe {
        // Check if this is a vector instruction by checking stval
        // (stval contains the faulting instruction encoding for illegal-instruction traps)
        let stval: usize;
        core::arch::asm!("csrr {}, stval", out(reg) stval);

        // Check if V extension is available by probing sstatus.VS
        if !crate::mem::vector::VectorState::is_available() {
            crate::println!("  VECTOR: V extension not available, killing process");
            kill_current_process(2, tf);
            return;
        }

        // Read sstatus and check current VS field
        let sstatus: usize;
        core::arch::asm!("csrr {}, sstatus", out(reg) sstatus);
        let vs = (sstatus >> 9) & 3;

        if vs == 0 {
            // VS is Off — this is a first-time vector instruction for this thread.
            // Mark the thread's vector state as dirty and enable VS.
            (*current).vector_state.mark_dirty();

            // Record lazy activation in stats
            crate::mem::vector::VECTOR_STATS.record_lazy_trap();
            let pid = (*current).owner;
            crate::println!("  VECTOR: lazy-activate pid={}", pid);

            // Increment vector task count
            crate::mem::vector::VECTOR_STATS.vector_tasks.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

            // Set VS=Initial (01) — allows vector instructions
            let new_sstatus = (sstatus & !(3usize << 9)) | (1usize << 9);
            core::arch::asm!("csrw sstatus, {}", in(reg) new_sstatus);

            // The instruction that faulted will be re-executed after sret
            // because we haven't advanced sepc.
            return;
        } else {
            // VS was already enabled but we still got an illegal instruction trap.
            // This is a genuine illegal instruction — kill the process.
            crate::println!("  VECTOR: illegal instruction (not V-related), killing process");
            kill_current_process(2, tf);
            return;
        }
    }
}

/// Save/restore vector context during context switch.
/// Called from scheduler BEFORE the context_switch assembly.
/// The outgoing thread's state is saved to its VectorState, and the incoming
/// thread's state is restored from its VectorState.
///
/// # Safety
/// Both pointers must be valid Thread pointers on the current HART.
pub unsafe fn switch_vector_context(from: *mut crate::proc::thread::Thread, to: *mut crate::proc::thread::Thread) {
    // --- Save outgoing thread's vector state ---
    if (*from).vector_state.dirty {
        // Save vector registers and CSRs
        (*from).vector_state.save();
        crate::mem::vector::VECTOR_STATS.record_save();

        // Disable VS for the outgoing thread
        let sstatus: usize;
        core::arch::asm!("csrr {}, sstatus", out(reg) sstatus);
        let clean = sstatus & !(3usize << 9); // VS=Off
        core::arch::asm!("csrw sstatus, {}", in(reg) clean);
    }

    // --- Restore incoming thread's vector state ---
    if (*to).vector_state.dirty {
        // Enable VS for the incoming thread
        let sstatus: usize;
        core::arch::asm!("csrr {}, sstatus", out(reg) sstatus);
        let enabled = (sstatus & !(3usize << 9)) | (1usize << 9); // VS=Initial
        core::arch::asm!("csrw sstatus, {}", in(reg) enabled);

        // Restore vector registers and CSRs
        (*to).vector_state.restore();
        crate::mem::vector::VECTOR_STATS.record_restore();
    }
}

fn syscall(tf: &mut TrapFrame) {
    crate::syscall::syscall_dispatch(tf);
}

fn page_fault(tf: &mut TrapFrame) {
    let stval: usize;
    unsafe {
        core::arch::asm!("csrr {}, stval", out(reg) stval);
    }
    let va = stval;

    // V21: Stack guard page detection — kernel stack overflow
    let sp: usize;
    unsafe { core::arch::asm!("mv {}, sp", out(reg) sp); }
    let stack_bottom = sp & !0xFFFF; // 64KB aligned kernel stack
    let guard_start = stack_bottom;
    let guard_end = stack_bottom + 0x1000; // 4KB guard page
    if va >= guard_start && va < guard_end {
        let pid = crate::sched::current_thread()
            .map(|t| unsafe { (*t).owner })
            .unwrap_or(0);
        crate::println!("STACK OVERFLOW: pid={} sp=0x{:x} fault=0x{:x}", pid, sp, va);
        kill_process(pid);
        crate::sched::schedule();
        return;
    }

    if va == 0 {
        // Null pointer dereference — kill process
        let pid = crate::sched::current_thread()
            .map(|t| unsafe { (*t).owner })
            .unwrap_or(0);
        crate::println!("  KILL: pid={} null pointer dereference", pid);
        kill_process(pid);
        crate::sched::schedule();
        return;
    }

    // Read current satp to find the current page table root
    let satp_val: usize;
    unsafe {
        core::arch::asm!("csrr {}, satp", out(reg) satp_val);
    }
    let root_phys = (satp_val & ((1usize << 44) - 1)) << 12;

    // Walk the current process's page table to check for COW
    if let Some((l0_phys, idx)) = unsafe { crate::proc::elf::walk_pt(root_phys, va, false) } {
        let l0 = unsafe {
            &*(crate::mem::sv39::pa_to_kva(l0_phys) as *const [crate::mem::sv39::PTE; 512])
        };
        let pte = l0[idx];
        if pte.is_cow() {
            // COW break: allocate new page, copy, update PTE
            let new_page = match crate::mem::buddy::alloc_page() {
                Some(p) => p,
                None => {
                    let pid = crate::sched::current_thread()
                        .map(|t| unsafe { (*t).owner })
                        .unwrap_or(0);
                    crate::println!("  KILL: pid={} OOM during COW", pid);
                    kill_process(pid);
                    crate::sched::schedule();
                    return;
                }
            };
            let old_kva = crate::mem::sv39::pa_to_kva(pte.phys_addr());
            let new_kva = crate::mem::sv39::pa_to_kva(new_page);
            unsafe {
                core::ptr::copy_nonoverlapping(old_kva as *const u8, new_kva as *mut u8, 4096);
            }
            let l0_mut = unsafe {
                &mut *(crate::mem::sv39::pa_to_kva(l0_phys) as *mut [crate::mem::sv39::PTE; 512])
            };
            let mut new_pte = crate::mem::sv39::PTE::empty();
            new_pte.set_ppn(new_page >> 12);
            new_pte.set_flags(true, true, pte.is_executable(), true); // R+W+U
            new_pte.set_accessed(true);
            new_pte.set_dirty(true);
            l0_mut[idx] = new_pte;
            unsafe {
                core::arch::asm!("sfence.vma {}", in(reg) va);
            }
            return;
        }
    }

    // Not a COW page — kill the process
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    crate::println!("  KILL: pid={} page fault at va=0x{:x}", pid, va);
    kill_process(pid);
    crate::sched::schedule();
}

/// Kill a process by PID — marks it as Dead and removes its thread.
/// Public for use by seccomp and security subsystem.
pub fn kill_process_impl(pid: u32) {
    kill_process(pid);
    crate::sched::schedule(); // reschedule immediately after killing
}

fn kill_process(pid: u32) {
    let mut procs = crate::proc::PROCESSES.lock();
    if let Some(proc) = procs.iter_mut().find(|p| p.pid == pid) {
        proc.state = crate::proc::process::ProcessState::Dead;
        if let Some(ref mut thread) = proc.thread {
            thread.state = crate::proc::thread::ThreadState::Dead;
        }
    }
}

/// Kill the current process due to an unhandled trap.
fn kill_current_process(scause: usize, tf: &TrapFrame) {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    let stval = tf.stval;
    crate::println!(
        "  KILL: pid={} unhandled trap scause=0x{:x} sepc=0x{:x} stval=0x{:x}",
        pid, scause, tf.sepc, stval
    );
    kill_process(pid);
}
