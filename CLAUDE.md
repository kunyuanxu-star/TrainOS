# TrainOS - Claude Code Context

## Project Overview
TrainOS is an educational operating system written in Rust for RISC-V 64-bit architecture (rv64gc). Uses RustSBI as boot firmware, runs in QEMU virt machine.

**Goal**: Surpass Linux in kernel architecture, security, performance, and developer experience.

## Current Status (2026-04-10)

### Completed Phases

**Phase 1-8: Core Infrastructure Complete**

- Microkernel architecture with minimal kernel (scheduling, memory, IPC, traps only)
- User-space services: init, driver, fs, network, vfs, shell
- VirtIO drivers in user space (block and network)
- Sv39 virtual memory with COW fork support
- Preemptive scheduling via timer interrupts
- SMP multicore support
- procfs and sysfs virtual filesystems
- TCP/IP stack in user-space network service

### Known Issues

**QEMU SATP Bug** (2026-04-10):
- QEMU 10.2.2 has a bug where `csrw satp` with non-zero value hangs
- This prevents MMU (Sv39) from being enabled
- User programs cannot run without MMU
- **Workaround**: Use machina (which doesn't have this bug) for testing MMU features
- Alternative: Wait for QEMU bug fix or use older QEMU version

**Timer Interrupt Issue** (2026-04-10):
- `sie.STIE` write causes hang in both QEMU and machina
- CLINT MMIO direct access works
- Timer-based preemption is disabled until this is resolved

**Release Build Hang FIXED** (2026-04-10):
- Root cause: LLVM optimizer issue with functions using inline asm + spin::Mutex in release mode
- Fix: Added `#[inline(never)]` to `sbi_console_putchar_raw` (console.rs) and `init_page_table_allocator_with_pool` (Sv39.rs)
- Release build now boots successfully to Boot 6 like debug build

## Architecture

**Memory Layout**:
- 0x80000000: DRAM base (physical)
- 0x80000000-0x80090000: Page table pool (identity-mapped, 9MB)
- 0x80200000: Kernel text start
- Sv39 user space: 0x0 - 0x3FFFFFFFFFFF (128GB)

**Key Constants**: PAGE_SIZE=4096, MAX_TASKS=256

## Microkernel Design

### Kernel Services (in kernel space)
- **Scheduling**: MLFQ scheduler manages task execution and preemption
- **Memory Management**: Sv39 page table, COW semantics, page fault handling
- **IPC (Inter-Process Communication)**: Message passing between processes via mailbox
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
| `boot.rs` | Entry point, boot stages, trap entry asm |
| `main.rs` | Kernel initialization |
| `process/mod.rs` | Task manager, scheduler, do_schedule, start_scheduler |
| `process/task.rs` | TaskControlBlock, kernel stack allocation, user address space |
| `process/context.rs` | TrapFrame, TaskContext, context_switch, return_to_user asm |
| `process/scheduler.rs` | MLFQ scheduler implementation |
| `trap/mod.rs` | Trap handling, timer interrupts, handle_trap |
| `syscall/mod.rs` | Syscall dispatcher, all syscalls |
| `memory/Sv39.rs` | Sv39 page table, COW support, handle_cow_page |
| `memory/mod.rs` | Memory subsystem init, page table allocator |
| `drivers/interrupt.rs` | CLINT timer, PLIC interrupts |
| `elf.rs` | ELF binary loader, embedded service binaries |
| `vfs/mod.rs` | Virtual File System |
| `fs/ramfs.rs` | RAM filesystem |
| `smp/boot.rs` | SMP multicore boot |

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
| `procfs.rs` | Procfs data structures and readers |
| `sysfs.rs` | Sysfs data structures and readers |
| `shell.rs` | Shell service |

## Build & Run

```bash
# Build kernel
cargo build -p os

# Run in QEMU
cargo run -p os

# Build specific user binary
cargo build -p user --bin <name>

# Copy binary to os/bin/
cargo objcopy -p user --bin <name> -- -O binary os/bin/<name>.bin
```

## Service Startup Sequence

1. Kernel loads `init.bin` as PID 1
2. init creates IPC endpoint
3. init spawns driver (PID 2)
4. init spawns fs (PID 3)
5. init spawns network (PID 4)
6. init spawns vfs (PID 5)
7. init spawns shell (PID 6)
8. init exits
9. Shell provides user interface

## Next Steps

1. **Network virtqueue DMA** - Implement actual DMA-based frame send/receive
2. **Enhanced shell** - More commands, better usability
3. **Error handling** - Improve robustness of services
4. **Security hardening** - Full capability enforcement
5. **Namespace isolation** - Process isolation improvements
