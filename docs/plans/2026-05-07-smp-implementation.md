# SMP Multi-Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add SMP support to TrainOS microkernel, enabling 2-4 HARTs to run user-space services concurrently.

**Architecture:** Global scheduler with spinlock, per-CPU data structures, shared kernel page table. Primary HART boots and initialises the system; secondary HARTs park until BOOT_READY signal, then enter the scheduler. IPC endpoints remain protected by existing Mutex; cross-core wakeup via the shared ready queue with passive (timer-based) reschedule in V2.1.

**Tech Stack:** Rust nightly, riscv64gc-unknown-none-elf, machina with `-smp N`.

---

## Task 1: Per-HART boot stacks and secondary entry point

**Files:**
- Modify: `kernel/src/main.rs` — boot stack assembly and secondary entry

- [ ] **Step 1: Allocate per-HART boot stacks in assembly**

Read `kernel/src/main.rs`. Replace the single boot stack assembly with multiple per-HART stacks:

```asm
core::arch::global_asm!(
    ".section .text.entry, \"ax\", @progbits",
    ".globl _start",
    "_start:",
    "    csrw sie, zero",
    // Read HART ID from tp register
    "    mv t0, tp",
    // Load per-HART stack: _boot_stack_top + hart_id * 65536
    "    slli t1, t0, 16",            // t1 = hart_id * 65536
    "    la t2, _boot_stacks",
    "    add t2, t2, t1",
    "    mv sp, t2",
    // If HART 0, jump to rust_main. Otherwise, rust_secondary.
    "    bnez t0, 1f",
    "    tail rust_main",
    "1:  tail rust_secondary",
    ".section .bss",
    ".align 12",                         // 4096-byte aligned
    "_boot_stacks:",
    "    .space 65536 * 4, 0",           // 4 HARTs × 64KB
);
```

Delete the old `_boot_stack_bottom`/`_boot_stack_top` labels.

- [ ] **Step 2: Add rust_secondary function**

Add to `kernel/src/main.rs`:

```rust
use core::sync::atomic::{AtomicBool, Ordering};

static BOOT_READY: AtomicBool = AtomicBool::new(false);

#[no_mangle]
extern "C" fn rust_secondary() -> ! {
    // Park until primary signals ready
    while !BOOT_READY.load(Ordering::Acquire) {
        unsafe { core::arch::asm!("wfi"); }
    }
    
    // Same setup as primary, minus BSS clear and memory init
    trap::enable_timer_interrupt();
    trap::init();
    mem::sv39::enable_mmu();
    
    // Enter scheduler
    per_cpu::init_secondary();
    sched::schedule();
    
    // Should never return
    idle_loop();
}
```

Add `mod per_cpu;` to main.rs.

- [ ] **Step 3: Update rust_main to signal BOOT_READY**

After spawning all services and before `start_scheduler()`, add:

```rust
BOOT_READY.store(true, Ordering::Release);
console::puts("  Secondary HARTs released\r\n");
```

- [ ] **Step 4: Build and verify with smp=1**

```bash
cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && cargo build --release -p kernel 2>&1 | tail -5
```
Expected: builds successfully. Test with smp=1 first:
```bash
timeout 8 /home/xukunyuan/code/AI4OSE/testOS/machina/target/release/machina \
  -M riscv64-ref -smp 1 \
  -bios /home/xukunyuan/code/AI4OSE/testOS/machina/pc-bios/rustsbi-riscv64-machina-fw_dynamic.bin \
  -kernel /home/xukunyuan/code/AI4OSE/testOS/TrainOS/target/riscv64gc-unknown-none-elf/release/kernel \
  -nographic 2>&1 || true
```
Expected: No regression — "TrainOS ready" appears as before.

---

## Task 2: HART ID and per-CPU module

**Files:**
- Create: `kernel/src/per_cpu.rs`

- [ ] **Step 1: Write per_cpu module**

Write `kernel/src/per_cpu.rs`:

```rust
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

/// Get mutable reference to this HART's PerCpu.
pub fn this_cpu() -> &'static mut PerCpu {
    let hart = hart_id();
    unsafe { &mut PER_CPU[hart] }
}

/// Get immutable reference to this HART's PerCpu.
pub fn this_cpu_ref() -> &'static PerCpu {
    let hart = hart_id();
    unsafe { &PER_CPU[hart] }
}

/// Get immutable reference to any HART's PerCpu.
pub fn cpu(hart: usize) -> &'static PerCpu {
    unsafe { &PER_CPU[hart] }
}

pub fn hart_id() -> usize {
    let id: usize;
    unsafe { core::arch::asm!("mv {}, tp", out(reg) id); }
    id
}

pub fn hart_count() -> usize {
    // For now, return a compile-time constant.
    // Future: read from device tree.
    MAX_HARTS
}

/// Primary HART init: create idle threads for all HARTs.
pub fn init() {
    for hart in 0..MAX_HARTS {
        let idle = Box::new(Thread::new_idle());
        let idle_ptr = Box::into_raw(idle);
        unsafe {
            PER_CPU[hart].idle = Some(idle_ptr);
        }
    }
}

/// Secondary HART init: set this HART's current to its idle thread.
pub fn init_secondary() {
    let hart = hart_id();
    let idle = unsafe { PER_CPU[hart].idle.unwrap() };
    unsafe { (*idle).state = ThreadState::Running; }
    this_cpu().current = Some(idle);
}
```

Add `pub mod per_cpu;` to `kernel/src/main.rs`.

- [ ] **Step 2: Build and verify**

```bash
cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && cargo build --release -p kernel 2>&1 | tail -5
```
Expected: builds.

---

## Task 3: Spinlock primitive

**Files:**
- Create: `kernel/src/sync.rs`

- [ ] **Step 1: Write spinlock module**

Write `kernel/src/sync.rs`:

```rust
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
            // Spin with a hint for the hypervisor
            unsafe { core::arch::asm!("pause"); }
        }
    }

    pub fn unlock(&self) {
        self.flag.store(false, Ordering::Release);
    }

    /// Try to lock. Returns true if lock was acquired.
    pub fn try_lock(&self) -> bool {
        self.flag.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok()
    }
}

unsafe impl Sync for SpinLock {}
```

Add `mod sync;` to `kernel/src/main.rs`.

- [ ] **Step 2: Build**

```bash
cargo build --release -p kernel 2>&1 | tail -3
```
Expected: builds.

---

## Task 4: Convert scheduler to spinlock

**Files:**
- Modify: `kernel/src/sched/mod.rs`

- [ ] **Step 1: Replace Mutex with SpinLock**

Read `kernel/src/sched/mod.rs`. Replace:
```rust
use spin::Mutex;
static SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());
```

With:
```rust
use crate::sync::SpinLock;
static SCHED_LOCK: SpinLock = SpinLock::new();
static mut SCHEDULER: Scheduler = Scheduler::new();
```

Update all accessors:

```rust
pub fn enqueue_thread(thread: *mut Thread) {
    SCHED_LOCK.lock();
    unsafe { SCHEDULER.enqueue(thread); }
    SCHED_LOCK.unlock();
}

pub fn schedule() {
    SCHED_LOCK.lock();
    let current_ptr = unsafe { SCHEDULER.current };
    
    if let Some(cur) = current_ptr {
        unsafe {
            if (*cur).state == ThreadState::Running {
                SCHEDULER.enqueue(cur);
            }
        }
    }
    
    let next = unsafe { SCHEDULER.dequeue_highest() };
    unsafe { SCHEDULER.current = next; }
    SCHED_LOCK.unlock();
    
    // Context switch outside lock
    unsafe {
        match (current_ptr, next) {
            (Some(old), Some(new)) => {
                // Save this HART's kernel_sp for sscratch
                // context_switch saves callee-saved regs including sp
                crate::proc::switch::context_switch(&mut (*old).task_ctx, &(*new).task_ctx);
            }
            (None, Some(new)) => {
                let ra = (*new).task_ctx.ra;
                let sp = (*new).task_ctx.sp;
                core::arch::asm!("mv sp, {sp}", "jr {ra}", sp = in(reg) sp, ra = in(reg) ra, options(noreturn));
            }
            _ => crate::idle_loop(),
        }
    }
}

pub fn current_thread() -> Option<*mut Thread> {
    SCHED_LOCK.lock();
    let cur = unsafe { SCHEDULER.current };
    SCHED_LOCK.unlock();
    cur
}
```

- [ ] **Step 2: Build and test with smp=1**

```bash
cargo build --release -p kernel 2>&1 | tail -5
```
Test on machina with smp=1. Expected: no regression.

---

## Task 5: Per-HART CLINT timer

**Files:**
- Modify: `kernel/src/trap/mod.rs`

- [ ] **Step 1: Make CLINT functions per-HART aware**

Read `kernel/src/trap/mod.rs`. Add `hart_id()` call and update CLINT offsets:

```rust
fn clint_mtimecmp_offset() -> usize {
    let hart = crate::per_cpu::hart_id();
    CLINT_BASE + 0x4000 + hart * 8
}

unsafe fn mtime() -> u64 {
    (CLINT_MTIME as *const u64).read_volatile()
}

unsafe fn set_mtimecmp(val: u64) {
    (clint_mtimecmp_offset() as *mut u64).write_volatile(val);
}
```

Replace the existing `set_mtimecmp` implementation.

- [ ] **Step 2: Build and test with smp=1**

```bash
cargo build --release -p kernel 2>&1 | tail -5
```
Test on machina. Expected: timer still works on HART 0.

---

## Task 6: Software interrupt handler (IPI stub)

**Files:**
- Modify: `kernel/src/trap/mod.rs`

- [ ] **Step 1: Add software interrupt handler**

Read `kernel/src/trap/mod.rs`. In the interrupt match of `handle_trap`, add:

```rust
1 => software_interrupt(tf), // S-mode Software Interrupt
```

Add handler:

```rust
fn software_interrupt(_tf: &mut TrapFrame) {
    // Acknowledge SSIP
    unsafe { core::arch::asm!("csrc sip, {}", in(reg) 1usize << 1); }
    // Reschedule (may pick up newly-woken thread)
    crate::sched::schedule();
}
```

- [ ] **Step 2: Build and test with smp=2**

```bash
cargo build --release -p kernel 2>&1 | tail -5
```

Test on machina with smp=2:
```bash
timeout 8 /home/xukunyuan/code/AI4OSE/testOS/machina/target/release/machina \
  -M riscv64-ref -smp 2 \
  -bios /home/xukunyuan/code/AI4OSE/testOS/machina/pc-bios/rustsbi-riscv64-machina-fw_dynamic.bin \
  -kernel /home/xukunyuan/code/AI4OSE/testOS/TrainOS/target/riscv64gc-unknown-none-elf/release/kernel \
  -nographic 2>&1 || true
```

Expected: Both HARTs reach idle loop. Services run. No crash.

---

## Task 7: Integration test — multi-process on 2 HARTs

**Files:**
- Modify: `kernel/src/proc/mod.rs` — add per-CPU idle in scheduler initialization
- Modify: `kernel/src/main.rs` — update start_scheduler call

- [ ] **Step 1: Update start_scheduler for SMP**

In `main.rs`, change the scheduler start to use per-CPU idle:

```rust
// Per-CPU idle threads are already created by per_cpu::init().
// Start scheduler on HART 0 using its idle thread.
let idle0 = per_cpu::cpu(0).idle.unwrap();
sched::start_scheduler(idle0);
```

- [ ] **Step 2: Remove old idle Box::new from main.rs**

Delete the old:
```rust
let idle = Box::new(crate::proc::thread::Thread::new_idle());
let idle_ptr: *mut crate::proc::thread::Thread = Box::into_raw(idle);
```

- [ ] **Step 3: Build and full integration test**

```bash
cargo build --release -p kernel 2>&1 | tail -5
```

Test:
```bash
timeout 8 /home/xukunyuan/code/AI4OSE/testOS/machina/target/release/machina \
  -M riscv64-ref -smp 2 \
  -bios /home/xukunyuan/code/AI4OSE/testOS/machina/pc-bios/rustsbi-riscv64-machina-fw_dynamic.bin \
  -kernel /home/xukunyuan/code/AI4OSE/testOS/TrainOS/target/riscv64gc-unknown-none-elf/release/kernel \
  -nographic 2>&1 || true
```

Expected: All services (init, ping, fs, test_fs, sh, test_fork) run correctly on 2 HARTs. "TrainOS IPC OK", "TEST_FS: PASS", "FORK: *", shell prompt all appear.

---

## Task 8: Commit and document

- [ ] **Step 1: Commit SMP changes**

```bash
cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && git add -A && git commit -m "feat: add SMP multi-core support (V2.1)

- Add per-HART boot stacks (4 × 64KB) with HART ID dispatch
- Add BOOT_READY synchronization for secondary HART parking
- Add per-CPU data structures (PerCpu with current/idle)
- Add real spinlock and convert scheduler from spin::Mutex
- Add per-HART CLINT timer offsets
- Add S-mode software interrupt handler (IPI stub)
- V2.1: passive cross-core wakeup via shared ready queue
- Tested with machina -smp 2

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>" && git push origin main 2>&1
```
