use crate::proc::thread::Thread;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

pub struct SpinLock {
    flag: AtomicBool,
}

impl SpinLock {
    pub const fn new() -> Self {
        SpinLock {
            flag: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) {
        while self
            .flag
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Hint for hypervisor to yield
            #[cfg(target_arch = "riscv64")]
            unsafe {
                core::arch::asm!("nop");
            }
            #[cfg(not(target_arch = "riscv64"))]
            unsafe {
                core::arch::asm!("pause");
            }
        }
    }

    pub fn unlock(&self) {
        self.flag.store(false, Ordering::Release);
    }

    #[allow(dead_code)]
    pub fn try_lock(&self) -> bool {
        self.flag
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
    }
}

unsafe impl Sync for SpinLock {}

// ═══════════════════════════════════════════════════════════════════════════
// V35 — ProxyLock (Proxy-Execution-Aware Lock)
// ═══════════════════════════════════════════════════════════════════════════
//
// ProxyLock extends the basic spinlock concept with proxy execution support.
// When a thread contends on a held lock, instead of spinning (wasting CPU
// cycles), it uses proxy execution to donate its remaining time slice to the
// lock holder, allowing the holder to finish its critical section sooner.
//
// This directly addresses priority inversion: the holder inherits the
// contending thread's effective priority while it holds the lock.
//
// Usage:
//   static LOCK: ProxyLock = ProxyLock::new();
//   LOCK.lock();
//   // critical section
//   LOCK.unlock();
//
// NOTE: ProxyLock is heavier than SpinLock — use SpinLock for short,
//       non-contended kernel-internal locks.  Use ProxyLock for locks that
//       may be held across user-space operations or I/O boundaries.

pub struct ProxyLock {
    /// Atomic flag: false = unlocked, true = locked.
    flag: AtomicBool,
    /// Thread pointer of the current lock holder (null = unlocked).
    holder: AtomicPtr<Thread>,
}

impl ProxyLock {
    pub const fn new() -> Self {
        ProxyLock {
            flag: AtomicBool::new(false),
            holder: AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    /// Acquire the lock.
    ///
    /// First attempts a quick CAS (spin a few times).  If the lock is still
    /// contended, sets up proxy execution: the current thread (blocker)
    /// donates its scheduling context to the lock holder so the holder can
    /// finish quickly.  The blocker then yields the CPU.
    ///
    /// On return, the caller holds the lock.
    pub fn lock(&self) {
        // Fast path: try to acquire immediately.
        if self
            .flag
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            // Record ourselves as holder.
            if let Some(current) = crate::sched::current_thread() {
                self.holder.store(current, Ordering::Relaxed);
            }
            return;
        }

        // Lock is contended.  Spin briefly (up to ~64 iterations) before
        // falling back to proxy execution.
        for _ in 0..64 {
            #[cfg(target_arch = "riscv64")]
            unsafe { core::arch::asm!("nop"); }
            #[cfg(not(target_arch = "riscv64"))]
            unsafe { core::arch::asm!("pause"); }

            if self
                .flag
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                if let Some(current) = crate::sched::current_thread() {
                    self.holder.store(current, Ordering::Relaxed);
                }
                return;
            }
        }

        // ── Proxy execution path ──────────────────────────────────────────
        // The lock is held by another thread.  Set up proxy execution:
        // donate our priority and time slice to the holder, then yield the
        // CPU so the holder (now boosted) can run and finish quickly.
        let blocker = crate::sched::current_thread();
        let holder_ptr = self.holder.load(Ordering::Acquire);

        if let Some(blk) = blocker {
            if !holder_ptr.is_null() {
                crate::sched::proxy_start(blk, holder_ptr);

                // Yield the CPU.  Because the blocker's state is still
                // `Running`, `schedule()` will re-enqueue it.  The holder
                // (now at boosted priority) runs first, finishes the critical
                // section, and calls `unlock()` which triggers `proxy_end()`.
                // When the blocker is picked again, the lock should be free.
                crate::sched::schedule();
            }
        }

        // Retry the acquire.
        while self
            .flag
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            #[cfg(target_arch = "riscv64")]
            unsafe { core::arch::asm!("nop"); }
            #[cfg(not(target_arch = "riscv64"))]
            unsafe { core::arch::asm!("pause"); }
        }
        if let Some(current) = crate::sched::current_thread() {
            self.holder.store(current, Ordering::Relaxed);
        }
    }

    /// Release the lock.
    ///
    /// Cleans up proxy execution state (restores the holder's original
    /// priority if proxy was active) and wakes any blocked contender.
    pub fn unlock(&self) {
        // End proxy execution if we were acting as a proxy holder.
        if let Some(current) = crate::sched::current_thread() {
            crate::sched::proxy_end(current);
        }

        self.holder.store(core::ptr::null_mut(), Ordering::Release);
        self.flag.store(false, Ordering::Release);
    }

    /// Try to acquire the lock without blocking.
    /// Returns `true` on success.
    #[allow(dead_code)]
    pub fn try_lock(&self) -> bool {
        if self
            .flag
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            if let Some(current) = crate::sched::current_thread() {
                self.holder.store(current, Ordering::Relaxed);
            }
            true
        } else {
            false
        }
    }
}

unsafe impl Sync for ProxyLock {}
