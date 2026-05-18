# TrainOS — Claude Code Context

## Project Overview
TrainOS is a microkernel OS in Rust for RISC-V 64-bit (rv64gc).
Uses RustSBI as boot firmware, runs on machina emulator.

**Goal**: Surpass Linux in kernel architecture, security, and performance.

## Iron Rules
1. Runtime: RustSBI (M-mode) + machina (RISC-V JIT emulator). Non-negotiable.
2. Architecture: RISC-V 64-bit (rv64gc), Sv39 virtual memory, MIT license.
3. Language: Rust nightly (`no_std` kernel + user-space, no heap in services).

## Current Status (2026-05-18) — V17.0

### Completed
- **Dynamic process spawning**: `sys_spawn` (syscall 3) creates new processes from user-provided ELF data
- **Process execution**: `sys_exec` (syscall 7) loads ELF from VFS and replaces current process image
- **POSIX I/O rewrite**: Per-process fd table (64 slots), proper path-based VFS forwarding, stdin/stdout/stderr
- **Process time accounting**: utime/stime tracking per process, wired into timer ticks and syscall dispatch
- **Shell V2**: Real ps (proclist), VFS-backed read/write/cat/ls, perf/mem/pid/date commands
- **Kernel print macros**: `println!()` and `print!()` using `core::fmt::Write`
- **Process crash isolation**: Unknown traps kill offending process instead of hanging kernel
- **main.rs refactored**: 1482 lines → ~260 lines using `spawn_service!` macro
- **TCPv2 Service**: Retransmission timer with exponential backoff, congestion window, slow start
- **VFS Service**: Directory tree, 16 file slots, /proc virtual filesystem
- **Namespaces**: UTS namespace (hostname isolation), PID namespace
- **Device driver framework**: Register/unregister/list drivers
- **CPU affinity**: sched_setaffinity/getaffinity
- **Resource tracking**: getrusage, times, sysinfo
- **81+ syscalls**: Full POSIX I/O, sockets, epoll, mmap, filesystem, time, process
- **VFS Service**: Enhanced FS service at EP 2 with directory tree, 16 file slots, CREATE/READ/WRITE/APPEND/DELETE/LIST/STAT operations
- **procfs**: Virtual /proc filesystem with /proc/uptime, /proc/meminfo, /proc/perf, /proc/version, /proc/proc, /proc/self
- SMP 2.0: Active IPI on IPC wakeup, per-CPU pick counts
- 35+ user-space services including TCP, procfs, HTTP server
- VirtIO block I/O, PCI enumeration, TFS journaling FS
- Capability system, 39+ syscalls, COW fork, POSIX compatibility
- Network stack with port-based datagram routing + TCP reliable streams

### Architecture
**Microkernel** — kernel provides:
- Capability system (CNode, Mint/Copy/Move/Revoke/Delete)
- Synchronous IPC (endpoint send/recv, 64-byte payload, cap transfer)
- 64-priority SMP scheduler (spinlock, bitmap O(1), soft affinity)
- Buddy allocator + Sv39 page tables + COW + process crash isolation

User-space services communicate via IPC. Well-known endpoints: EP 1 (init), EP 2 (VFS), EP 3 (NET).

## Build & Run

```bash
# Build all services
cd TrainOS && make services

# Build kernel (includes embedded service binaries)
cargo build --release -p kernel

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
| `main.rs` | Entry point, boot sequence, `spawn_service!` macro, service spawning |
| `console.rs` | KernelWriter, `println!`/`print!` macros via SBI console |
| `sync.rs` | SpinLock primitive (atomic CAS with pause) |
| `per_cpu.rs` | Per-CPU data (hart_id, current, idle) |
| `mem/buddy.rs` | Buddy allocator (orders 0-12) |
| `mem/sv39.rs` | Sv39 page tables, MMU, copy_kernel_mappings, make_satp |
| `mem/layout.rs` | Physical/virtual address constants |
| `mem/heap.rs` | Kernel heap bump allocator (KernelAllocator wrapper) |
| `trap/asm.rs` | Trap entry/exit assembly (35-field frame, 280 bytes) |
| `trap/mod.rs` | TrapFrame, dispatch, CLINT timer, page fault with COW, process kill on crash |
| `proc/process.rs` | Process (pid, state, PT root, CNode id, thread, uid/gid) |
| `proc/thread.rs` | Thread, TaskContext (with satp), WaitTarget |
| `proc/switch.rs` | context_switch + user_trap_return assembly |
| `proc/elf.rs` | ELF64 loader with process-private PT helpers |
| `proc/mod.rs` | Process manager (spawn, fork_child, PROCESSES table) |
| `sched/mod.rs` | 64-priority SMP scheduler (SpinLock, bitmap, ThreadQueue) |
| `cap/types.rs` | CapType, Rights, Resource, Slot, ResourceData |
| `cap/ops.rs` | alloc_resource, mint, copy_cap, move_cap, delete_cap, revoke |
| `cap/mod.rs` | Global RESOURCES table |
| `ipc/message.rs` | Message, CapTransfer, TransferMode |
| `ipc/endpoint.rs` | Endpoint, send/recv with priority inheritance, IPI wakeup |
| `ipc/mod.rs` | Global ENDPOINTS table |
| `syscall/mod.rs` | Syscall dispatch table (40+ syscalls), MMIO proxy |
| `syscall/ipc.rs` | ep_create, send, recv with cap checks |
| `syscall/proc.rs` | spawn, exit, yield, mmio_map, fork, proclist, kill, blk_read/write, shm_map, signal, waitpid |
| `syscall/cap.rs` | mint, copy, move, delete, cap_stats with caller CNode |
| `syscall/posix.rs` | open, read, write, close, stat, lseek, dup, getcwd |

### User-space Services

| Service | EP | Purpose |
|---------|-----|---------|
| `init/` | 1 | System init, IPC receiver |
| `fs/` | 2 | VFS: directories, files, procfs virtual files |
| `net/` | 3 | Network stack: port registration, datagram routing |
| `tcp/` | dynamic | TCP reliable stream protocol (3-way handshake, seq/ack, teardown) |
| `echo/` | dynamic | Echo service (port 7) |
| `http/` | 8 | HTTP server |
| `sh/` | dynamic | Interactive shell |
| `proc/` | dynamic | Process listing and management |
| `pci/` | dynamic | PCI device enumeration |
| `veth/` | dynamic | Virtual Ethernet driver |
| `tfs/` | dynamic | TFS file system service |
| `tfs_jrnl/` | dynamic | TFS journaling layer |
| `drv/` | dynamic | VirtIO block/network driver |
| `demo/` | dynamic | System demo (IPC/FS/MEM/CAP/PERF checks) |
| `stress/` | dynamic | Block I/O stress test |
| `bench/` | dynamic | Performance benchmark suite |

## V17.0 Changes (2026-05-18)

### Kernel Improvements
- **`println!()` / `print!()` macros**: `core::fmt::Write`-based kernel printing eliminates ~500 lines of manual digit-by-digit printing
- **Process crash isolation**: `kill_current_process()` in trap handler kills offending process instead of hanging the kernel on unknown traps. Null pointer dereferences and unhandled page faults now gracefully terminate the process.
- **Refactored main.rs**: `spawn_service!` macro reduces ~1200 lines of repetitive spawn+print code to ~35 concise invocations. Services organized by priority group with rationale comments.

### TCP Service (new)
- User-space service implementing TCP state machine
- 3-way handshake (SYN → SYN-ACK → ACK)
- Reliable in-order data delivery with sequence numbers and ACKs
- Connection teardown (FIN → FIN-ACK)
- Allocated statically (no heap), supporting up to 8 concurrent connections

### VFS Service (rewritten)
- Directory tree support (/, /proc, /home, /etc, /tmp)
- 16 file slots with path-based lookup
- Operations: READ, WRITE, APPEND, DELETE, LIST, STAT
- procfs virtual files: /proc/uptime, /proc/meminfo, /proc/perf, /proc/version, /proc/proc, /proc/self

## Commit Convention
Format: `type: description` (conventional commits)
Types: feat, fix, docs, refactor, test
Example: `feat: add TCP service with reliable stream protocol`
