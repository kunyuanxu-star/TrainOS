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
- VFS, RAM filesystem, TCP/IP stack (stub)
- Basic syscalls: read, write, getpid, sched_yield, exit, clone
- User mode return via return_to_user function (integrated in scheduler)
- Sv39 page table with COW support

**Working**: Debug mode runs successfully with full boot sequence.
**Issue**: Timer interrupt does not fire in QEMU with RustSBI-QEMU. This prevents preemption and multi-tasking. The OS boots and runs but idle task stays on wfi.

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
| `os/src/syscall/mod.rs` | Syscall dispatcher, sys_clone, sys_execve, sys_exit |
| `os/src/memory/Sv39.rs` | Sv39 page table, COW support, handle_cow_page |
| `os/src/drivers/interrupt.rs` | CLINT timer, PLIC interrupts |
| `os/src/vfs/mod.rs` | Virtual File System |
| `os/src/fs/ramfs.rs` | RAM filesystem |

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

### Scheduler Flow (timer interrupt)
1. Timer interrupt fires → trap entry saves registers to trap frame
2. handle_trap() called → CURRENT_TRAP_FRAME set
3. handle_timer_interrupt() → schedule_preempt()
4. schedule_preempt(): save current task's trap frame, fetch next task
5. Copy next task's trap frame to current trap frame location
6. Return from trap via sret

## Timer Interrupt Issue

The timer interrupt is set via SBI_SET_TIMER but does not fire in QEMU with RustSBI-QEMU 0.2.0-alpha.10. This prevents preemption and multi-tasking.

**Symptoms**: OS boots and runs idle task on wfi, but timer interrupt never fires despite being armed.

**Possible causes**:
- QEMU virt CLINT emulation issue with RustSBI
- SBI timer function not properly emulated by RustSBI-QEMU
- Timebase/clock configuration issue
- Interrupt delegation issue (mideleg.stimer should be set)

**Workaround**: Currently using wfi (wait for interrupt) in idle task, but timer-based preemption doesn't work.

## Next Steps (Priority Order)

1. **Debug timer interrupt** - Timer doesn't fire, prevents preemption
2. **Install RISC-V toolchain** - Build user programs
3. **ELF loading** - Load ELF binaries into user space
4. **Test user mode return** - Verify return_to_user works correctly
5. **Fix release mode** - spin::Mutex optimization issue
6. **Complete COW fork** - Full copy-on-write fork implementation

## Development Notes

### Debug vs Release
- Debug mode: All boot stages complete, scheduler runs, idle task runs on wfi
- Release mode: Hang in process::init(), likely spin::Mutex issue

### Timer Interrupt Debugging
- Using SBI_SET_TIMER (a7=0, a0=target_time) via ecall
- Also tried direct CLINT access (set_mtimecmp)
- Timer armed but never fires in QEMU
- Only supervisor_ecall seen in QEMU interrupt debug output (-d int)
- mideleg shows stimer is delegated: ssoft, stimer, sext (0x1666)

### spin::Mutex Issues
Release mode hang seems related to spin::Mutex at opt-level=2. Possible causes:
- Memory ordering issues
- Lock elision optimization
- Static initialization order (FIO)
- Try opt-level=1 or different Mutex crate

### User Mode Return Requirements
1. Valid page table with user mappings (Sv39::UserAddressSpace)
2. Trap frame set up: sepc=user_pc, sstatus spp=0, spie=1, sie=0
3. User stack pointer set (user_sp)
4. SATP register set to user's page table (satp)
5. Return via sret (using return_to_user assembly)

### Syscall Implementation
- sys_clone: Creates child process with COW address space, child returns 0, parent returns child PID
- sys_exit: Halts the process (currently just loops on wfi)
- sys_getpid: Returns current PID from CURRENT_PID
- sys_sched_yield: Signals schedule request (but needs timer interrupt to work)
