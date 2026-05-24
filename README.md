# TrainOS

A microkernel operating system written in Rust for RISC-V 64-bit (rv64gc). Runs on RustSBI firmware on QEMU.

**Goal**: Surpass Linux in kernel architecture, security, and performance — fully designed and implemented with AI.

**Current Version**: V34.0 | **Syscalls**: 295+ | **Kernel**: ~15,000 LOC | **License**: MIT

---

## Architecture

TrainOS is a **microkernel** — kernel mechanisms are minimal, everything else runs in user space:

| Subsystem | Capability |
|-----------|-----------|
| **Capability System** | CNode with Mint/Copy/Move/Revoke/Delete, parent-rights enforcement, audit logging |
| **IPC** | Synchronous message passing, priority inheritance, distributed IPC across nodes |
| **Scheduler** | NUMA-aware, EEVDF deadline-based, 64-priority per-node queues, SMP |
| **Memory Manager** | Buddy allocator, Sv39 page tables, transactional MMU, COW fork, W^X enforcement |
| **Security** | seccomp filter, CHERI software capabilities, ASLR/KASLR, PMP-based TEE enclaves |
| **Hypervisor** | RISC-V H-extension, two-stage address translation, VM lifecycle, VirtIO backend |

---

## Feature Matrix

### Kernel Services

| Category | Features |
|----------|----------|
| **Process** | spawn, fork(COW), exec, exit, kill, waitpid, signal, prctl, priority |
| **Memory** | mmap, munmap, mprotect, brk, shm_map, madvise, mincore, mlock, page sharing |
| **Filesystem** | open, read, write, close, stat, lseek, dup, getcwd, symlink, readlink, fsync, flock, fallocate, sendfile, ioctl(termios) |
| **Socket** | socket, bind, listen, accept, connect, sendto, recvfrom, getsockopt, setsockopt, shutdown |
| **IPC** | ep_create, send, recv, call, reply + System V semaphores (semget/semop/semctl), message queues (msgget/msgsnd/msgrcv) |
| **Time** | nanosleep, clock_gettime, gettimeofday, settimeofday, POSIX timers (timer_create/delete/settime/gettime) |
| **Poll** | poll, ppoll, pselect6, epoll_create/ctl/wait |
| **Namespace** | UTS (hostname), PID, user namespace with uid mapping |
| **Capability** | mint, copy, move, delete, cap_stats, cap_audit |
| **Device** | driver register/unregister/list, MMIO map/read/write, blk-mq, I/O scheduler |

### Advanced Subsystems

| Subsystem | Description |
|-----------|-------------|
| **io_uring** | Async I/O with per-process SQ/CQ rings, shared memory mapping, zero-copy splice |
| **eBPF Extensions** | Sandboxed bytecode verifier (DFS back-edge detection), 12-opcode interpreter, 4 hook types, 3 example extensions |
| **WASM Runtime** | 36-opcode stack interpreter, WASI preview2 (21 host functions), libOS mode, syscall-as-host-function (55 mappings), eBPF+WASM hybrid |
| **NUMA** | Per-node ready queues, EEVDF scheduling, load balancing, per-CPU counters, MCS lock, RCU, page migration |
| **Distributed IPC** | Node discovery (ping/pong heartbeat), remote messaging, distributed capability passing, remote memory pooling, cluster PID namespace |
| **GPU/AI** | GPU driver (command ring, fence, MSI-X), GART memory, AI workload scheduler (4-level priority + MPS), tensor ops (MATMUL/CONV/RELU/SOFTMAX), model registry, inference pipeline, P/D separation scheduler, KV-cache paged management |
| **Virtualization** | RISC-V H-extension CSRs, G-stage MMU, VM lifecycle (create/destroy/start/pause/resume), VirtIO backend, PV timer, virtual PLIC, snapshot/restore |
| **TEE** | PMP-based enclaves (16 entries), SHA-256 attestation, enclave secure IPC (32 channels), CPU+GPU heterogeneous TEE, TCB measurement |

### Security Hardening

| Mechanism | Implementation |
|-----------|---------------|
| **W^X** | Page table enforcement, auto-fix on violation |
| **ASLR** | PCG-based stack/mmap/PIE randomization, KASLR kernel slide (>30 bits entropy) |
| **Stack Canary** | 0xDEADBEEF_CAFEBABE guard, overflow detection in trap handler |
| **Heap Canary** | Pre/post allocation canary verification on free |
| **seccomp** | Per-process syscall filter (16 rules, allow/kill/log) |
| **CHERI** | Software 128-bit fat pointer, 16-cap per-process table, pointer validation |
| **Sandbox** | Path-based (32 rules), network port (8 rules/process), UID namespace |
| **Cap Audit** | 256-entry circular log with timestamps, leak detection on exit |

### Production Readiness

| Area | Features |
|------|----------|
| **Linux ABI** | 120+ syscall mappings, flag/errno translation, ELF auxv, dynamic linker (R_RISCV_RELATIVE/GLOB_DAT/JUMP_SLOT) |
| **/proc** | cpuinfo, meminfo, mounts, stat, loadavg, uptime, per-process maps/status/cmdline/fd |
| **/sys** | devices, class/block, class/net |
| **Service Manager** | Dependency-based boot, auto-restart (3 retries), start/stop/restart/list |
| **Network** | DHCP client, static IP, DNS resolver, /etc/hosts |
| **Package Manager** | install/remove/list, dependency tracking, /var/lib/pkgs database |
| **Hardware** | QEMU virt, SiFive HiFive Unmatched, StarFive VisionFive 2, Canaan K230 configs |

---

## Build & Run

```bash
# Prerequisites
rustup toolchain install nightly
rustup target add riscv64gc-unknown-none-elf
rustup component add rust-src

# Build everything
cd TrainOS && make all

# Run on QEMU (2 CPUs)
make run
```

Or manually:
```bash
qemu-system-riscv64 -machine virt -smp 2 -nographic \
  -bios rustsbi-qemu-new.bin \
  -kernel target/riscv64gc-unknown-none-elf/release/kernel
```

---

## Evolution Timeline

```
V1-V12    V13-V20     V21-V30          V31-V34
████████  ██████████  ██████████████   █████████
 基础      功能完善    路线图驱动         调研驱动
  内核     81+ syscall  10版本            4版本
  IPC      35+ 服务     15,050 行         5,700 行
```

| Phase | Versions | Approach | Key Outcome |
|-------|----------|----------|-------------|
| **Foundation** | V1-V12 | Incremental | Boot, MMU, scheduler, IPC, FS, network, VirtIO |
| **Feature** | V13-V20 | Feature-driven | TCP, VFS, namespaces, 35+ services, POSIX |
| **Roadmap** | V21-V30 | Plan-driven (10 versions in 4 waves) | Formal verification, io_uring, virtualization, eBPF, NUMA, distributed IPC, CHERI/ASLR, WASM, GPU/AI, Linux ABI |
| **Research** | V31-V34 | CCF-A paper research-driven | Transactional MMU (CortenMM SOSP'25), WASM hybrid (WABI EuroSys'25), TEE (TEEM³ ASPLOS'26), P/D scheduler (OSDI'24) |

### Research Foundation (V31-V34)

V31-V34 are based on a systematic survey of 27 CCF-A conference papers (SOSP/OSDI/EuroSys/ASPLOS/USENIX ATC 2024-2026). See [research report](os-ccfa-research-2024-2026/report.md) for full analysis.

---

## Project Structure

```
TrainOS/
├── kernel/src/                  # Kernel (~15,000 LOC)
│   ├── cap/                     # Capability system
│   ├── ipc/                     # IPC endpoints, messages
│   ├── syscall/                 # Syscall dispatch (295+), POSIX, memory, socket, fs
│   ├── mem/                     # Buddy allocator, Sv39 page tables, heap, TxMMU
│   ├── trap/                    # Trap asm, dispatch, page fault with COW
│   ├── proc/                    # Process, thread, switch, ELF loader
│   ├── sched/                   # EEVDF scheduler, NUMA-aware
│   ├── security/                # W^X, seccomp, cap audit, canary, TEE
│   ├── aslr/                    # ASLR, CHERI, sandbox
│   ├── iouring/                 # io_uring async I/O
│   ├── extension/               # eBPF-like kernel extensions
│   ├── hypervisor/              # RISC-V H-extension, VM lifecycle, VirtIO, PV timer
│   ├── numa/                    # NUMA scheduler, per-CPU counters, MCS lock, RCU
│   ├── distributed/             # Distributed IPC, remote memory
│   ├── wasm/                    # WASM interpreter, WASI, libOS, hostcall, hybrid
│   ├── ai/                      # GPU driver, AI scheduler, tensor ops, KV-cache
│   ├── compat/                  # Linux ABI, /proc, /sys, dynamic linker, deploy
│   ├── device/                  # Driver framework, blk-mq, I/O scheduler
│   ├── invariant.rs             # Kernel invariant checks
│   └── main.rs                  # Boot sequence, service spawning
├── services/                    # User-space services
├── lib/tros/                    # User-space syscall library
├── docs/
│   ├── specs/                   # Design specifications + roadmap
│   └── plans/                   # Implementation plans
├── os-ccfa-research-2024-2026/  # OS conference paper research
│   ├── outline.yaml             # 27-item research framework
│   ├── fields.yaml              # 26-field definition
│   ├── results/                 # 12 deep-researched JSON files
│   └── report.md                # 510-line research report
├── Makefile
└── Cargo.toml
```

---

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Pure Rust (`no_std`) | Memory safety at compile time, no UB from C |
| Microkernel | Minimal TCB, fault isolation, formal verification feasible |
| Capability-based security | Fine-grained access control, no ambient authority |
| RISC-V only | Clean ISA, open standard, growing ecosystem |
| Sv39 virtual memory | Standard RISC-V paging, 512GB user address space |
| AI-designed | All code and architecture co-designed with AI |

---

## Documentation

- [V21-V30 Roadmap](docs/specs/2026-05-18-trainos-v21-v30-roadmap.md)
- [Wave 1 Design Spec](docs/specs/2026-05-24-wave1-v21-v22-v23-design.md)
- [OS Research Report](os-ccfa-research-2024-2026/report.md)
- [CLAUDE.md](CLAUDE.md) — AI agent context

---

## License

MIT
