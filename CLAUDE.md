# TrainOS - Claude Code Context

## Project Overview
TrainOS is an educational operating system written in Rust for RISC-V 64-bit architecture (rv64gc). Uses RustSBI as boot firmware, runs in QEMU virt machine.

**Goal**: Surpass Linux in kernel architecture, security, performance, and developer experience.

## Current Status (2026-04-03)

### Phase 1: Make It Runnable (In Progress)

**Completed**:
- Boot sequence runs successfully through all stages
- Fixed page table allocator issues (PT pool now at PA 0x80080000 in PMP6 RWX region)
- PageTablePool::alloc() properly tracks allocated frames with allocated_frames bitmap
- Timer initialization works with direct CLINT MMIO access
- Kernel page table with identity mappings (0x80000000-0x88000000) created during init
- Context switch in scheduler with trap frame handling
- Basic fork (sys_clone) implementation with COW semantics
- TaskControlBlock with kernel stack + trap frame allocation
- Trap handling, timer interrupts, syscall dispatch
- VFS, RAM filesystem
- Basic syscalls: read, write, getpid, sched_yield, exit, clone
- Sv39 page table infrastructure with COW support
- ELF binary loader implementation
- User program loading implemented in start_scheduler()
- RISC-V toolchain installed (xPack v12.3.0-2)
- User programs compiled (hello, shell, vi) for RISC-V
- User address space now maps kernel region (0x80000000-0x88000000) for trap handling

**Working**: Debug mode runs successfully with full boot sequence, ELF parsing and loading, return_to_user called. Trap handler correctly handles ecalls with sepc increment. User page table includes kernel mappings.

**Debug Output Confirms**:
- 'T' debug print in trap handler NEVER appears - no traps are occurring

**Critical Fix Applied**:
- ELF loader was ONLY loading first PT_LOAD segment (had `break` statement)
- Entry point 0x11d78 was in SECOND LOAD segment, not the first!
- Removed `break` statement to load ALL LOAD segments
- Now entry page is correctly mapped: va=0x11000, pa=0x8007d000, RX
- This means user program is either not running, or running without making any ecalls

**Issues**:
1. **sie CSR write hangs after trap::init()** - sie write works early in boot but hangs after trap::init(). Timer interrupts still work via sstatus.SIE.
2. **Timer interrupt via CLINT** - CLINT is programmed via direct MMIO, timer can fire but preemption requires working timer interrupt routing.
3. **User program doesn't produce output** - return_to_user() is called, but user program produces no output. QEMU times out. Investigation ongoing.

## Recent Fixes (2026-04-03)

**Page allocator find_free_bit fix (CRITICAL BUG)**:
- The original find_free_bit used `word.trailing_zeros()` which finds the first SET bit
- Changed to `(!word).trailing_zeros()` to find the first FREE (unset) bit
- This bug caused ALL pages to be allocated at the same PA (0x80071000)
- The bug was present from the start, which explains many previous issues

**ELF loader overlap fix**:
- Fixed fundamental issue with segment page mapping when p_vaddr is not page-aligned
- The overlap formula: overlap_start = max(p_vaddr, curr_vaddr), overlap_end = min(p_vaddr + p_filesz, curr_vaddr + 4096)
- Page is zeroed first, then segment data is copied to the correct offset within the page

**Trap handler sepc increment fix**:
- Added `(*trap_frame).sepc += 4` after do_syscall returns
- This ensures sret returns to the instruction AFTER the ecall, not the ecall itself
- Without this fix, ecalls would loop infinitely returning to the same ecall

**Sv39 MMU successfully enabled!**:
- Expanded identity mapping region to 0x80000000-0x80090000 (9MB)
- This covers both page tables (PT pool at 0x80080000-0x80090000) and kernel (0x80200000-0x80400000)
- Page table pool at 0x80080000 in PMP6 RWX region

**Page table pool fix**:
- PT pool moved from 0x80000000 (PMP3 read-only) to 0x80080000 (PMP6 RWX)
- Added allocated_frames bitmap to PageTablePool to properly track allocations

### User Program Debugging Status (2026-04-03)

**Symptoms**:
- System boots successfully through all stages
- ELF loading reports entry=0x11d78, sp=0x7ffffffdf0
- return_to_user() is called
- QEMU times out with no user program output
- Trap handler debug 'T' NEVER appears - no traps are occurring at all

**Analysis**:
- Page allocator bug fixed (was returning same PA for all allocations)
- ELF loader now loads ALL LOAD segments correctly
- Trap handler correctly increments sepc after syscalls
- User address space now includes kernel region mapping (0x80000000-0x88000000)
- Entry point (0x11d78) is correctly within the LOAD segment [0x1041a, 0x21e1c)
- Code at entry point looks like valid RISC-V instructions
- 'T' debug print in trap handler would appear on ANY trap (ecall, timer, etc.)
- Since 'T' never appears, NO traps are occurring
- This strongly suggests the user program is NOT reaching any ecall instruction

**HELLO Binary Analysis**:
- File size: 11382 bytes (debug build)
- Entry point: 0x11d78 (within LOAD segment)
- LOAD segment: p_vaddr=0x1041a, p_filesz=0x11a04
- Entry point file offset: 0x40a + (0x11d78 - 0x1041a) = 0x1d78
- Code at 0x1d78: 13 01 01 cc ... (valid instruction bytes)

**Next Debugging Steps**:
1. Verify user code at entry point is actually valid and being executed
2. Add debug output BEFORE and AFTER return_to_user to confirm execution
3. Check if user program makes any ecalls at all
4. Try simpler test program that does single ecall immediately

### Sie Write Issue Details

**Finding**: sie CSR write hangs after trap::init() is called.

**What trap::init() does**:
1. Sets stvec to trap handler entry point
2. Sets sstatus.SIE bit (enables supervisor interrupts)
3. Calls clint_init() which sets CLINT mtimecmp via direct MMIO

**Root cause**: Unknown. Possible that clint_init() or sstatus write has side effects.

## Build & Run

```bash
# Debug mode (WORKS)
cargo build -p os
cargo run -p os

# Release mode (has hang issue - investigate later)
cargo build --release -p os
cargo run --release -p os
```

## Architecture

**Memory Layout**:
- 0x80000000: DRAM base (physical)
- 0x80000000-0x80090000: Page table pool (identity-mapped, 9MB)
- 0x80200000: Kernel text start
- Sv39 user space: 0x0 - 0x3FFFFFFFFFFF (128GB)

**Key Constants**: PAGE_SIZE=4096, MAX_TASKS=256

## Key Files

| File | Purpose |
|------|---------|
| `os/src/boot.rs` | Entry point, boot stages, trap entry asm |
| `os/src/process/mod.rs` | Task manager, scheduler, do_schedule, start_scheduler |
| `os/src/process/task.rs` | TaskControlBlock, kernel stack allocation, user address space |
| `os/src/process/context.rs` | TrapFrame, TaskContext, context_switch, return_to_user asm |
| `os/src/process/scheduler.rs` | MLFQ scheduler implementation |
| `os/src/trap/mod.rs` | Trap handling, timer interrupts, handle_trap |
| `os/src/syscall/mod.rs` | Syscall dispatcher, sys_clone, do_syscall |
| `os/src/memory/Sv39.rs` | Sv39 page table, COW support, handle_cow_page |
| `os/src/memory/mod.rs` | Memory subsystem init, page table allocator |
| `os/src/drivers/interrupt.rs` | CLINT timer (direct MMIO), PLIC interrupts |
| `os/src/vfs/mod.rs` | Virtual File System |
| `os/src/fs/ramfs.rs` | RAM filesystem |
| `os/src/elf.rs` | ELF binary loader |

## RISC-V Toolchain (INSTALLED)

**xPack RISC-V Embedded GCC v12.3.0-2** installed.

**User programs** (built for riscv64gc-unknown-none-elf):
- Built via `cargo build --target riscv64gc-unknown-none-elf --release -p user`

## Next Steps (Priority Order)

1. **Debug user program execution** - return_to_user is called but user program produces no output. Need to verify ELF entry point is correct and code is mapped properly.
2. **Debug sie CSR write hang** - sie write works early in boot but hangs after trap::init().
3. **Timer interrupt in QEMU** - Enable timer interrupts via sie.STIE or workaround.
4. **Verify user mode syscalls** - Once user program runs, test ecall/syscall functionality.
5. **Fix release mode** - spin::Mutex optimization issue.

## Timer Interrupt Issue

**Status**: Timer cannot be enabled via sie.STIE due to sie write hang.

**Current workaround**: Direct CLINT MMIO access for timer, but interrupts still not enabled because sie.STIE cannot be set.

## Microkernel Architecture

TrainOS implements a **microkernel design** where the kernel only provides minimal core services:

### Kernel Services (in kernel space)
- **Scheduling**: MLFQ scheduler manages task execution and preemption
- **Memory Management**: Sv39 page table, COW semantics, page fault handling
- **IPC (Inter-Process Communication)**: Message passing between processes via mailbox
- **Trap Handling**: Exception and interrupt dispatch, syscalls

### User-Space Services (run as normal processes)
- **Drivers**: Device drivers (display, keyboard, network, etc.)
- **Filesystem**: VFS with pluggable backends (RAM fs, eventually ext2, network fs)
- **Init Service**: First user process (PID 1), responsible for system initialization
- **Shell**: Command-line interface
- **Applications**: User applications

### IPC System Calls (1000-1004)
| Syscall | Name | Description |
|---------|------|-------------|
| 1000 | ipc_register | Register a port for IPC |
| 1001 | ipc_connect | Connect to a named port |
| 1002 | ipc_send | Send a message to a port |
| 1003 | ipc_recv | Receive a message (blocking) |
| 1004 | ipc_call | Send and wait for response |

### Process Structure
Each process has:
- **TaskControlBlock**: Kernel state (registers, stack pointers, page table)
- **Mailbox**: Queue of pending IPC messages
- **PID**: Process identifier (PID 0 = idle, PID 1 = init)
- **Status**: Ready, Running, Blocked, Exited

### IPC Flow
1. Process A calls `ipc_send(dest_port, message)` via syscall
2. Kernel delivers message to Process B's mailbox
3. If Process B is blocked on `ipc_recv()`, it becomes Ready
4. Scheduler picks Process B to run
5. Process B calls `ipc_recv()` to retrieve message from mailbox
