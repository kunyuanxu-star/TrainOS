# TrainOS

A microkernel operating system written in Rust for RISC-V 64-bit (rv64gc). Runs on RustSBI firmware with the machina emulator.

**Long-term goal**: Surpass Linux in kernel architecture, security, and performance.

## Architecture

TrainOS is a **microkernel** — the kernel provides exactly four mechanisms and nothing more:

| Mechanism | Responsibility |
|-----------|---------------|
| Capability System | Access control tokens for all kernel resources |
| IPC Router | Synchronous message passing between processes |
| Scheduler | 64 priority levels with bitmap O(1) picking |
| Memory Manager | Buddy allocator, Sv39 page tables, COW |

All device drivers, filesystems, network stacks, and POSIX compatibility run as **user-space services** communicating via IPC.

## Iron Rules

1. **Runtime**: RustSBI (M-mode firmware) + machina (RISC-V JIT emulator). Non-negotiable.
2. **Language**: Rust nightly (kernel `no_std`, user-space services `no_std`).
3. **Architecture**: RISC-V 64-bit (rv64gc), Sv39 virtual memory.
4. **License**: MIT.

## Current Status (2026-05-07)

### V1.1 — Bootable Microkernel with IPC

- Boot from RustSBI to S-mode with MMU enabled (Sv39)
- Buddy physical page allocator (orders 0–12, 4KB–16MB)
- Sv39 page tables with 2MB kernel superpages
- Trap entry/exit in assembly (full register save/restore)
- CLINT timer interrupts at 10ms tick
- Process/Thread model with per-process page tables
- ELF64 loader for user-space binaries
- 64-priority bitmap scheduler
- Capability system (CNode, Mint/Copy/Move/Revoke/Delete)
- IPC endpoints (create, send, recv)
- System call dispatch table
- User-space init process prints "TrainOS ready"

## Build & Run

### Prerequisites

```bash
rustup toolchain install nightly
rustup target add riscv64gc-unknown-none-elf
rustup component add rust-src llvm-tools-preview rustfmt clippy
```

### Build Kernel

```bash
cd TrainOS
cargo build --release -p kernel
```

### Run on machina

```bash
cd ..  # back to testOS/
./machina/target/release/machina \
  -M riscv64-ref \
  -bios machina/pc-bios/rustsbi-riscv64-machina-fw_dynamic.bin \
  -kernel TrainOS/target/riscv64gc-unknown-none-elf/release/kernel \
  -nographic
```

Expected output:

```
TrainOS booting...
  Memory subsystem initialized
  CLINT timer initialized
  Trap handling initialized
  Capability system initialized
  IPC subsystem initialized
  MMU enabled (Sv39)
  Init process spawned (pid=1)
  Starting scheduler...
TrainOS ready
```

## Project Structure

```
TrainOS/
├── Cargo.toml                 # Workspace root
├── rust-toolchain.toml        # nightly + riscv64gc target
├── kernel/
│   ├── Cargo.toml
│   ├── build.rs               # Linker script passthrough
│   ├── linker.ld              # Memory layout (base 0x80200000)
│   └── src/
│       ├── main.rs            # Entry point, boot sequence
│       ├── console.rs         # SBI console debug output
│       ├── mem/
│       │   ├── layout.rs      # Physical/virtual address constants
│       │   ├── buddy.rs       # Buddy allocator
│       │   ├── sv39.rs        # Sv39 page tables, MMU
│       │   └── heap.rs        # Kernel heap bump allocator
│       ├── trap/
│       │   ├── asm.rs         # Trap entry/exit assembly
│       │   └── mod.rs         # TrapFrame, dispatch, CLINT timer
│       ├── proc/
│       │   ├── process.rs     # Process struct
│       │   ├── thread.rs      # Thread, TaskContext, TrapFrame
│       │   ├── switch.rs      # context_switch + user_trap_return asm
│       │   ├── elf.rs         # ELF64 loader
│       │   └── mod.rs         # Process manager (spawn)
│       ├── sched/
│       │   └── mod.rs         # 64-priority bitmap scheduler
│       ├── cap/
│       │   ├── types.rs       # CapType, Resource, Slot, Rights
│       │   ├── ops.rs         # Mint, Copy, Move, Revoke, Delete
│       │   └── mod.rs         # Global resource table
│       ├── ipc/
│       │   ├── message.rs     # Message, CapTransfer
│       │   ├── endpoint.rs    # Endpoint, send, recv
│       │   └── mod.rs         # Endpoint table
│       └── syscall/
│           ├── mod.rs         # Syscall dispatch table
│           ├── proc.rs        # spawn, exit syscalls
│           ├── ipc.rs         # ep_create, send, recv syscalls
│           └── cap.rs         # mint, copy, move, delete syscalls
└── services/
    └── init/
        ├── Cargo.toml
        └── src/
            └── main.rs        # User-space init ("TrainOS ready")
```

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Microkernel | Minimal trusted computing base; services isolated in user space |
| Per-process PT with kernel mappings | Avoid satp switch on trap entry; kernel always accessible |
| 2MB kernel superpages | 128MB DRAM mapped via one L1 page (64 entries) |
| CLINT init before MMU | MMIO devices not mapped in kernel page table |
| Pre-MMU physical addressing | Page table setup uses PA directly before satp is set |
| `user_trap_return` via context switch | Unified path for initial sret and subsequent returns |
| Trap frame on kernel stack | Matches RISC-V calling convention; no heap allocation |

## Virtual Memory Layout

### Physical (128MB DRAM)

```
0x80000000 ┌──────────┐
           │ RustSBI  │ ~128KB
0x80020000 ├──────────┤
           │ Kernel   │ text, rodata, data, bss
           ├──────────┤
           │ Heap     │ kernel bump allocator (4MB)
           ├──────────┤
           │ Free     │ buddy-managed pages
0x88000000 └──────────┘
```

### Virtual (Sv39, per-process)

```
User space (low 256GB):
  0x0 - 0x00000040_0000     program (ELF load)
  0x00000040_0000 - ...     heap / mmap
  0x3FFFFF_FFF000           user stack (top of user space)

Kernel space (high 256GB):
  0xFFFFFFC0_0000_0000      identity-mapped DRAM (128MB)
```

## System Calls

| Number | Name | Description |
|--------|------|-------------|
| 1 | putchar | SBI console putchar (forwarded to M-mode) |
| 10 | ep_create | Create IPC endpoint |
| 11 | send | Send message to endpoint |
| 12 | recv | Receive message from endpoint (blocking) |
| 30 | mint | Derive capability with reduced rights |
| 31 | copy | Copy capability to another CNode |
| 32 | move | Move capability to another CNode |
| 33 | delete | Delete capability from slot |

## Roadmap

- [x] V1.0: Boot, MMU, buddy allocator, scheduler, user-mode init
- [x] V1.1: Capability system, IPC endpoints, syscall dispatch
- [ ] V1.2: Multi-process IPC, user-space driver service
- [ ] V1.3: File system service, POSIX compatibility service
- [ ] V1.4: Network stack service, VirtIO drivers
- [ ] V2.0: SMP multi-core, priority inheritance, COW fork

## License

MIT
