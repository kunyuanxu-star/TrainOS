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
- Kernel page table with identity mappings (0x80000000-0x80090000) created during init
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

**Working**: Debug mode runs successfully with full boot sequence, ELF parsing and loading, return_to_user called. Trap handler correctly handles ecalls with sepc increment.

**Issues**:
1. **sie CSR write hangs after trap::init()** - sie write works early in boot but hangs after trap::init(). Timer interrupts still work via sstatus.SIE.
2. **Timer interrupt via CLINT** - CLINT is programmed via direct MMIO, timer can fire but preemption requires working timer interrupt routing.
3. **User program doesn't produce output** - return_to_user() is called, but user program produces no output. QEMU times out. Investigation ongoing.

## Recent Fixes (2026-04-03)

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
- ELF loading reports entry=0x117d4, sp=0x7ffffffdf0
- return_to_user() is called (confirmed by debug output)
- QEMU times out with no user program output

**Analysis**:
- ELF loader correctly maps segment pages with proper overlap calculation
- Trap handler increments sepc after syscalls
- The issue might be with ELF entry point calculation or segment mapping

**HELLO Binary Analysis**:
- File size: 8600 bytes
- Entry point: 0x117d4
- LOAD2 segment: p_vaddr=0x1130c, p_filesz=0x892, p_offset=0x30c
- Entry point 0x117d4 falls within LOAD2 range [0x1130c, 0x11B9E)
- But byte at file offset 0x7D4 (corresponding to entry) appears to be 0x5171, which may not be valid code

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
