# TrainOS SMP Multi-Core Design

**Date**: 2026-05-07
**Status**: Draft
**Goal**: Add SMP (Symmetric Multi-Processing) support to TrainOS microkernel, enabling concurrent execution of user-space services across multiple RISC-V HARTs.

## 1. Boot Architecture

### 1.1 Primary vs Secondary HARTs

```
Primary HART (HART 0)              Secondary HARTs (HART 1..N-1)
 _start:                            _start:
   csrw sie, zero                     csrw sie, zero
   la sp, _boot_stack_top             la sp, per_hart_stack[N]
   tail rust_main                     tail rust_secondary
   │                                    │
 rust_main(0):                      rust_secondary():
   clear BSS (primary ONLY)           while !BOOT_READY.load() { wfi }
   mem::init()                        trap::init()         (set stvec)
   trap::clint_init()                 per_cpu::init()
   trap::enable_timer_interrupt()     enable MMU (shared PT)
   trap::init() (set stvec)           enable timer interrupt
   per_cpu::init()                    schedule()           (enters idle)
   cap::init()
   ipc::init()
   enable MMU
   spawn all services
   BOOT_READY.store(true, Release)
   start_scheduler()
```

### 1.2 Rules
- **BSS clear**: Only the primary HART clears BSS. Secondary HARTs skip it.
- **Kernel page table**: All HARTs share the same root page table. `satp` is set identically.
- **BOOT_READY**: `AtomicBool` with `Release`/`Acquire` ordering for synchronization.
- **Boot stacks**: N × 64KB stacks allocated at compile time via linker script (N=4 initially).

## 2. Per-CPU Data

### 2.1 PerCpu Structure

```rust
struct PerCpu {
    hart_id: usize,
    current: Option<*mut Thread>,
    idle: *mut Thread,
    kernel_sp: usize,       // for sscratch restore
}

static PER_CPU: [PerCpu; 4] = [...];
```

### 2.2 HART ID Discovery

The RISC-V `tp` register holds the HART ID (set by RustSBI). `hart_id()` reads it:

```rust
fn hart_id() -> usize {
    let id: usize;
    unsafe { core::arch::asm!("mv {}, tp", out(reg) id); }
    id
}
```

## 3. Scheduler Changes

### 3.1 V2.1: Global Queues + Kernel Spinlock

All CPUs share one set of ready queues (64 priority levels) protected by a **real spinlock** (not spin::Mutex — a raw atomic-based lock):

```rust
static SCHED_LOCK: AtomicBool = AtomicBool::new(false);
static SCHEDULER: Scheduler = Scheduler::new(); // no Mutex wrapper

fn schedule() {
    spin_lock(&SCHED_LOCK);
    // re-enqueue current if Running
    // dequeue highest
    // sched.current = next (per-CPU, for the calling hart)
    spin_unlock(&SCHED_LOCK);
    context_switch(old, new);
}
```

### 3.2 Spinlock

```rust
fn spin_lock(lock: &AtomicBool) {
    while lock.compare_exchange(false, true, AcqRel, Relaxed).is_err() {
        // Busy-wait with hint for hypervisor
        unsafe { core::arch::asm!("pause"); }
    }
}
```

### 3.3 Soft Affinity

Scheduler stores `last_cpu[tid]` hint. When multiple threads have the same priority, prefer the thread that last ran on the current CPU (cache warmth).

### 3.4 CLINT Timer Per-HART

```
CLINT_MTIMECMP(hart) = CLINT_BASE + 0x4000 + hart * 8
CLINT_MTIME          = CLINT_BASE + 0xBFF8  (shared)
```

Each HART arms its own timer independently.

## 4. IPI (Inter-Processor Interrupt)

### 4.1 Mechanism

RISC-V S-mode software interrupt via CLINT MSIP:
```
CLINT_MSIP(hart) = CLINT_BASE + hart * 4
```

Writing 1 to this address triggers an S-mode software interrupt on the target HART.

### 4.2 Trap Handler Extension

```
scause=1 (interrupt) → software_interrupt:
  csrs sip, (1<<1)     // ack SSIP
  schedule()            // may pick newly-woken thread
```

### 4.3 V2.1 IPI Policy

**Cross-core wakeup**: When `send()` wakes a receiver on a different CPU, write to CLINT_MSIP to nudge that CPU. This is optional in V2.1 — the 10ms timer tick will naturally cause a reschedule.

**TLB shootdown**: V2.1 does NOT handle cross-core TLB invalidation. Since each process is single-threaded (one CPU at a time), COW pages are never accessed by multiple CPUs simultaneously. Full TLB shootdown protocol is deferred to V2.2.

## 5. IPC Under SMP

### 5.1 Current State

IPC endpoints are protected by `ENDPOINTS` global Mutex, which serializes all `send`/`recv` operations. This is SMP-safe by design.

### 5.2 Cross-Core Wakeup

When `send()` wakes a receiver that was blocked (via `waiting_receiver.take()`), the receiver thread is enqueued in the shared ready queue with the scheduler lock held. On `spin_unlock(&SCHED_LOCK)`, the receiver is visible to all CPUs.

### 5.3 V2.1 Simplification

No active IPI on `send()`. Receiver thread will be picked up on the next timer tick (≤10ms). This avoids interrupt nesting complexity while proving multi-core correctness. Active IPI added in V2.2.

## 6. Memory Ordering

### 6.1 Rules
- All scheduler state mutations happen under `SCHED_LOCK` → `AcqRel` ordering.
- `BOOT_READY` uses `Release` (primary) / `Acquire` (secondary).
- `PerCpu.current` is R/W only by the owning HART; read-only by IPI handler on the same HART.

### 6.2 sfence.vma
- TLB flush (`sfence.vma`) is per-HART (local). No cross-HART TLB flush in V2.1.
- Kernel page table is read-only after boot (no runtime kernel PT modifications).

## 7. V2.1 Scope

### In Scope
- [ ] Secondary HART boot sequence (park + wake)
- [ ] Per-CPU data structures
- [ ] Global scheduler lock (spinlock replacing spin::Mutex)
- [ ] Per-HART CLINT timer
- [ ] Software interrupt (IPI) handler stub
- [ ] Multi-core IPC (same lock, cross-core enqueue)
- [ ] Boot test with 2 HARTs

### Out of Scope (deferred)
- [ ] Active cross-core IPI on send() wakeup
- [ ] Cross-core TLB shootdown
- [ ] NUMA awareness
- [ ] Per-CPU ready queues
- [ ] Work stealing

## 8. Test Strategy

1. **Boot test**: Launch machina with `-smp 2`. Verify both HARTs reach idle loop.
2. **Timer test**: Verify each HART receives independent timer interrupts.
3. **IPC test**: Run existing multi-process demo (init + ping) on 2 HARTs.
4. **Stress test**: 4 processes on 2 HARTs doing IPC ping-pong.

---

## Appendix: Full V2.1-V3.0 Roadmap

| Phase | Content | Priority |
|-------|---------|----------|
| A1 (V2.1) | SMP multi-core | Foundation |
| C1 (V2.2) | Capability enforcement | Security |
| B1 (V2.3) | POSIX compatibility service | Ecosystem |
| A2 (V2.4) | VirtIO block driver | Hardware |
| A3 (V2.5) | VirtIO network + stack | Hardware |
| C2 (V2.6) | Namespace isolation | Security |
| B2 (V2.7) | Standard C program support | Ecosystem |
| V3.0 | SMP 2.0: per-CPU queues + work stealing + full IPI | Performance |
