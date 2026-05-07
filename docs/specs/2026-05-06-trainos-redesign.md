# TrainOS Redesign Specification

**Date**: 2026-05-06
**Status**: Draft
**Goal**: Completely redesign TrainOS as a microkernel OS for RISC-V 64-bit (rv64gc), targeting RustSBI + machina as the runtime environment. Long-term ambition: surpass Linux in architecture, security, and performance.

## Iron Rules

1. **Runtime**: RustSBI (M-mode firmware) + machina (RISC-V full-system emulator with JIT). These are non-negotiable.
2. **Language**: Rust (kernel in `no_std`, user-space services may use `std`-subset).
3. **Architecture**: RISC-V 64-bit (rv64gc), Sv39 virtual memory.
4. **License**: MIT.

---

## 1. Architecture Overview

### 1.1 Kernel Scope (absolute minimum)

The kernel does **exactly four things**:

| Module | Responsibility | Must NOT do |
|--------|---------------|-------------|
| Capability System | Access control for all resources | Policy decisions |
| IPC Router | Message delivery between processes | Buffer messages, retry |
| Scheduler | CPU time allocation, priority inheritance | Know about application workload |
| Memory Manager | Physical page allocation, Sv39 page table, COW | Track user-level memory semantics |

Everything else — device drivers, filesystems, network protocols, POSIX compatibility — runs in **user space** and communicates via IPC.

### 1.2 System Diagram

```
User space:
  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐
  │ POSIX    │  │ Network  │  │ FS +     │  │ Other    │
  │ Server   │  │ Stack    │  │ Blk Drv  │  │ Services │
  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘
       └──────────────┴────────────┴──────────────┘
                          │
              Synchronous Message Passing IPC
                          │
Kernel (S-mode): ┌────────┴────────┐
                 │  Capability     │
                 │  IPC Router     │
                 │  Scheduler      │
                 │  Memory Manager │
                 └────────┬────────┘
                          │
Firmware (M-mode):  RustSBI
                          │
Emulator:            machina
```

### 1.3 Design Principles

- **Small trusted base**: kernel <5000 LOC, auditable
- **No policy in kernel**: kernel enforces mechanisms, user space defines policy
- **Explicit resource management**: every resource accessed via a capability token
- **Zero-copy data paths**: shared memory mapping for bulk IPC transfers
- **Deterministic IPC latency**: priority inheritance prevents unbounded inversion

---

## 2. Memory Layout & Boot

### 2.1 Physical Memory (128MB DRAM @ 0x80000000)

```
0x80000000 ┌──────────────────┐
           │  RustSBI          │  ~128KB (loaded by machina)
0x80020000 ├──────────────────┤
           │  Kernel .text     │
           │  Kernel .rodata   │
           │  Kernel .data/.bss│
           ├──────────────────┤
           │  Kernel heap      │  grows upward (page frames, TCBs, EPs, CNode storage)
           ├─ ─ ─ ─ ─ ─ ─ ─ ─ ┤  ← dynamic boundary
           │  User pages       │  allocated downward from 0x80800000
0x80800000 ├──────────────────┤
           │  Page Table Pool  │  pre-allocated L0/L1/L2 page table pages
0x88000000 └──────────────────┘  end of DRAM
```

### 2.2 Virtual Memory (Sv39, per-process)

```
User space (low 256GB):
  0x00000000_0000 - 0x00000040_0000   program text/data (ELF load)
  0x00000040_0000 - 0x3FFFFFFF_FFFF   heap / mmap / shared memory windows

Kernel space (high 256GB):
  0xFFFFFFC0_0000_0000 - end          identity-mapped physical memory
```

### 2.3 Boot Sequence

1. machina loads RustSBI at 0x80000000
2. RustSBI loads kernel ELF at 0x80020000, jumps to `_start` in S-mode
3. `_start`: clear BSS, set per-HART kernel stack, set `sscratch`, call `rust_main`
4. `rust_main(hart_id)`:
   - Init console (SBI ecall for debug output)
   - Init buddy allocator over physical memory range
   - Build Sv39 page table (kernel identity map, 128MB)
   - `sfence.vma` + `csrw satp` + `fence.i` → enable MMU
   - Init kernel heap (linked list allocator)
   - Set `stvec` to trap vector
   - Init CLINT timer (10ms tick)
   - Init IPC subsystem (endpoint table)
   - Init capability root (root CNode)
   - Init scheduler (idle task + init task)
   - Create init process from embedded ELF binary
   - `start_scheduler()` → never returns

---

## 3. Capability System

### 3.1 Concept

A capability is an unforgeable token that grants access to a kernel resource. Every process has a **capability space** (CNode tree). Without a cap, a process cannot touch a resource.

### 3.2 Capability Types

| Type | Represents | Rights |
|------|-----------|--------|
| `Mem` | Physical memory page | R, W, X, Map (can map into page table) |
| `EP` | IPC endpoint | Send, Receive, Both |
| `Proc` | Target process | Spawn-Thread, Destroy, Monitor |
| `CNode` | Nested capability node | — |
| `Null` | Empty slot | — |

### 3.3 CNode Structure

Each CNode is a fixed-size array of slots (power of 2, typically 256). Slots hold typed capabilities. CNodes can be nested to form trees.

### 3.4 Operations

| Operation | Semantics |
|-----------|-----------|
| `Mint(src, rights_mask)` | Create derived cap with reduced rights. Establishes parent-child relationship. |
| `Copy(src, dest_proc, dest_slot)` | Shallow copy of a cap to another process's CNode. |
| `Move(src, dest_proc, dest_slot)` | Transfer cap; source slot becomes Null. |
| `Revoke(src)` | Recursively destroy all caps derived from this one. |
| `Delete(slot)` | Remove cap from slot. If last reference, free the resource. |

### 3.5 Reference Counting & Derivation Tree

- Every kernel resource object has a reference count incremented by each cap pointing to it.
- `Mint` creates a child derivation; `Revoke` walks the derivation tree.
- When refcnt reaches 0, the resource is freed.
- Derivation tree guarantees: revoke a parent → all children become invalid.

---

## 4. IPC System

### 4.1 Endpoint Model

An endpoint is a kernel object connecting sender(s) to receiver(s). Each endpoint has exactly one receiver and (potentially) multiple senders.

```
  sender A ──┐
              ├──► [EP] ──► receiver (blocked in recv)
  sender B ──┘
```

### 4.2 Message Format

```
┌────────────────┬──────────┬─────────────────────────────────┐
│ sender_pid u32 │ opcode   │  payload: [u8; 64]               │
│                │ u16      │                                  │
├────────────────┴──────────┼─────────────────────────────────┤
│  cap_transfers: [CapTransfer; 4]  (inline caps to copy/move) │
└─────────────────────────────────────────────────────────────┘

CapTransfer:
  - src_slot: u32   (sender's CNode slot index)
  - dest_slot: u32  (receiver's CNode slot index)
  - mode: Copy | Move
```

### 4.3 Transfer Modes

**Short message** (payload <= 64 bytes): Register-based, single memcpy in kernel. No allocation.

**Long message** (payload > 64 bytes): Sender mints a Mem cap (R-only), sends via short message. Receiver maps the shared page. Zero-copy data access.

### 4.4 System Calls

| Syscall | Args | Returns | Description |
|---------|------|---------|-------------|
| `ep_create(rights)` | EP rights mask | EP cap | Create new endpoint |
| `ep_delete(ep_slot)` | CNode slot | — | Destroy endpoint |
| `send(ep_slot, msg_ptr, cap_count)` | — | error code | Non-blocking send. Fails if no receiver. |
| `recv(ep_slot)` | — | (msg, caps) | Block until message arrives. |
| `call(ep_slot, msg_ptr, cap_count)` | — | reply msg | Send + block for reply (RPC). |
| `reply(call_id, msg_ptr, cap_count)` | — | error code | Reply to a pending call. |

### 4.5 Priority Inheritance for IPC

The kernel automatically applies priority inheritance to prevent inversion:

1. `send`: receiver inherits max(sender.prio, receiver.prio) until recv completes.
2. `call`: server inherits caller's priority while processing; caller's priority boosted on reply path.
3. Transitive: A→B→C chain propagates A's priority to C.

Inheritance is ephemeral — cleared when the blocking operation ends.

---

## 5. Scheduler

### 5.1 Design

- 64 priority levels (0 = lowest, idle; 63 = highest).
- Bitmap for O(1) "find highest non-empty priority".
- Within same priority: round-robin, 10ms time slice.
- Strict priority: always run the highest-priority ready thread.

### 5.2 Data Structures

```
ready_queues: [LinkedList<Thread>; 64]
priority_bitmap: u64  (bit i set iff queue i is non-empty)
current: &Thread
```

### 5.3 State Machine

```
  spawn → Ready
  Ready → Running   (picked by scheduler)
  Running → Ready   (preempted: time slice exhausted or higher-priority thread woken)
  Running → Waiting (recv/call/reply on empty EP, or explicit yield)
  Waiting → Ready   (message arrives on waited EP)
  Running → Dead    (exit syscall or killed)
  Dead → (resource reclamation by parent or init)
```

### 5.4 Timer Interrupt

- CLINT timer fires every 10ms.
- Decrement current thread's time slice.
- If slice exhausted: move current to end of same-priority queue, pick next.
- If higher-priority thread became ready (via IPC wakeup): preempt immediately.

---

## 6. Memory Management

### 6.1 Buddy Allocator (Physical Pages)

Orders 0-11 (4KB to 8MB per block). Standard binary buddy.

```
API:
  alloc_pages(order: u8) → Option<PhysFrame>
  free_pages(frame: PhysFrame, order: u8)
```

### 6.2 Sv39 Page Table Management

Three-level page table: L2 (root) → L1 → L0 → 4KB page.

PTE format: `[PPN[53:10] | RSW[1:0] | D | A | G | U | X | W | R | V]`

RSW bits (software-defined):
- `RSW[1]`: COW (Copy-on-Write flag)
- `RSW[0]`: Shared (shared memory, refcnt > 1 expected)

### 6.3 Page Fault Handler

| Fault Type | Action |
|-----------|--------|
| Instruction page fault | Allocate page, map from ELF data, or kill process |
| Load page fault, COW=1 | Copy page, update both processes' PTEs, clear COW if refcnt==1 |
| Store page fault, COW=1 | Copy page, update both processes' PTEs, clear COW if refcnt==1 |
| Access violation | Kill process (no cap = no access) |

### 6.4 COW (Copy-on-Write)

Used for `fork`:
1. Duplicate parent's page table entries.
2. For each writable page: set W=0, COW=1 in both parent and child PTEs.
3. Increment page refcnt.
4. On write fault: allocate new page, memcpy, update faulting process's PTE (W=1, COW=0).
5. If other process now has the only reference: clear its COW bit and set W=1.

### 6.5 Operations

| Operation | Description |
|-----------|-------------|
| `map(proc, vaddr, frame, flags)` | Map phys frame into proc's page table at vaddr |
| `unmap(proc, vaddr)` | Remove mapping, flush TLB |
| `share(src_proc, src_vaddr, dst_proc, dst_vaddr, frame)` | Share page between two processes |
| `fork(parent)` → child | Create child process with COW-shared address space |

---

## 7. Process Model

### 7.1 Process Structure

```rust
struct Process {
    pid: u32,
    cap_space: CNode,          // root CNode for this process
    page_table_root: PhysFrame, // Sv39 root page table
    thread: Thread,             // single thread (v1)
    base_priority: u8,          // 0-63
    effective_priority: u8,     // base_priority max inherited_priority
    state: ProcessState,
    parent: Option<u32>,
}

struct Thread {
    tid: u32,
    owner: u32,                // pid
    trap_frame: TrapFrame,     // all 32 GPRs + sepc + sstatus + scause + stval
    kernel_stack: [u8; 8192],  // 8KB per-thread kernel stack
    state: ThreadState,
}
```

### 7.2 Lifecycle

1. **Spawn**: Load ELF binary, allocate CNode + page table, map text/data/bss, allocate user stack, set sepc=entry, enqueue as Ready.
2. **Run**: Scheduler picks the thread, restores trap frame, `sret` to user mode.
3. **Wait**: Thread issues `recv`/`call` → blocks on EP → moved to Waiting queue.
4. **Wake**: Message arrives → thread moved to Ready, potentially preempts current.
5. **Exit**: Thread calls `exit` → state=Dead, resources queued for reclamation. Parent notified.
6. **Kill**: Process with Proc cap calls `kill` → target moves to Dead.

### 7.3 First Process (init)

The kernel embeds the init binary in `.rodata`. During boot, the kernel:
1. Creates the first Process + Thread + CNode.
2. Loads the embedded init ELF.
3. Grants init the root capability (cap to the root CNode).
4. Init is responsible for spawning all other services via `spawn` syscalls.

### 7.4 v1 Limitation: Single-Threaded Processes

V1: 1 process = 1 thread. Multi-threading is a future extension. The thread struct is embedded directly in Process rather than stored in a Vec. This simplifies the scheduler and IPC wakeup logic.

---

## 8. Trap & Interrupt Handling

### 8.1 Trap Vector

Single `stvec` entry point in assembly (`__trap_vector`):

```
__trap_vector:
    // Atomically swap sp and sscratch
    csrrw sp, sscratch, sp
    // Now sp points to kernel stack, old sp is in sscratch
    // Save all GPRs to kernel stack (building TrapFrame)
    sd ra, 0*8(sp)
    sd t0, 1*8(sp)
    ...
    // Save sepc, scause, stval, sstatus
    csrr t0, sepc; sd t0, 32*8(sp)
    csrr t0, scause; sd t0, 33*8(sp)
    ...
    // Call Rust handler
    mv a0, sp          // a0 = &TrapFrame
    call handle_trap
    // Restore everything, sret
```

### 8.2 Trap Dispatch (Rust)

```rust
fn handle_trap(tf: &mut TrapFrame) {
    match tf.scause {
        // Interrupts
        trap::interrupt::SUPERVISOR_TIMER => timer_interrupt(tf),
        trap::interrupt::SUPERVISOR_EXTERNAL => external_interrupt(tf),
        // Exceptions
        trap::exception::USER_ECALL => syscall_dispatch(tf),
        trap::exception::STORE_PAGE_FAULT | LOAD_PAGE_FAULT | INST_PAGE_FAULT => {
            page_fault_handler(tf)
        }
        _ => kill_process(tf, "unhandled trap"),
    }
}
```

### 8.3 Syscall Convention

RISC-V ecall convention:
- `a7` = syscall number
- `a0-a5` = arguments
- `a0-a1` = return values

---

## 9. Build System & Project Structure

### 9.1 Repository Layout

```
testOS/
├── machina/                  # existing machina emulator (unchanged)
├── rustsbi/                  # existing RustSBI firmware (unchanged)
├── TrainOS/                  # new TrainOS codebase
│   ├── Cargo.toml            # workspace
│   ├── kernel/               # kernel crate (no_std, rv64gc target)
│   │   ├── Cargo.toml
│   │   ├── link.ld           # linker script
│   │   ├── build.rs
│   │   └── src/
│   │       ├── main.rs       # entry, boot, rust_main
│   │       ├── arch/         # RISC-V specific (trap asm, CSR ops, sfence)
│   │       ├── cap/          # capability system
│   │       ├── ipc/          # IPC router, endpoint
│   │       ├── sched/        # scheduler
│   │       ├── mem/          # buddy allocator, Sv39, COW
│   │       ├── proc/         # process/thread management
│   │       ├── trap/         # trap dispatch
│   │       └── syscall/      # syscall table + handlers
│   ├── services/             # user-space service binaries
│   │   ├── init/             # root init service
│   │   ├── posix/            # POSIX compatibility server
│   │   ├── fs/               # filesystem service
│   │   └── net/              # network stack service
│   ├── lib/                  # shared libraries
│   │   ├── libtros/          # TrainOS syscall bindings (crate for user programs)
│   │   └── libipc/           # IPC message serialization
│   └── tests/                # integration tests
├── docs/
│   └── superpowers/
│       ├── specs/            # design specs
│       └── plans/            # implementation plans
└── README.md
```

### 9.2 Build Targets

- `kernel`: `riscv64gc-unknown-none-elf`, built with `cargo build --release -p kernel`
- `services/*`: Same target, embedded as binary blobs in kernel via `include_bytes!`
- `lib/*`: `riscv64gc-unknown-none-elf`, linked by services

### 9.3 Run Command

```bash
cargo build --release -p kernel
./machina/target/release/machina \
  -M riscv64-ref \
  -bios rustsbi/rustsbi-riscv64-machina-fw_dynamic.bin \
  -kernel TrainOS/target/riscv64gc-unknown-none-elf/release/kernel \
  -nographic
```

---

## 10. Testing Strategy

### 10.1 Unit Tests (kernel)

Run on host (x86-64) for pure-logic modules (cap, buddy allocator, IPC message parsing, scheduler queue manipulation). Anything touching CSRs or page tables requires machina.

### 10.2 Integration Tests (machina)

- Boot test: kernel starts, prints banner, reaches idle loop
- Syscall test: spawn a trivial "hello world" user process, verify it runs
- IPC test: two processes communicate via endpoint
- COW test: fork a process, verify COW page fault triggers correctly
- Stress test: N processes ping-ponging messages

### 10.3 Quality Gates

1. `cargo build` succeeds for all crates
2. `cargo clippy` passes with zero warnings
3. All tests pass on host
4. All integration tests pass on machina

---

## 11. V1 Milestone Scope

The first milestone delivers a **bootable microkernel with IPC**:

- [x] Boot to S-mode, enable MMU, reach Rust main
- [ ] Physical page allocator (buddy)
- [ ] Sv39 page table management (map, unmap, COW)
- [ ] Capability system (CNode, Mint, Copy, Move, Revoke, Delete)
- [ ] IPC endpoints (create, send, recv, call, reply)
- [ ] Scheduler (64 priority levels, round-robin within level, priority inheritance)
- [ ] Process spawn from ELF, single-threaded
- [ ] Timer interrupts (10ms tick)
- [ ] init service: a user-space process that prints "TrainOS ready" via SBI console
- [ ] Integration test: spawn two processes that IPC-pingpong

Out of scope for v1:
- SMP (multi-core)
- Filesystem (beyond embedded init)
- Network
- POSIX server
- Multi-threading

---

## Appendix A: Known Design Decisions & Rationale

| Decision | Rationale |
|----------|-----------|
| 64 priority levels | Fits in a u64 bitmap for O(1) scheduling; more than enough for embedded/workstation |
| 10ms time slice | Standard choice; balances responsiveness vs context-switch overhead |
| 8KB kernel stack | Enough for trap handling + IPC + page fault without overflow |
| Max 64-byte short message | Fits in roughly a cache line; common-case syscalls need fewer bytes |
| Single-threaded v1 | Scheduler, IPC wakeup, and TLB shootdown are simpler; add later with well-defined interface |
| Sv39 (not Sv48) | All RISC-V hardware supports Sv39; 256GB user space is more than enough for v1; upgrade path is designed in |
| Buddy allocator | Good fragmentation/performance tradeoff for 128MB; simple to implement |
| Embedded service binaries | Avoids need for a filesystem or bootloader in v1 |
