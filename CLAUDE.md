# TrainOS - Claude Code Context

## Project Overview
TrainOS is an educational operating system written in Rust for RISC-V 64-bit architecture (rv64gc). Uses RustSBI as boot firmware, runs on **machina** (preferred) or QEMU.

**Goal**: Surpass Linux in kernel architecture, security, performance, and developer experience.

## Current Status (2026-04-14)

### Completed Phases

**Phase 1-8: Core Infrastructure Complete**

- Microkernel architecture with minimal kernel (scheduling, memory, IPC, traps only)
- User-space services: init, driver, fs, network, vfs, shell
- VirtIO drivers in user space (block and network)
- Sv39 virtual memory with COW fork support (MMU currently disabled)
- Preemptive scheduling via timer interrupts
- SMP multicore support
- procfs and sysfs virtual filesystems
- TCP/IP stack in user-space network service

### Kernel Shell Features (BARE Mode)
- Global system tick counter incremented by timer interrupts
- Uptime display (seconds and total ticks)
- IRQ count and rate (interrupts per second)
- MLFQ scheduler queue visualization (4 queues, pri 0-3)
- Real-time memory usage statistics (used/total/free and percentage)
- HART ID display
- Current task ID and priority display
- WFI power management

### Recent Changes (2026-04-14)
- Added uptime and IRQ rate to periodic status display
- Fixed duplicate syscall numbers (29, 96, 200, 201)
- Removed dead code and unused imports
- Applied cargo fix suggestions for code quality
- Removed debug print from syscall handler

**Phase 1-8: Core Infrastructure Complete**

- Microkernel architecture with minimal kernel (scheduling, memory, IPC, traps only)
- User-space services: init, driver, fs, network, vfs, shell
- VirtIO drivers in user space (block and network)
- Sv39 virtual memory with COW fork support
- Preemptive scheduling via timer interrupts
- SMP multicore support
- procfs and sysfs virtual filesystems
- TCP/IP stack in user-space network service

### Runtime Environment

**Primary**: machina (RISC-V full-system emulator with JIT)
- Build: `cargo build --release -p os`
- Run: `./machina/target/debug/machina -M riscv64-ref -bios machina/pc-bios/rustsbi-riscv64-machina-fw_dynamic.bin -kernel TrainOS/target/riscv64gc-unknown-none-elf/release/os -nographic`

**Secondary**: QEMU (has SATP bug, not recommended)
- Build: `cargo build --release -p os`
- Run: `qemu-system-riscv64 -machine virt -nographic -bios rustsbi-qemu-new.bin -kernel target/riscv64gc-unknown-none-elf/release/os`

### Known Issues

**PTE Encoding FIXED** (2026-04-15/16):
- **Non-leaf PTEs**: `make_nonleaf_pte()` was using wrong 3-field split format
- Per Sv39 spec, non-leaf PTEs must store PPN contiguously at bits [43:10]
- Fixed: `((ppn as u64) << 10) | 0x01`
- **Leaf PTEs**: `new_leaf()` and `make_leaf_pte()` were also using wrong 3-field split format
- For 4KB leaf PTEs, PPN must be at bits [53:10] contiguously
- Fixed both to: `((ppn as u64) << 10) | flags`
- With correct PTE encoding, page table walks now work correctly
- Test output: `L2[0] PTE=0x2000000f` (correct for PA 0x80000000)

**Machina MMU Enable Hang FIXED** (2026-04-16):
- The hang was caused by machina's JIT taking an exit_tb path when handling `csrw satp` with bit 63 set
- Fixed in machina by adding inline SATP handling in `gen_csr_read` and `gen_csr_write`
- TrainOS can now enable MMU successfully
- User mode execution still has issues (see User Mode Return Issue below)

**User Mode Return Issue** (2026-04-16, PARTIALLY FIXED):
- First `sret` now works! "READY" is printed via ecall from user mode
- The initial issue was **PTE encoding** - both leaf and non-leaf PTEs were using wrong format
- After PTE fix, user mode entry works and first ecall succeeds
- However, subsequent execution produces garbage output
- After "READY", we see `h �` (garbage) followed by trap with scause=0, sepc=0
- This suggests user code is executing but corrupting output, or second ecall fails
- Root cause of remaining issue: likely trap handler doesn't properly handle user mode traps
- When ecall from user mode, sscratch should swap with kernel sp via TSS
- Current code may not be correctly setting up trap handling for user mode returns
- **TODO**: Verify trap handler saves/restores state correctly for user mode ecalls

**Memory Display Bug FIXED** (2026-04-16):
- `free_pages()` was computing `free - base_page * 64` incorrectly
- Fixed to `free.saturating_sub(self.base_page)` - base_page is a page number, not bit index
- Memory now shows correct 99% free

**QEMU SATP Bug** (2026-04-10):
- QEMU 10.2.2 has a bug where `csrw satp` with non-zero value hangs
- This prevents MMU (Sv39) from being enabled on QEMU
- **Use machina instead** for MMU testing

**Timer Interrupt Issue FIXED** (2026-04-10):
- Root cause: Instruction order bug in `enable_timer_interrupt()` - `li t0` came AFTER `csrs sie, t0`
- Fix: Corrected instruction order to load immediate before using it
- Timer interrupts now fire correctly (WFI returns on timer tick)
- Preemptive scheduling requires MMU to switch to user mode

**Release Build Hang FIXED** (2026-04-10):
- Root cause: LLVM optimizer issue with functions using inline asm + spin::Mutex in release mode
- Fix: Added `#[inline(never)]` to `sbi_console_putchar_raw` (console.rs) and `init_page_table_allocator_with_pool` (Sv39.rs)
- Release build now boots successfully to Boot 6 like debug build

**Kernel Builtin Shell** (2026-04-14):
- When MMU is disabled, system runs a kernel builtin shell in supervisor mode
- Displays enhanced periodic status with:
  - System tick counter (real-time from global counter)
  - HART ID
  - Memory usage (used KB / total KB, % free)
  - Scheduler task counts (total, ready)
  - MLFQ queue distribution (Q0-Q3 with priorities)
  - Current running task ID and priority
- Uses WFI for power management when idle
- Timer interrupts wake the system from WFI
- Shows "--- System Status ---" every ~5 seconds

## Architecture

**Memory Layout**:
- 0x80000000: DRAM base (physical)
- 0x80080000-0x88000000: Page table pool (128 pages = 512KB)
- 0x80200000: Kernel text start
- Sv39 user space: 0x0 - 0x3FFFFFFFFFFF (128GB)

**Page Table Pool**:
- Located at PA 0x80080000 (128 pages, 512KB total)
- Pool base address was previously 0x88000000 but that address is at the RAM boundary (0x80000000 + 128MB) and is not valid RAM
- The debug print showing "root_ppn=880" was a print bug (printing truncated value), not actual truncation
- Both debug and release builds correctly allocate page tables from the pool at 0x80080000

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

1. **Fix user mode return issue** (HIGH PRIORITY):
   - `return_to_user` causes trap with scause=0, sepc=0 after sret
   - Entry point 0x11326 is 2-byte aligned (RVC compressed code)
   - User page table correctly maps entry page to PA 0x80079000
   - Yet sepc=0 suggests sret jumps to address 0 instead
   - SKIP_USER_MODE=true workaround allows system to run in kernel mode
   - Debug: verify sret behavior, check if sepc write is working
   - Alternative: try identity-mapping user VA to same PA as kernel

2. **Memory display percentage bug** (FIXED):
   - `free_pages()` was computing `free - base_page * 64` incorrectly
   - Fixed to `free.saturating_sub(self.base_page)`
   - Memory now shows correct 99% free

3. **Network virtqueue DMA** - Implement actual DMA-based frame send/receive
4. **Enhanced shell** - More commands, better usability
5. **Error handling** - Improve robustness of services
6. **Security hardening** - Full capability enforcement
7. **Namespace isolation** - Process isolation improvements
