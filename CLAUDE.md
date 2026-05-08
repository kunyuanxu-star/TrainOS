# TrainOS — Claude Code Context

## Project Overview
TrainOS is a microkernel OS in Rust for RISC-V 64-bit (rv64gc).
Uses RustSBI as boot firmware, runs on machina emulator.

**Goal**: Surpass Linux in kernel architecture, security, and performance.

## Iron Rules
1. Runtime: RustSBI (M-mode) + machina (RISC-V JIT emulator). Non-negotiable.
2. Architecture: RISC-V 64-bit (rv64gc), Sv39 virtual memory, MIT license.
3. Language: Rust nightly (`no_std` kernel + user-space, no heap in services).

## Current Status (2026-05-08) — V8.0

### Completed
- SMP 2.0: Active IPI on IPC wakeup, per-CPU pick counts
- 24 user-space services: init, ping, fs, test_fs, sh, test_fork, uart, test_posix, drv, net, echo, test_net, test_c, proc, test_proc, demo, stress, bb, pci, veth, tfs, tfs_jrnl, edit, cat
- VirtIO block I/O: Full virtqueue management, sector read/write via kernel proxy
- Proc service: Process listing (pid, prio, state) and kill capability
- Demo service: System health check (IPC, FS, MEM, CAP, PERF)
- Block I/O stress/benchmark: stress and bb services for storage testing
- PCI enumeration: pci service for device discovery
- Virtual Ethernet: veth service for network device access
- TFS journaling file system: tfs + tfs_jrnl services for persistent storage
- Text utilities: edit (editor) and cat (file viewer) services
- Per-process CNode capability enforcement
- POSIX open/read/write/close syscalls (IPC→FS translation)
- COW fork with full page table deep-copy and page fault handler
- IPC priority inheritance
- Network stack with port-based datagram routing
- C/ASM program support via Python ELF64 generator
- VirtIO MMIO kernel proxy (read32/write32), block device detected

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
| `syscall/mod.rs` | Syscall dispatch table (31 syscalls) |
| `syscall/ipc.rs` | ep_create, send, recv with cap checks |
| `syscall/proc.rs` | spawn, exit, yield, mmio_map, fork, proclist, kill, blk_read, blk_write |
| `syscall/cap.rs` | mint, copy, move, delete, cap_stats with caller CNode |
| `syscall/posix.rs` | open, read, write, close (IPC→FS translation) |

### User-space

| Directory | Purpose |
|-----------|---------|
| `lib/tros/` | User-space syscall library (25+ wrappers) |
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
| `services/demo/` | System demo (IPC, FS, MEM, CAP, PERF checks) |
| `services/stress/` | Block I/O stress test |
| `services/bb/` | Block I/O benchmark |
| `services/pci/` | PCI device enumeration |
| `services/veth/` | Virtual Ethernet driver |
| `services/tfs/` | TFS file system service |
| `services/tfs_jrnl/` | TFS journaling layer |
| `services/edit/` | Text editor |
| `services/cat/` | File viewer |

## Known Issues
- `%` operator broken on RISC-V release mode: use `n - (n/10)*10` instead
- PMP blocks user-mode MMIO access below 0x80000000 (needs firmware update)
- VecDeque unavailable (nightly `no_global_oom_handling`): use custom Vec+head queue
- spin::Mutex not SMP-safe for scheduler: replaced with custom SpinLock
- (FIXED V8.0A) WFI starvation: completed services now call `tros::exit(0)` instead of `loop { wfi }`. Added `sys_yield()` syscall (nr=6) and `tros::yield_cpu()` for voluntary yielding.

## Commit Convention
Format: `type: description` (conventional commits)
Types: feat, fix, docs, refactor, test
Example: `feat: add SMP multi-core support (V2.1)`
