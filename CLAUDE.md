# TrainOS - Claude Code Context

## Project Overview
TrainOS is an educational operating system written in Rust for RISC-V 64-bit architecture (rv64gc). Uses RustSBI as boot firmware, runs on **machina** (preferred) or QEMU.

**Goal**: Surpass Linux in kernel architecture, security, performance, and developer experience.

## Current Status (2026-04-28)

### Build & Run Mode
- **MMU enabled** on machina: Sv39 page table active, kernel identity-mapped 128MB DRAM
- **User mode attempted**: sret works, trap mechanism works (timer interrupts fire)
- **CRITICAL BUG**: User program stuck at entry point — sepc never advances from entry PC
- **SMP**: Disabled for debugging (causes watchdog timeout)
- `QEMU_SKIP_MMU=false` in `Sv39.rs`, `SKIP_USER_MODE=false` in `process/mod.rs`

### Completed Phases 1-8: Core Infrastructure
- Microkernel architecture: scheduling, memory, IPC, traps in kernel space
- User-space services: init, driver, fs, network, vfs, shell
- VirtIO drivers in user space (block and network)
- Sv39 virtual memory with COW fork support (MMU currently disabled)
- Preemptive scheduling via timer interrupts
- SMP multicore support (disabled for debugging)
- procfs and sysfs virtual filesystems
- TCP/IP stack in user-space network service

### Kernel Shell Features (BARE Mode)
- Global system tick counter, uptime, IRQ count/rate
- MLFQ scheduler queue visualization (4 queues, pri 0-3)
- Real-time memory usage statistics
- HART ID, current task ID/priority display
- WFI power management

### Recent Changes (2026-04-28)

**Trap Entry/Exit Rewrite + MMU Enable** (commits `1df658a`, `0b021bf`):
- Rewrote `__trap_entry` to use `csrrw sp, sscratch, sp` for atomic sp/sscratch swap
- Properly handles both kernel-mode and user-mode trap entry/return
- `return_to_user_with_mmu` simplified with named register inline asm
- Full 128MB DRAM identity mapped via 64 L1 entries × 512 L2 entries
- Added `sfence.vma` + `fence.i` after `csrw satp` for TLB coherence
- Fixed syscall argument reading: get_arg functions now read from trap frame offsets
- Added syscall 1 (SBI-compatible console putchar) for legacy user programs
- Built machina emulator for MMU testing
- **MMU enabled and sret to user mode works**
- **Trap mechanism verified**: timer interrupts fire, trap handler runs, returns correctly
- **CRITICAL**: User program stuck at entry point — sepc=0x11158 on every trap
  - Root cause: machina emulator has TWO bugs:
    1. 32-bit instructions don't execute at low VAs (< 0x80000000)
    2. First instruction after sret never executes (even at kernel VAs,
       unless it's a jump-to-self)
  - Workaround: ELF loader relocates user programs to kernel VAs (0x90000000+)
  - Status: S-mode execution with kernel VA relocation works (timer interrupts
    confirm CPU executes instructions). But syscalls (ebreak/ecall) don't
    produce visible output yet — needs further investigation.

### Previous Fixes (2026-04-10 to 2026-04-17)
- PTE Encoding: Non-leaf and leaf PTEs use contiguous PPN at bits [43:10] / [53:10]
- Machina MMU: Fixed JIT handling of `csrw satp` with bit 63 set
- Memory Display: Fixed free_pages subtraction overflow
- Timer Interrupt: Fixed instruction order in `enable_timer_interrupt()`
- Release Build: Added `#[inline(never)]` for functions with inline asm + Mutex
- ELF Corruption: Rebuilt os/bin/init.bin from correct ELF binary

## Runtime Environment

**Primary**: machina (RISC-V full-system emulator with JIT)
- Build: `cargo build --release -p os`
- Run: `./machina/target/debug/machina -M riscv64-ref -bios machina/pc-bios/rustsbi-riscv64-machina-fw_dynamic.bin -kernel TrainOS/target/riscv64gc-unknown-none-elf/release/os -nographic`
- Status: Not built in current environment

**Secondary**: QEMU 11.0.0 (has SATP bug, MMU cannot be enabled)
- Build: `cargo build --release -p os`
- Run: `qemu-system-riscv64 -machine virt -nographic -bios rustsbi-qemu-new.bin -kernel target/riscv64gc-unknown-none-elf/release/os`
- Runs in BARE mode with kernel builtin shell only

## Architecture

**Memory Layout**:
- 0x80000000: DRAM base (physical, 128MB)
- 0x80080000-0x88000000: Page table pool (128 pages = 512KB)
- 0x80200000: Kernel text start
- Sv39 user space: 0x0 - 0x3FFFFFFFFFFF (128GB)

**Key Constants**: PAGE_SIZE=4096, MAX_TASKS=256

## Microkernel Design

### Kernel Services (in kernel space)
- **Scheduling**: MLFQ scheduler manages task execution and preemption
- **Memory Management**: Sv39 page table, COW semantics, page fault handling
- **IPC**: Message passing between processes via mailbox
- **Trap Handling**: Exception and interrupt dispatch, syscalls

### User-Space Services
| Service | PID | Binary | Purpose |
|---------|-----|--------|---------|
| init | 1 | init.bin | System initialization, spawns all services |
| driver | 2 | driver.bin | VirtIO block and net device access |
| fs | 3 | fs.bin | Filesystem operations (VFS + RAM fs) |
| network | 4 | network.bin | TCP/IP protocol stack |
| vfs | 5 | vfs.bin | procfs and sysfs virtual filesystems |
| shell | 6 | shell.bin | Command-line interface |

### IPC System Calls (1000-1004)
| Syscall | Name | Description |
|---------|------|-------------|
| 1000 | endpoint_create | Create IPC endpoint |
| 1001 | endpoint_delete | Delete IPC endpoint |
| 1002 | send | Send message to process/port |
| 1003 | recv | Receive message (blocking) |
| 1004 | call | Synchronous RPC call |

### Service Spawning (syscall 1105)
| Service ID | Service | Description |
|------------|---------|-------------|
| 0 | driver | VirtIO driver service |
| 1 | fs | Filesystem service |
| 2 | shell | Shell service |
| 3 | network | Network TCP/IP stack |
| 4 | vfs | procfs/sysfs |

## Key Files

### Kernel (`os/src/`)
| File | Purpose |
|------|---------|
| `boot.rs` | Entry point, boot stages, trap entry/exit asm (rewritten 2026-04-28) |
| `main.rs` | Kernel initialization |
| `process/mod.rs` | Task manager, scheduler, start_scheduler, kernel shell |
| `process/task.rs` | TaskControlBlock, kernel stack allocation, user address space |
| `process/context.rs` | TrapFrame, TaskContext, context_switch, return_to_user (simplified) |
| `process/scheduler.rs` | MLFQ scheduler implementation |
| `trap/mod.rs` | Trap handling, timer interrupts, handle_trap |
| `syscall/mod.rs` | Syscall dispatcher, all syscalls |
| `memory/Sv39.rs` | Sv39 page table (full 128MB identity map), COW, enable_sv39 |
| `memory/mod.rs` | Memory subsystem init, page table allocator |
| `drivers/interrupt.rs` | CLINT timer, PLIC interrupts |
| `elf.rs` | ELF binary loader, embedded service binaries |

### User Space (`user/src/`)
| File | Purpose |
|------|---------|
| `init.rs` | Init service - spawns all other services |
| `driver.rs` | Driver service - VirtIO block/net MMIO |
| `driver_blk.rs` | VirtIO block device driver |
| `driver_net.rs` | VirtIO network device driver |
| `driver_mmio.rs` | MMIO access syscalls |
| `fs.rs` | FS service - filesystem operations |
| `network.rs` | Network service - TCP/IP stack |
| `net/*.rs` | Protocol implementations (eth, ipv4, tcp, udp, arp, dns) |
| `vfs_service.rs` | VFS service - procfs/sysfs handlers |
| `shell.rs` | Shell service |

## Build & Run

```bash
# Build kernel
cargo build -p os

# Build release
cargo build --release -p os

# Build all user binaries
cargo build --release -p user

# Copy binary to os/bin/
cargo objcopy -p user --bin <name> -- -O binary os/bin/<name>.bin

# Run in QEMU (BARE mode only - no MMU)
cargo run -p os
```

## Current Config Flags

| Flag | File | Value | Effect |
|------|------|-------|--------|
| `QEMU_SKIP_MMU` | `memory/Sv39.rs` | `true` | Skip MMU enable (safe for QEMU) |
| `SKIP_USER_MODE` | `process/mod.rs` | `true` | Run kernel builtin shell instead of user mode |

To enable user mode (requires machina): set `QEMU_SKIP_MMU=false` and `SKIP_USER_MODE=false`.

## Next Steps (Priority Order)

1. **Fix post-sret instruction execution** — First instruction after sret never executes
   (even at kernel VAs). Debug machina's sret implementation; consider mret-based workaround.
2. **Enable syscall output** — Verify ebreak/ecall syscalls produce user-visible output
3. **Service startup chain** — Test init → driver → fs → network → vfs → shell
4. **Enable SMP** — Debug watchdog timeout, bring up multi-core support
5. **Network virtqueue DMA** — Implement actual DMA-based frame send/receive
6. **Security hardening** — Full capability enforcement, namespace isolation
