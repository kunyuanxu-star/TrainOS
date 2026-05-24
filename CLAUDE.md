# TrainOS — Claude Code Context

## Project Overview
TrainOS is a microkernel OS in Rust for RISC-V 64-bit (rv64gc).
Uses RustSBI as boot firmware, runs on machina emulator.

**Goal**: Surpass Linux in kernel architecture, security, and performance.

## Development Roadmap

The V21–V30 roadmap is defined in [docs/specs/2026-05-18-trainos-v21-v30-roadmap.md](docs/specs/2026-05-18-trainos-v21-v30-roadmap.md).

| Phase | Theme |
|-------|-------|
| V21 | Formal verification & security hardening |
| V22 | High-performance async I/O (io_uring) |
| V23 | Virtualization & hypervisor |
| V24 | Programmable kernel extensions (eBPF-like) |
| V25 | NUMA scalability (256+ cores) |
| V26 | Distributed IPC & remote memory |
| V27 | Defense in depth (CHERI, ASLR, sandbox) |
| V28 | WASM/WASI universal runtime |
| V29 | AI-native OS (GPU, tensor accelerators) |
| V30 | Production readiness & Linux ABI compatibility |

## Iron Rules
1. Runtime: RustSBI (M-mode) + QEMU (RISC-V `-machine virt`).
2. Architecture: RISC-V 64-bit (rv64gc), Sv39 virtual memory, MIT license.
3. Language: Rust nightly (`no_std` kernel + user-space, no heap in services).

## Current Status (2026-05-24) — V30.0 (Complete)

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

### Wave 1 (V21-V23) — 2026-05-24

#### V21 — Formal Verification & Security Hardening
- **Kernel invariant checks**: Scheduler (bitmap/queue cross-validation), memory (allocated+free==total), IPC (wait queue cycle detection), W^X, stack canary
- **Periodic trigger**: Every 100 timer ticks, all invariants verified
- **Capability security**: `sys_mint` parent-rights enforcement, 256-entry audit log with timestamps, cap leak detection on process exit
- **Memory safety**: Heap canary (0xDEAD_BEEF_CAFE_BABE) on alloc/free, user buffer bounds checking in read/write, W^X auto-enforcement (clears X on W+X pages), kernel stack guard page overflow detection
- **Syscall audit**: Per-process seccomp filter (16 rules), global syscall frequency counters (`SYS_SYSCALL_STATS` nr=132), sensitive operation audit (kill/mmap/munmap/mprotect)

#### V22 — High-Performance Async I/O (io_uring)
- **io_uring core**: Real VFS IPC dispatch for READ/WRITE/OPEN/CLOSE/STAT in `execute_sqe`, per-process SQ/CQ ring buffers
- **Shared memory rings**: SQ/CQ physical pages mapped into user address space, zero-copy data path via `share_page` and `splice_pages`
- **Block device layer**: Request merging (adjacent sector coalescing), per-CPU blk-mq submission queues, pluggable I/O scheduler framework (`NoopScheduler` + `DeadlineScheduler` with read/write deadlines)

#### V23 — Virtualization & Hypervisor
- **H-extension CSRs**: `hgatp`, `hstatus`, `hedeleg`, `hideleg`, `vsstatus`, `vstvec` read/write wrappers
- **Two-stage address translation**: G-stage page table creation/destruction, guest physical → host physical mapping
- **VM lifecycle**: `vm_create/destroy/start/pause/resume` with full GPR and CSR context save/restore
- **VirtIO backend**: Guest MMIO decode and forwarding to host driver services
- **Paravirtual timer + PLIC**: Offset-based time CSR, timer compare with interrupt injection, 64-IRQ virtual PLIC
- **Snapshot/restore**: Full VM state serialization (GPRs, CSRs, G-stage metadata) with magic-number validation

### Wave 2 (V24/V25/V28) — 2026-05-24

#### V24 — Programmable Kernel Extensions (eBPF-like)
- **Bytecode verifier**: DFS-based reachability analysis, back-edge detection, register bounds checking, scratch memory OOB detection, 512-byte max
- **Sandboxed interpreter**: 12 opcodes (MOV/ADD/SUB/CMP/JMP/JE/JNE/LOAD/STORE/PUSH/POP/RET), 32 virtual u64 regs, 256B scratch buffer, 1000-cycle budget with timeout
- **Hook points**: SYSCALL_ENTER/EXIT (dispatch), TIMER (tick handler), IPC_SEND (endpoint send path)
- **Example extensions**: syscall tracer, packet counter, performance monitor (as const bytecode arrays)
- **Security**: isolated register set, bounds-checked scratch memory, per-invocation cycle budget, auto-disable on violation

#### V25 — NUMA Scalability
- **Per-node ready queues**: 64-priority ThreadQueue per node with independent bitmaps
- **EEVDF scheduler**: Deadline-based ordering, vruntime tracking, weight-weighted time slices, `push_sorted_by_deadline()`
- **Load balancing**: Every 1000 ticks, migrate from busiest to idlest node if imbalance > 25%
- **Synchronization**: Per-CPU counters (AtomicU64 per hart), MCS lock (cache-friendly queued spinning), RCU grace-period tracking
- **Memory sharding**: Per-node allocation stats, local-first `node_alloc_page()` with remote fallback, `migrate_page()` with data copy
- **Topology discovery**: `register_node()` for multi-node config, QEMU virt default single-node

#### V28 — WASM/WASI Runtime
- **WASM interpreter**: 36 opcodes (i32/i64 const, arithmetic, bitwise, shifts, comparisons, load/store, memory ops, control flow), 256-slot value stack, 32-frame call stack, 10k-cycle budget
- **Module management**: Section parsers (Type/Import/Function/Export/Code), 16-module registry, 64KB linear memory (expandable to 256KB)
- **WASI preview2**: 21 host functions (`fd_read/write/close/seek`, `clock_time_get`, `random_get`, `proc_exit`, environ/args stubs)
- **libOS mode**: Runtime RISC-V ELF generation for spawning WASM as standalone process, direct kernel-context execution
- **Host function table**: 16 slots for native Rust ↔ WASM interop

### Wave 3 (V26/V27/V29) — 2026-05-24

#### V26 — Distributed IPC & Remote Memory
- **Node discovery**: Ping/pong heartbeat protocol, 500-tick periodic probe, dead node detection
- **Remote messaging**: `remote_send/recv` via TCP net service, serialized wire protocol (ping/pong/data/cap/mem_alloc/mem_free/proclist packet types)
- **Distributed cap passing**: `remote_mint` with serialized capability transfer format
- **Remote memory pooling**: `RemoteMemPool` per-node tracking, `remote_alloc_page/free`, page migration across nodes
- **Cluster PID namespace**: `(node_id << 24) | local_pid` encoding, cross-node process list RPC

#### V27 — Defense in Depth (CHERI + ASLR + Sandbox)
- **CHERI capability table**: 16 caps per process, `validate_ptr()` on syscalls, `/proc/cheri` status
- **KASLR**: Kernel base slide (0-255 pages) randomized at boot, entropy > 30 bits
- **ASLR enhancement**: Per-process stack randomization, heap randomization, `aslr_entropy()` reporting
- **Path sandbox**: 32 path-prefix rules, enforced in open/read/write/unlink/rename
- **Network sandbox**: 8 port-range rules per process, enforced in bind/connect
- **UID namespace**: 8-entry uid translation table, non-root "root" mapping

#### V29 — AI-Native OS (GPU + Tensor)
- **GPU driver**: Command ring submission via MMIO, fence polling, GART-style memory allocator (64 regions, 64KB each), MSI-X interrupt handling, utilization tracking
- **AI workload scheduler**: 4 priority levels (LOW/NORMAL/HIGH/REALTIME), FIFO within priority, time-slicing (1000-op quantum), preemption support, MPS (4 concurrent workloads per GPU)
- **Tensor operations**: MATMUL/CONV/RELU/SOFTMAX/ADD, F32/F16/INT8 dtypes
- **Model management**: 8 models per GPU, weight storage in GPU memory, load/unload/list
- **Inference pipeline**: `inference_submit` wraps tensor ops into workloads, latency stats tracking
- **17 new syscalls**: GPU_* (220-229), AI_* (222-231), MODEL_* (232-234), INFERENCE_* (235-236)

### Wave 4 (V30) — 2026-05-24

#### V30 — Production Readiness & Linux ABI Compatibility
- **42 new syscalls (240-281)**: System V IPC (semget/semop/semctl, msgget/msgsnd/msgrcv/msgctl), signals (sigaction/sigprocmask/sigreturn/rt_sigaction), termios (TCGETS/TCSETS/TIOCGWINSZ), filesystem (symlink/readlink/fsync/fdatasync/flock/fallocate/sendfile), process (prctl/getpriority/setpriority/sched_getparam/sched_setparam), memory (madvise/mincore/mlock/munlock), time (settimeofday, POSIX timers), socket (getsockopt/setsockopt/getpeername/getsockname/shutdown), poll/ppoll/pselect6
- **Linux ABI**: 120+ Linux→TrainOS syscall mappings, open/mmap flag translation, 34-value errno conversion, full ELF auxiliary vector (20 AT_* entries)
- **/proc filesystem**: cpuinfo, meminfo, mounts, stat, loadavg, uptime, per-process maps/status/cmdline/fd/
- **/sys device model**: devices, class/block, class/net, system/cpu
- **Dynamic linker**: .interp/.dynamic parsing, DT_NEEDED resolution, R_RISCV_RELATIVE/GLOB_DAT/JUMP_SLOT relocations
- **Self-hosting**: rustc/cargo runtime framework, 256MB memory requirement, cross→native bootstrap path
- **Production deployment**: QEMU/HiFive/VisionFive2/K230 platform configs, systemd-lite service manager (dependency-based boot, auto-restart), DHCP/DNS network config, package manager (install/remove/list)

### Architecture
**Microkernel** — kernel provides:
- Capability system (CNode, Mint/Copy/Move/Revoke/Delete)
- Synchronous IPC (endpoint send/recv, 64-byte payload, cap transfer)
- 64-priority SMP scheduler (spinlock, bitmap O(1), soft affinity)
- Buddy allocator + Sv39 page tables + COW + process crash isolation

User-space services communicate via IPC. Well-known endpoints: EP 1 (init), EP 2 (VFS), EP 3 (NET).

## Build & Run

```bash
# Build everything
cd TrainOS && make all

# Run on QEMU (interactive)
make run

# Run test suite
make test
```

Or manually:
```bash
qemu-system-riscv64 -machine virt -smp 2 -nographic \
  -bios rustsbi-qemu-new.bin \
  -kernel target/riscv64gc-unknown-none-elf/release/kernel
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
| `invariant.rs` | Kernel invariant checks (scheduler, memory, IPC, W^X, canary) |
| `security/mod.rs` | W^X enforcement, seccomp filter, cap audit log, stack canary |
| `iouring/mod.rs` | io_uring async I/O (SQ/CQ rings, real VFS dispatch) |
| `hypervisor/mod.rs` | VM lifecycle (create/destroy/start/pause/resume), VmContext |
| `hypervisor/csr.rs` | H-extension CSR wrappers (hgatp, hstatus, hedeleg, hideleg) |
| `hypervisor/mmu.rs` | G-stage two-stage address translation |
| `hypervisor/virtio.rs` | VirtIO MMIO backend for guest VMs |
| `hypervisor/timer.rs` | Paravirtual timer with offset-based time CSR |
| `hypervisor/plic.rs` | Virtual PLIC (64 IRQ sources, inject/claim/complete) |
| `hypervisor/snapshot.rs` | VM state serialization/deserialization |
| `device/sched.rs` | Pluggable I/O scheduler (Noop + Deadline) |
| `device/merge.rs` | Block request merging (adjacent sector coalescing) |
| `distributed/mod.rs` | Distributed IPC (remote node registry, endpoint publish/lookup) |
| `extension/mod.rs` | eBPF-like kernel extension framework |
| `numa/mod.rs` | NUMA-aware scheduling and memory allocation |
| `aslr/mod.rs` | ASLR (PCG randomization for stack, mmap, PIE) |
| `wasm/mod.rs` | WASM module loader and management |
| `ai/mod.rs` | GPU registry and AI workload scheduling |
| `compat/mod.rs` | Linux ABI syscall translation table |

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
