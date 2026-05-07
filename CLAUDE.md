# TrainOS — Claude Code Context

## Project Overview
TrainOS is a microkernel OS in Rust for RISC-V 64-bit (rv64gc).
Uses RustSBI as boot firmware, runs on machina emulator.

**Goal**: Surpass Linux in kernel architecture, security, and performance.

## Iron Rules
1. Runtime: RustSBI (M-mode) + machina (RISC-V JIT emulator). Non-negotiable.
2. Architecture: RISC-V 64-bit (rv64gc), Sv39 virtual memory, MIT license.
3. Language: Rust nightly (`no_std` kernel + user-space, no heap in services).

## Current Status (2026-05-07) — V2.5

### Completed
- SMP multi-core (2 HARTs, spinlock scheduler, per-CPU data, per-HART CLINT)
- Per-process CNode capability enforcement
- POSIX open/read/write/close syscalls (IPC→FS translation)
- COW fork with full page table deep-copy and page fault handler
- IPC priority inheritance (receiver inherits sender priority)
- Network stack service with port-based datagram routing
- VirtIO MMIO mapping framework (PMP blocks actual MMIO in user-mode)
- 11 user-space services running concurrently

### Architecture
**Microkernel** — kernel provides:
- Capability system (CNode, Mint/Copy/Move/Revoke/Delete)
- Synchronous IPC (endpoint send/recv, 64-byte payload, cap transfer)
- 64-priority SMP scheduler (spinlock, bitmap O(1), soft affinity)
- Buddy allocator + Sv39 page tables + COW

User-space services communicate via IPC. Well-known endpoints: EP 1 (init), EP 2 (FS), EP 3 (NET).

## Build & Run

```bash
# Build kernel (includes embedded service binaries)
cd TrainOS && cargo build --release -p kernel

# Run on machina (from testOS/)
./machina/target/release/machina \
  -M riscv64-ref -smp 2 \
  -bios machina/pc-bios/rustsbi-riscv64-machina-fw_dynamic.bin \
  -kernel TrainOS/target/riscv64gc-unknown-none-elf/release/kernel \
  -nographic
```

## Key Files

### Kernel (`kernel/src/`)

| File | Purpose |
|------|---------|
| `main.rs` | Entry point, boot sequence, service spawning |
| `sync.rs` | SpinLock primitive (atomic CAS with pause) |
| `per_cpu.rs` | Per-CPU data (hart_id, current, idle) |
| `mem/buddy.rs` | Buddy allocator (orders 0-12) |
| `mem/sv39.rs` | Sv39 page tables, MMU, copy_kernel_mappings, make_satp |
| `mem/layout.rs` | Physical/virtual address constants |
| `mem/heap.rs` | Kernel heap bump allocator (KernelAllocator wrapper) |
| `trap/asm.rs` | Trap entry/exit assembly (35-field frame, 280 bytes) |
| `trap/mod.rs` | TrapFrame, dispatch, CLINT timer, page fault, IPI |
| `proc/process.rs` | Process (pid, state, PT root, CNode id, thread) |
| `proc/thread.rs` | Thread, TaskContext (with satp), WaitTarget |
| `proc/switch.rs` | context_switch + user_trap_return assembly |
| `proc/elf.rs` | ELF64 loader with process-private PT helpers |
| `proc/mod.rs` | Process manager (spawn, fork_child, PROCESSES table) |
| `sched/mod.rs` | 64-priority scheduler (SpinLock, bitmap, ThreadQueue) |
| `cap/types.rs` | CapType, Rights, Resource, Slot, ResourceData |
| `cap/ops.rs` | alloc_resource, mint, copy_cap, move_cap, delete_cap, revoke |
| `cap/mod.rs` | Global RESOURCES table |
| `ipc/message.rs` | Message, CapTransfer, TransferMode |
| `ipc/endpoint.rs` | Endpoint, send/recv with priority inheritance |
| `ipc/mod.rs` | Global ENDPOINTS table |
| `syscall/mod.rs` | Syscall dispatch table (18 syscalls) |
| `syscall/ipc.rs` | ep_create, send, recv with cap checks |
| `syscall/proc.rs` | spawn, exit, mmio_map, fork helpers |
| `syscall/cap.rs` | mint, copy, move, delete with caller CNode |
| `syscall/posix.rs` | open, read, write, close (IPC→FS translation) |

### User-space

| Directory | Purpose |
|-----------|---------|
| `lib/tros/` | User-space syscall library (15+ wrappers) |
| `services/init/` | System init, IPC receiver (EP 1) |
| `services/ping/` | IPC send demo |
| `services/fs/` | File system service (EP 2, READ/WRITE ops) |
| `services/test_fs/` | FS RPC test client |
| `services/sh/` | Interactive shell (help, echo, read, write, ps) |
| `services/test_fork/` | COW fork test |
| `services/test_posix/` | POSIX API test |
| `services/drv/` | VirtIO MMIO driver framework |
| `services/net/` | Network stack (EP 3, port routing) |
| `services/echo/` | Echo service (port 7) |
| `services/test_net/` | Network stack test client |

## Known Issues
- `%` operator broken on RISC-V release mode: use `n - (n/10)*10` instead
- PMP blocks user-mode MMIO access below 0x80000000 (needs firmware update)
- VecDeque unavailable (nightly `no_global_oom_handling`): use custom Vec+head queue
- spin::Mutex not SMP-safe for scheduler: replaced with custom SpinLock
- test_fork wfi loop starves lower-priority services (pre-existing scheduler quirk)

## Commit Convention
Format: `type: description` (conventional commits)
Types: feat, fix, docs, refactor, test
Example: `feat: add SMP multi-core support (V2.1)`
