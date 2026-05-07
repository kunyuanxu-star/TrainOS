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

## Current Status (2026-05-07)

### V2.5 — SMP Microkernel with Network Stack

- **SMP multi-core**: 2 HARTs, spinlock scheduler, per-CPU data, per-HART CLINT
- **11 user-space services**: init, ping, fs, test_fs, sh, test_fork, uart, test_posix, drv, net, echo
- **Capability enforcement**: Per-process CNode with auto-stored EP caps
- **POSIX compatibility**: open/read/write/close translated to IPC→FS service
- **COW fork**: Deep-copy page tables with page fault handler
- **IPC priority inheritance**: Receiver inherits sender priority
- **Network stack**: Port-based datagram routing, service registration
- **VirtIO MMIO**: Framework verified, blocked by PMP (needs firmware update)

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
├── services/                 # 11 user-space services
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
│   └── test_net/             # Network stack test
├── lib/tros/                 # User-space syscall library
├── docs/
│   ├── specs/                # Design specifications
│   └── plans/                # Implementation plans
└── Cargo.toml                # Workspace root
```

## System Calls

| Number | Name | Description |
|--------|------|-------------|
| 0 | exit | Terminate process |
| 1 | putchar | SBI console output (forwarded) |
| 2 | getchar | SBI console input (forwarded) |
| 3 | spawn | Spawn new process from ELF |
| 4 | fork | COW fork current process |
| 5 | getpid | Get current process ID |
| 10 | ep_create | Create IPC endpoint |
| 11 | send | Send message to endpoint |
| 12 | recv | Receive message from endpoint (blocking) |
| 20 | mmio_map | Map physical MMIO into process address space |
| 30 | mint | Derive capability with reduced rights |
| 31 | copy | Copy capability to another CNode |
| 32 | move | Move capability between CNodes |
| 33 | delete | Delete capability from slot |
| 50-53 | open/read/write/close | POSIX compatibility (IPC→FS) |

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
- [ ] V3.0: Per-CPU scheduler queues, active IPI, TLB shootdown, PMP fix

## License

MIT
