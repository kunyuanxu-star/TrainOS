//! Threading Support Module
//!
//! Provides pthread-like threading interface

use spin::Mutex;

/// Maximum number of threads
pub const MAX_THREADS: usize = 64;

/// Thread ID
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThreadId(usize);

impl ThreadId {
    pub fn new(id: usize) -> Self {
        Self(id)
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }
}

/// Thread status
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThreadStatus {
    Ready,
    Running,
    Blocked,
    Exited,
}

/// Thread local storage key
pub struct ThreadLocalKey<T> {
    _marker: core::marker::PhantomData<T>,
}

impl<T> ThreadLocalKey<T> {
    pub fn new() -> Self {
        Self {
            _marker: core::marker::PhantomData,
        }
    }

    pub fn with<F, R>(&self, _f: F) -> R
    where
        F: FnOnce(Option<&T>) -> R,
    {
        // Thread-local storage would need compiler support or per-CPU data
        // For now, return None
        _f(None)
    }
}

impl<T> Default for ThreadLocalKey<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread control block
pub struct Thread {
    /// Thread ID
    pub id: ThreadId,
    /// Thread name (for debugging)
    pub name: [u8; 32],
    /// Thread status
    pub status: ThreadStatus,
    /// Thread stack pointer
    pub sp: usize,
    /// Thread program counter
    pub pc: usize,
    /// Thread local storage pointer
    pub tls: usize,
    /// Parent thread ID
    pub parent_id: Option<ThreadId>,
    /// Exit code
    pub exit_code: Option<i32>,
}

impl Thread {
    pub fn new(id: usize) -> Self {
        Self {
            id: ThreadId::new(id),
            name: [0; 32],
            status: ThreadStatus::Ready,
            sp: 0,
            pc: 0,
            tls: 0,
            parent_id: None,
            exit_code: None,
        }
    }

    pub fn set_name(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(31);
        self.name[..len].copy_from_slice(&bytes[..len]);
        self.name[len] = 0;
    }
}

/// Thread table
pub struct ThreadTable {
    threads: [Option<Thread>; MAX_THREADS],
    next_id: usize,
}

impl ThreadTable {
    pub fn new() -> Self {
        let mut threads: [Option<Thread>; MAX_THREADS] = unsafe { core::mem::zeroed() };
        for i in 0..MAX_THREADS {
            threads[i] = None;
        }
        Self {
            threads,
            next_id: 1,
        }
    }

    /// Create a new thread
    pub fn create(&mut self, pc: usize, sp: usize, parent_id: Option<ThreadId>) -> Option<ThreadId> {
        let id = self.next_id;
        self.next_id += 1;

        for i in 0..MAX_THREADS {
            if self.threads[i].is_none() {
                let mut thread = Thread::new(id);
                thread.pc = pc;
                thread.sp = sp;
                thread.parent_id = parent_id;
                thread.status = ThreadStatus::Ready;
                self.threads[i] = Some(thread);
                return Some(ThreadId::new(id));
            }
        }
        None
    }

    /// Get thread by ID
    pub fn get(&self, id: ThreadId) -> Option<&Thread> {
        let idx = id.as_usize();
        if idx < MAX_THREADS {
            self.threads[idx].as_ref()
        } else {
            None
        }
    }

    /// Get mutable thread by ID
    pub fn get_mut(&mut self, id: ThreadId) -> Option<&mut Thread> {
        let idx = id.as_usize();
        if idx < MAX_THREADS {
            self.threads[idx].as_mut()
        } else {
            None
        }
    }

    /// Set thread status
    pub fn set_status(&mut self, id: ThreadId, status: ThreadStatus) {
        if let Some(thread) = self.get_mut(id) {
            thread.status = status;
        }
    }

    /// Exit thread
    pub fn exit(&mut self, id: ThreadId, code: i32) {
        if let Some(thread) = self.get_mut(id) {
            thread.status = ThreadStatus::Exited;
            thread.exit_code = Some(code);
        }
    }

    /// Join thread (wait for exit)
    pub fn join(&mut self, id: ThreadId) -> Option<i32> {
        if let Some(thread) = self.get(id) {
            if thread.status == ThreadStatus::Exited {
                return thread.exit_code;
            }
        }
        None
    }
}

impl Default for ThreadTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Global thread table
static THREAD_TABLE: Mutex<Option<ThreadTable>> = Mutex::new(None);

/// Get or create thread table
fn get_thread_table() -> spin::MutexGuard<'static, Option<ThreadTable>> {
    let mut guard = THREAD_TABLE.lock();
    if guard.is_none() {
        *guard = Some(ThreadTable::new());
    }
    guard
}

/// Create a new thread
pub fn thread_create(pc: usize, sp: usize) -> Option<ThreadId> {
    get_thread_table().as_mut()?.create(pc, sp, None)
}

/// Get current thread ID
pub fn thread_self() -> ThreadId {
    // In a real implementation, this would read from per-CPU data
    ThreadId::new(0)
}

/// Exit current thread
pub fn thread_exit(code: i32) {
    let tid = thread_self();
    if let Some(ref mut table) = *get_thread_table() {
        table.exit(tid, code);
    }
}

/// Join (wait for) a thread
pub fn thread_join(tid: ThreadId) -> Option<i32> {
    if let Some(ref mut table) = *get_thread_table() {
        table.join(tid)
    } else {
        None
    }
}

/// Yield CPU to another thread
pub fn thread_yield() {
    // In a real implementation, this would call the scheduler
    crate::syscall::sys_sched_yield();
}

// ============================================
// pthread-like API
// ============================================

/// pthread attribute
#[derive(Debug, Clone, Copy)]
pub struct PthreadAttr {
    /// Stack size
    pub stack_size: usize,
    /// Guard size
    pub guard_size: usize,
    /// Scheduling policy
    pub sched_policy: i32,
    /// Priority
    pub sched_priority: i32,
    /// Detached state
    pub detached: bool,
}

impl Default for PthreadAttr {
    fn default() -> Self {
        Self {
            stack_size: 2 * 1024 * 1024,  // 2MB default
            guard_size: 4096,             // 4KB guard
            sched_policy: 0,              // SCHED_OTHER
            sched_priority: 0,
            detached: false,
        }
    }
}

/// Mutex type
#[repr(C)]
pub struct PthreadMutex {
    /// Mutex state (0 = unlocked, 1 = locked)
    state: Mutex<u32>,
}

impl PthreadMutex {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(0),
        }
    }

    /// Lock the mutex
    pub fn lock(&self) {
        let mut state = self.state.lock();
        *state = 1;
    }

    /// Try to lock the mutex (non-blocking)
    pub fn try_lock(&self) -> bool {
        let mut state = self.state.lock();
        if *state == 0 {
            *state = 1;
            true
        } else {
            false
        }
    }

    /// Unlock the mutex
    pub fn unlock(&self) {
        let mut state = self.state.lock();
        *state = 0;
    }
}

impl Default for PthreadMutex {
    fn default() -> Self {
        Self::new()
    }
}

/// Condition variable
pub struct PthreadCond {
    /// Wait queue (simplified)
    _queue: Mutex<u32>,
}

impl PthreadCond {
    pub fn new() -> Self {
        Self {
            _queue: Mutex::new(0),
        }
    }

    /// Wait on condition
    pub fn wait(&self, _mutex: &PthreadMutex) {
        // In a real implementation, this would block and add to wait queue
    }

    /// Signal one waiting thread
    pub fn signal(&self) {
        // Wake up one waiting thread
    }

    /// Broadcast to all waiting threads
    pub fn broadcast(&self) {
        // Wake up all waiting threads
    }
}

impl Default for PthreadCond {
    fn default() -> Self {
        Self::new()
    }
}

/// Barrier
pub struct PthreadBarrier {
    /// Number of threads to wait
    count: Mutex<usize>,
    /// Current count
    current: Mutex<usize>,
}

impl PthreadBarrier {
    pub fn new(n: usize) -> Self {
        Self {
            count: Mutex::new(n),
            current: Mutex::new(0),
        }
    }

    /// Wait at barrier
    pub fn wait(&self) -> usize {
        let mut current = self.current.lock();
        let n = *self.count.lock();
        *current += 1;
        if *current >= n {
            *current = 0;
            n  // Return number of threads
        } else {
            0
        }
    }
}

/// RwLock (read-write lock)
pub struct PthreadRwLock {
    /// Number of readers
    readers: Mutex<u32>,
    /// Write locked flag
    write_locked: Mutex<bool>,
}

impl PthreadRwLock {
    pub fn new() -> Self {
        Self {
            readers: Mutex::new(0),
            write_locked: Mutex::new(false),
        }
    }

    /// Read lock
    pub fn read_lock(&self) {
        let mut readers = self.readers.lock();
        *readers += 1;
    }

    /// Read unlock
    pub fn read_unlock(&self) {
        let mut readers = self.readers.lock();
        *readers = readers.saturating_sub(1);
    }

    /// Write lock
    pub fn write_lock(&self) {
        let mut write_locked = self.write_locked.lock();
        *write_locked = true;
    }

    /// Write unlock
    pub fn write_unlock(&self) {
        let mut write_locked = self.write_locked.lock();
        *write_locked = false;
    }
}

impl Default for PthreadRwLock {
    fn default() -> Self {
        Self::new()
    }
}

/// Once primitive for one-time initialization
pub struct Once {
    _state: Mutex<u32>,
}

impl Once {
    pub fn new() -> Self {
        Self {
            _state: Mutex::new(0),
        }
    }
}

impl Default for Once {
    fn default() -> Self {
        Self::new()
    }
}
