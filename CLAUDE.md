# TrainOS - Claude Code Context

## Project Overview
TrainOS is an educational operating system written in Rust for RISC-V 64-bit architecture (rv64gc). Uses RustSBI as boot firmware, runs in QEMU virt machine.

**Goal**: Surpass Linux in kernel architecture, security, performance, and developer experience.

## Current Status (2026-03-30)

### Phase 1: Make It Runnable (In Progress)

**Completed**:
- Timer interrupt initialization (clint_init)
- Context switch in scheduler with trap frame handling
- Basic fork (sys_clone) implementation with COW semantics
- TaskControlBlock with kernel stack + trap frame allocation
- Trap handling, timer interrupts, syscall dispatch
- VFS, RAM filesystem
- Basic syscalls: read, write, getpid, sched_yield, exit, clone
- User mode return via return_to_user function (integrated in scheduler)
- Sv39 page table with COW support
- ELF binary loader infrastructure

**Working**: Debug mode runs successfully with full boot sequence.
**Issue**: Timer interrupt does not fire in QEMU with RustSBI-QEMU. This prevents preemption and multi-tasking.

### Build & Run

```bash
# Debug mode (WORKS)
cargo build -p os
cargo run -p os

# Release mode (has hang issue - investigate later)
cargo build --release -p os
cargo run --release -p os
```

### Debug Mode Boot Sequence (Working)
```
RustSBI → Boot 1 → memory init → SMP init (SXCIE) →
[process] init → [clint] timer init → [fs] VFS init →
[run] first process → [sched] scheduler → [sched] task created
→ idle task running with wfi (timer interrupt not firing)
```

## Architecture

**Memory Layout**:
- 0x80000000: DRAM base (physical)
- 0x80200000: Kernel text start
- Sv39 user space: 0x0 - 0x3FFFFFFFFFFF (128GB)

**Key Constants**: PAGE_SIZE=4096, MAX_TASKS=256

## Key Files

| File | Purpose |
|------|---------|
| `os/src/boot.rs` | Entry point, boot stages, trap entry asm |
| `os/src/process/mod.rs` | Task manager, scheduler, schedule_preempt, do_schedule, start_scheduler |
| `os/src/process/task.rs` | TaskControlBlock, kernel stack allocation, user address space |
| `os/src/process/context.rs` | TrapFrame, TaskContext, context_switch, return_to_user asm |
| `os/src/process/scheduler.rs` | MLFQ scheduler implementation |
| `os/src/trap/mod.rs` | Trap handling, timer interrupts, handle_trap |
| `os/src/syscall/mod.rs` | Syscall dispatcher, sys_clone, sys_execve (stub) |
| `os/src/memory/Sv39.rs` | Sv39 page table, COW support, handle_cow_page |
| `os/src/drivers/interrupt.rs` | CLINT timer, PLIC interrupts |
| `os/src/vfs/mod.rs` | Virtual File System |
| `os/src/fs/ramfs.rs` | RAM filesystem |
| `os/src/elf.rs` | ELF binary loader |

## Context Switch Flow

### Kernel Thread (context_switch)
1. Save callee-saved registers to old context (ra, sp, s0-s11)
2. Load callee-saved registers from new context
3. Return to new context's ra (which is the task entry point)

### User Task (return_to_user)
1. Set new page table (satp)
2. Flush TLB (sfence.vma)
3. Set sepc to user program counter
4. Set sp to user stack
5. Restore registers from trap frame
6. Set sstatus (SPP=0 for user mode, SPIE=1, SIE=0)
7. Return to user mode via sret

### Scheduler Flow (timer interrupt - NOT WORKING)
1. Timer interrupt fires → trap entry saves registers to trap frame
2. handle_trap() called → CURRENT_TRAP_FRAME set
3. handle_timer_interrupt() → schedule_preempt()
4. schedule_preempt(): save current task's trap frame, fetch next task
5. Copy next task's trap frame to current trap frame location
6. Return from trap via sret

## Timer Interrupt Issue

**Status**: NOT WORKING - Timer interrupt does not fire in QEMU with RustSBI-QEMU 0.2.0-alpha.10

**Symptoms**: OS boots and runs idle task on wfi, but timer interrupt never fires despite being armed.

**Debug Attempts**:
- Tried direct CLINT access (set_mtimecmp)
- Tried SBI_SET_TIMER via ecall
- Verified STIE bit is set in sie
- Verified mideleg has stimer delegated
- QEMU debug output shows only supervisor_ecall, no timer interrupts

**Root Cause**: Likely QEMU/RustSBI compatibility issue with CLINT timer emulation.

**Workaround**: Currently using wfi (wait for interrupt) in idle task, but timer-based preemption doesn't work.

## Next Steps (Priority Order)

1. **Install RISC-V toolchain** - Build user programs (blocker: no toolchain)
2. **Complete sys_execve implementation** - Load ELF into user address space
3. **Test user mode return** - Verify return_to_user works correctly
4. **Debug timer interrupt** - Or find alternative scheduling approach
5. **Fix release mode** - spin::Mutex optimization issue

## Development Notes

### Debug vs Release
- Debug mode: All boot stages complete, scheduler runs, idle task runs on wfi
- Release mode: Hang in process::init(), likely spin::Mutex issue

### RISC-V Toolchain
- Not installed yet
- Need to run `user/build-toolchain.sh` to download prebuilt toolchain
- Or install via package manager: `riscv64-unknown-elf-gcc`

### User Program Structure
User programs in `user/`:
- `user/src/hello.rs` - Hello world program
- `user/src/shell.rs` - Simple shell
- `user/src/vi.rs` - VI-like text editor

### Syscall Implementation
- sys_clone: Creates child process with COW address space
- sys_execve: Stub - needs full implementation to load ELF
- sys_exit: Halts the process
- sys_sched_yield: Signals schedule request

### ELF Loading
- ELF structures defined in `elf.rs` (Elf64Header, Elf64Phdr, etc.)
- `load_elf()` function exists but not integrated with sys_execve
- Needs: read from VFS, parse headers, set up page table, copy segments
