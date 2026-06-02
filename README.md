# TrainOS

[English](README.md) | [中文](README_zh.md)

A microkernel operating system written in Rust for RISC-V 64-bit (rv64gc). Runs on RustSBI firmware on QEMU.

**Goal**: Surpass Linux in kernel architecture, security, and performance — fully designed and implemented with AI.

**Current Version**: V38.0 | **Syscalls**: 362+ | **Kernel**: ~20,000 LOC | **License**: MIT

---

## Architecture

TrainOS is a **microkernel** — kernel mechanisms are minimal, everything else runs in user space:

| Subsystem | Capability |
|-----------|-----------|
| **Capability System** | CNode with Mint/Copy/Move/Revoke/Delete, parent-rights enforcement, audit logging |
| **IPC** | Synchronous message passing, priority inheritance, distributed IPC across nodes |
| **Scheduler** | NUMA-aware, EEVDF deadline-based, 64-priority per-node queues, SMP |
| **Memory Manager** | Buddy allocator, Sv39/Sv48 page tables, transactional MMU, COW fork, W^X enforcement |
| **Security** | seccomp filter, CHERI software capabilities, ASLR/KASLR, PMP/ePMP TEE enclaves |
| **Hypervisor** | RISC-V H-extension, two-stage address translation, VM lifecycle, VirtIO backend |

---

## Feature Matrix

### Kernel Services

| Category | Features |
|----------|----------|
| **Process** | spawn, fork(COW), exec, exit, kill, waitpid, signal, prctl, priority |
| **Memory** | mmap, munmap, mprotect, brk, shm_map, madvise, mincore, mlock, mseal, mTHP |
| **Filesystem** | open, read, write, close, stat, lseek, dup, getcwd, symlink, readlink, fsync, flock, fallocate, sendfile, ioctl(termios) |
| **Socket** | socket, bind, listen, accept, connect, sendto, recvfrom, getsockopt, setsockopt, shutdown |
| **IPC** | ep_create, send, recv, call, reply + System V semaphores, message queues |
| **Time** | nanosleep, clock_gettime, gettimeofday, settimeofday, POSIX timers |
| **Poll** | poll, ppoll, pselect6, epoll_create/ctl/wait |
| **I/O** | io_uring with zero-copy, RWF_UNCACHED/ATOMIC, cachestat, per-CPU blk-mq |

### Advanced Subsystems

| Subsystem | Description |
|-----------|-------------|
| **io_uring** | Async I/O with per-process SQ/CQ rings, shared memory mapping, zero-copy splice |
| **eBPF Extensions** | Sandboxed bytecode verifier, 12-opcode interpreter, 4 hook types, eBPF+WASM hybrid |
| **WASM Runtime** | 36-opcode interpreter, WASI preview2 (21 host funcs), syscall-as-host-function (55 mappings) |
| **NUMA** | Per-node ready queues, EEVDF, load balancing, per-CPU counters, MCS lock, RCU |
| **Distributed IPC** | Node discovery, remote messaging, distributed capability passing, remote memory pooling |
| **GPU/AI** | GPU driver, AI workload scheduler (MPS), tensor ops, model registry, P/D separation, KV-cache |
| **Virtualization** | RISC-V H-extension, G-stage MMU, VM lifecycle, VirtIO backend, PV timer, VS-AIA, snapshot |
| **TEE** | AP-TEE compliant enclaves, RATS remote attestation, multi-zone isolation, secure storage |
| **GUI** | Framebuffer driver, window manager (32 windows), widget toolkit, GUI service (EP 9) |

### RISC-V ISA Extensions

| Category | Extensions |
|----------|-----------|
| **Vector & AI** | RVV 1.0 (lazy context switch, vector memcpy) |
| **Interrupt & Timer** | AIA (APLIC+IMSIC), Sstc (direct timer), VS-AIA (virtualized) |
| **Memory & Paging** | Sv48/Sv57, Svnapot (64KB pages), Svpbmt (memory types), Svinval (TLB), Sspmp (S-mode PMP) |
| **Cache & Crypto** | Zicbom/Zicboz (cache ops), Zkr (entropy), Zkne (AES), Zknh (SHA), Zks (SM3/SM4) |
| **Security & IOMMU** | ePMP (enhanced PMP), RISC-V IOMMU, Pointer Masking (Ssnpm) |
| **Performance** | Sscofpmf (PMU with 29 events), Sdtrig (hardware debug), Smstateen |
| **Optimization** | B Extension (Zbb/Zbs/Zbkb), Zicond (conditional move), Zihintpause |

### Security Hardening

| Mechanism | Implementation |
|-----------|---------------|
| **W^X** | Page table enforcement, auto-fix on violation |
| **ASLR/KASLR** | PCG-based stack/mmap/PIE randomization, kernel slide (>30 bits) |
| **Stack/Heap Canary** | Guard page + magic value verification |
| **seccomp/CHERI/Sandbox** | Per-process syscall filter + 128-bit fat pointer + path/network/UID sandbox |
| **TEE Attestation** | SHA-512 measurement + Ed25519 signature + RATS challenge-response |

### Production Readiness

| Area | Features |
|------|----------|
| **Linux ABI** | 120+ syscall mappings, flag/errno translation, dynamic linker (RISC-V RELA) |
| **/proc + /sys** | cpuinfo, meminfo, mounts, stat, loadavg, per-process maps/status/cmdline/fd |
| **Service Manager** | Dependency-based boot, auto-restart, DHCP/DNS/package manager |
| **Hardware** | QEMU virt, SiFive HiFive, StarFive VisionFive 2, Canaan K230 |

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
V1-V12   V13-V20    V21-V30      V31-V34    V35-V38
███████  █████████  ████████████  █████████  █████████
 基础     功能完善    路线图驱动     调研驱动    Linux追平
  内核    81+ sc     10版本        4版本       +RISC-V
  IPC     35+服务    15,050行      5,700行     20+ ISA扩
```

| Phase | Versions | Approach | Key Outcome |
|-------|----------|----------|-------------|
| **Foundation** | V1-V12 | Incremental | Boot, MMU, scheduler, IPC, FS, network, VirtIO |
| **Feature** | V13-V20 | Feature-driven | TCP, VFS, namespaces, 35+ services, POSIX |
| **Roadmap** | V21-V30 | 10 versions / 4 waves | Formal verification, io_uring, virt, eBPF, NUMA, dist-IPC, CHERI/ASLR, WASM, GPU/AI, Linux ABI |
| **Research** | V31-V34 | CCF-A paper-driven | TxMMU (CortenMM SOSP'25), WASM hybrid (WABI), TEE (TEEM³), P/D scheduler (OSDI'24) |
| **Linux Parity** | V35-V38 | Linux + RISC-V survey | PREEMPT_LAZY, Proxy Exec, mseal, mTHP, RWF_ATOMIC, RVV 1.0, AIA, Sv48, crypto, PMU, GUI |

---

## Project Structure

```
TrainOS/
├── kernel/src/                  # Kernel (~20,000 LOC)
│   ├── cap/                     # Capability system
│   ├── ipc/                     # IPC endpoints & messages
│   ├── syscall/                 # Syscall dispatch (362+)
│   ├── mem/                     # Buddy, Sv39/Sv48, TxMMU, mTHP, cache ops
│   ├── trap/                    # Trap dispatch, PMU, debug, Sstc, AIA
│   ├── proc/                    # Process, thread, ELF loader
│   ├── sched/                   # EEVDF scheduler, NUMA-aware
│   ├── security/                # W^X, seccomp, cap audit, TEE, attestation
│   ├── crypto/                  # AES, SHA-2/3, SM3/SM4 (Zk extensions)
│   ├── iouring/                 # io_uring async I/O
│   ├── extension/               # eBPF-like kernel extensions
│   ├── hypervisor/              # H-extension, VM, VirtIO, VS-AIA
│   ├── numa/                    # NUMA, per-CPU counters, MCS, RCU
│   ├── distributed/             # Distributed IPC
│   ├── wasm/                    # WASM interpreter, WASI, hostcall
│   ├── ai/                      # GPU driver, AI scheduler, tensor ops
│   ├── compat/                  # Linux ABI, /proc, /sys, dynamic linker
│   ├── device/                  # Framebuffer, window mgr, widgets, blk-mq
│   └── main.rs                  # Boot sequence
├── services/                    # User-space services
├── lib/tros/                    # User-space syscall library
├── docs/                        # Specs, plans, roadmap
├── os-ccfa-research-2024-2026/  # CCF-A paper research (12 deep-researched)
├── .ide/Dockerfile              # CNB cloud-native dev environment
├── .cnb.yml                     # CNB cloud IDE workspace config
└── Makefile
```

---

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Pure Rust (`no_std`) | Memory safety at compile time |
| Microkernel | Minimal TCB, fault isolation |
| Capability-based security | Fine-grained access control |
| RISC-V only | Clean open ISA, growing ecosystem |
| Sv39 + Sv48 virtual memory | 512GB → 256TB address space |
| AI-designed | All code and architecture co-designed with AI |
| Dual platform | GitHub + CNB cloud-native development |

---

## Documentation

- [V21-V30 Roadmap](docs/specs/2026-05-18-trainos-v21-v30-roadmap.md)
- [OS CCF-A Research Report](os-ccfa-research-2024-2026/report.md)
- [CLAUDE.md](CLAUDE.md) — AI agent context

---

## License

MIT
