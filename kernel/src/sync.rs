use core::sync::atomic::{AtomicBool, Ordering};

pub struct SpinLock {
    flag: AtomicBool,
}

impl SpinLock {
    pub const fn new() -> Self {
        SpinLock { flag: AtomicBool::new(false) }
    }

    pub fn lock(&self) {
        while self.flag.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            // Hint for hypervisor to yield
            #[cfg(target_arch = "riscv64")]
            unsafe { core::arch::asm!("nop"); }
            #[cfg(not(target_arch = "riscv64"))]
            unsafe { core::arch::asm!("pause"); }
        }
    }

    pub fn unlock(&self) {
        self.flag.store(false, Ordering::Release);
    }

    #[allow(dead_code)]
    pub fn try_lock(&self) -> bool {
        self.flag.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok()
    }
}

unsafe impl Sync for SpinLock {}
