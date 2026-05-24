# TrainOS

A microkernel operating system written in Rust for RISC-V 64-bit (rv64gc). Runs on RustSBI firmware with the machina emulator.

**Goal**: Surpass Linux in kernel architecture, security, and performance.

## Architecture

TrainOS is a **microkernel** — the kernel provides four mechanisms:

| Mechanism | Responsibility |
|-----------|---------------|
| Capability System | Access control tokens (CNode, Mint/Copy/Move/Revoke/Delete) |
| IPC Router | Synchronous message passing with priority inheritance |
| Scheduler | 64 priority levels, bitmap O(1), SMP-aware spinlock |
| Memory Manager | Buddy allocator, Sv39 page tables, COW fork |

All device drivers, filesystems, network stacks, and POSIX compatibility run as **user-space services**.

## Iron Rules

1. **Runtime**: RustSBI (M-mode firmware) + machina (RISC-V JIT emulator).
2. **Language**: Rust nightly (`no_std` kernel, `no_std` user-space).
3. **Architecture**: RISC-V 64-bit (rv64gc), Sv39 virtual memory.
4. **License**: MIT.

## Development Roadmap

See [V21-V30 Roadmap](docs/specs/2026-05-18-trainos-v21-v30-roadmap.md) — a 10-version plan to surpass Linux based on top conference research and open-source evolution.

## Current Status (2026-05-18)

### V20.0 — Applications & Foundation: BusyBox, Persistent VFS, Shell Pipelines

- **40+ system calls**: Process, IPC, capability, MMIO, block I/O, POSIX (stat/lseek/dup/getcwd), signal, waitpid, shm_map
- **36+ user-space services**: init, ping, fs(VFS), test_fs, sh, test_fork, uart, test_posix, drv, net, tcp(NEW), echo, test_net, test_c, proc, test_proc, demo, stress, bb, pci, veth, tfs, tfs_jrnl, edit, cat, bench, rustdemo, mkfs, http, test_smp, test_posix2, test_mount, test_http, reg, test_sdp, pkg
- **TCP reliable stream protocol**: 3-way handshake, sequence numbers, ACKs, connection teardown
- **VFS with procfs**: Directory tree, 16-file slots, virtual /proc files (uptime, meminfo, perf, version, proc)
- **Process crash isolation**: Kernel kills offending process instead of hanging on unhandled traps
- **Kernel print macros**: `println!()` / `print!()` using `core::fmt::Write` engine
- **Refactored main.rs**: ~260 lines (down from 1482), `spawn_service!` macro
- **SMP verified**: 2 HARTs with concurrent IPC, fork under SMP
- **Capability system**: Full CNode with Mint/Copy/Move/Revoke/Delete
- **POSIX compatibility**: open/read/write/close/stat/lseek/dup/getcwd
- **COW fork + page fault handler**: Full deep-copy with COW breaking
- **IPC priority inheritance**: Receiver inherits sender priority
- **Network stack**: Port-based datagram routing (UDP) + TCP reliable streams
- **VirtIO block I/O**: Full sector read/write via kernel proxy
- **File system**: TFS journaling file system, persistent storage
- **PCI/MMIO device support**: PCI enumeration, VirtIO transport, network driver
- **System utilities**: Shell, process manager, demo, edit, cat, block I/O stress/bench
- **Multi-user support**: UID/GID, chmod, setuid/getuid

## Build & Run

```bash
# Prerequisites
rustup toolchain install nightly
rustup target add riscv64gc-unknown-none-elf
rustup component add rust-src llvm-tools-preview rustfmt clippy

# Build all services and kernel
cd TrainOS
cargo build --release -p init -p ping -p fs -p test_fs -p sh \
  -p test_fork -p uart -p test_posix -p drv -p net -p echo -p test_net
cp target/riscv64gc-unknown-none-elf/release/* kernel/src/
cargo build --release -p kernel

# Run on machina (2 CPUs)
cd .. && ./machina/target/release/machina \
  -M riscv64-ref -smp 2 \
  -bios machina/pc-bios/rustsbi-riscv64-machina-fw_dynamic.bin \
  -kernel TrainOS/target/riscv64gc-unknown-none-elf/release/kernel \
  -nographic
```

## Project Structure

```
TrainOS/
├── kernel/src/               # ~30 files, ~3500 lines
│   ├── cap/                  # Capability system (types, ops)
│   ├── ipc/                  # IPC endpoints, messages
│   ├── syscall/              # Syscall dispatch + POSIX module
│   ├── mem/                  # Buddy, Sv39, heap
│   ├── trap/                 # Trap asm, dispatch, page fault
│   ├── proc/                 # Process, thread, switch, ELF
│   ├── sched/                # 64-prio SMP scheduler
│   ├── sync.rs               # SpinLock primitive
│   ├── per_cpu.rs            # Per-CPU data structures
│   └── main.rs               # Boot sequence
├── services/                 # 24 user-space services
│   ├── init/ (pid=1)         # IPC receiver
│   ├── ping/ (pid=2)         # IPC sender demo
│   ├── fs/   (pid=3)         # File system service
│   ├── test_fs/              # FS RPC test
│   ├── sh/                   # Interactive shell
│   ├── test_fork/            # COW fork test
│   ├── uart/                 # UART MMIO demo
│   ├── test_posix/           # POSIX API test
│   ├── drv/                  # VirtIO MMIO driver
│   ├── net/                  # Network stack (port routing)
│   ├── echo/                 # Echo service (port 7)
│   ├── demo/                 # System demo (V8.0 banner, IPC/FS/MEM/CAP/PERF)
│   ├── stress/               # Block I/O stress test
│   ├── bb/                   # Block I/O benchmark
│   ├── pci/                  # PCI device enumeration
│   ├── veth/                 # Virtual Ethernet driver
│   ├── tfs/                  # TFS file system service
│   ├── tfs_jrnl/             # TFS journaling layer
│   ├── edit/                 # Text editor
│   ├── cat/                  # File viewer
│   └── test_net/             # Network stack test
├── lib/tros/                 # User-space syscall library
├── docs/
│   ├── specs/                # Design specifications
│   └── plans/                # Implementation plans
└── Cargo.toml                # Workspace root
```

## System Calls (35+)

| Number | Name | Description |
|--------|------|-------------|
| 0 | exit | Terminate process |
| 1 | putchar | SBI console output (forwarded) |
| 2 | getchar | SBI console input (forwarded) |
| 3 | spawn | Spawn new process from ELF |
| 4 | fork | COW fork current process |
| 5 | getpid | Get current process ID |
| 6 | yield | Yield CPU (stays ready) |
| 10 | ep_create | Create IPC endpoint |
| 11 | send | Send message to endpoint |
| 12 | recv | Receive message from endpoint (blocking) |
| 20 | mmio_map | Map physical MMIO into process address space |
| 21 | mmio_read32 | Read 32-bit MMIO (kernel proxy) |
| 22 | mmio_write32 | Write 32-bit MMIO (kernel proxy) |
| 30 | mint | Derive capability with reduced rights |
| 31 | copy | Copy capability to another CNode |
| 32 | move | Move capability between CNodes |
| 33 | delete | Delete capability from slot |
| 34 | cap_stats | Query capability counts |
| 40 | proclist | List processes |
| 41 | kill | Kill process by PID |
| 42 | meminfo | Query allocated page count |
| 43 | perf_stats | IPC/context-switch counters |
| 44 | uptime | System uptime in ticks |
| 45 | blk_write | Write block via VirtIO |
| 46 | blk_read | Read block via VirtIO |
| 50 | open | POSIX open (IPC→FS) |
| 51 | read | POSIX read (IPC→FS) |
| 52 | write | POSIX write (IPC→FS) |
| 53 | close | POSIX close (IPC→FS) |

Full reference: [docs/syscalls.md](docs/syscalls.md)

## Documentation
- [Architecture Guide](docs/architecture.md)
- [Service Development Tutorial](docs/service-dev.md)  
- [System Call Reference](docs/syscalls.md)
- [Design Specifications](docs/specs/)

## Demo Output

```
========================================
  TrainOS V8.0 System Demo
========================================

[1/5] IPC: ping -> init ... OK
[2/5] FS: write -> read ... OK
[3/5] MEM: allocated pages = 1234 OK
[4/5] CAP: capability system ... OK
[5/5] PERF: IPC counters ... OK

========================================
  All systems operational
  TrainOS V8.0 — READY
========================================
```

## IPC Protocol

**Well-Known Endpoints**:
| EP | Service | Purpose |
|----|---------|---------|
| 1 | init | System init, IPC receiver |
| 2 | fs | File system service |
| 3 | net | Network stack (port routing) |

**FS Operations** (via EP 2):
| Opcode | Name | Format |
|--------|------|--------|
| 2 | READ | [reply_ep] → response payload |
| 3 | WRITE | [len, data..., reply_ep] → "OK" |

**NET Operations** (via EP 3):
| Opcode | Name | Format |
|--------|------|--------|
| 1 | REGISTER | [port(2), listener_ep(2)] |
| 2 | SEND | [port(2), len(1), data...] → routed to listener |

## Roadmap

- [x] V1.0-V1.1: Boot, MMU, buddy, scheduler, init, cap + IPC + syscalls
- [x] V1.2-V1.4: IPC demo, FS service, Shell
- [x] V1.5-V2.0: MMIO, COW fork, priority inheritance
- [x] V2.1-V2.5: SMP, Cap enforcement, POSIX, VirtIO, Network
- [x] V3.0-V3.2: Active IPI, C program support, VirtIO block, Proc service
- [x] V4.0-V6.0: PCI, VETH networking, TFS journaling file system
- [x] V7.0-V8.0: Demo suite, block stress, text utilities, full doc
- [x] V9.0-V12.0: Package manager, HTTP server, multi-user, shared memory, signals
- [x] V13.0: TCP protocol, VFS+procfs, process crash isolation, kernel print macros
- [ ] V14.0: Epoll/kqueue async I/O, socket API, container namespaces

## License

MIT
