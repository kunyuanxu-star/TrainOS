# TrainOS - Claude Code Context

## Project Overview
TrainOS is an educational operating system written in Rust for RISC-V 64-bit architecture (rv64gc). Uses RustSBI as boot firmware, runs in QEMU virt machine.

**Goal**: Surpass Linux in kernel architecture, security, performance, and developer experience.

## Current Status (2026-03-30)

### Phase 1: Make It Runnable (In Progress)

**Completed**:
- Timer interrupt initialization (using SBI interface)
- Context switch in scheduler with trap frame handling
- Basic fork (sys_clone) implementation
- TaskControlBlock with kernel stack + trap frame allocation
- Trap handling, timer interrupts, syscall dispatch
- VFS, RAM filesystem, TCP/IP stack (stub)
- Basic syscalls: read, write, getpid, sched_yield
- User mode return via return_to_user function

**Working**: Debug mode runs successfully with full boot sequence.
**Issue**: Timer interrupt does not fire reliably in QEMU with RustSBI-QEMU. The OS boots and runs idle task but timer interrupt mechanism needs debugging.

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
→ idle task running with wfi
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
| `os/src/boot.rs` | Entry point, boot stages |
| `os/src/process/mod.rs` | Task manager, scheduler, schedule_preempt, start_scheduler |
| `os/src/process/task.rs` | TaskControlBlock, kernel stack allocation |
| `os/src/process/context.rs` | TrapFrame, TaskContext, context_switch, return_to_user asm |
| `os/src/trap/mod.rs` | Trap handling, timer interrupts |
| `os/src/syscall/mod.rs` | Syscall dispatcher, sys_clone, sys_execve |
| `os/src/memory/Sv39.rs` | Sv39 page table, COW support |
| `os/src/drivers/interrupt.rs` | CLINT timer, PLIC interrupts |

## Context Switch Flow

### Kernel Thread (context_switch)
1. Save callee-saved registers to old context
2. Load callee-saved registers from new context
3. Return to new context's ra

### User Task (return_to_user)
1. Set new page table (satp)
2. Flush TLB
3. Set sepc to user program counter
4. Set sp to user stack
5. Restore registers from trap frame
6. Set sstatus (SPP=0 for user mode, SPIE=1)
7. Return to user mode via sret

## Timer Interrupt Issue

The timer interrupt is set via SBI_SET_TIMER but does not fire in QEMU with RustSBI-QEMU 0.0.2. This prevents preemption and multi-tasking.

**Symptoms**: OS boots and runs idle task on wfi, but timer interrupt never fires.

**Possible causes**:
- QEMU virt CLINT emulation issue with RustSBI
- SBI timer function not properly emulated
- Timebase/clock configuration issue

## Next Steps (Priority Order)

1. **Debug timer interrupt** - Timer doesn't fire, preventing preemption
2. **Create user task with address space** - Test user mode return
3. **ELF loading** - Load ELF binaries into user space
4. **Fix release mode** - spin::Mutex optimization issue
5. **COW fork** - Full copy-on-write fork implementation

## Development Notes

### Debug vs Release
- Debug mode: All boot stages complete, scheduler runs, idle task runs on wfi
- Release mode: Hang in process::init(), likely spin::Mutex issue

### spin::Mutex Issues
Release mode hang seems related to spin::Mutex at opt-level=2. Possible causes:
- Memory ordering issues
- Lock elision optimization
- Static initialization order (FIO)
- Try opt-level=1 or different Mutex crate

### Timer Interrupt Debugging
- Using SBI_SET_TIMER (a7=0, a0=target_time)
- get_mtime() reads from CLINT MMIO (0x200bff8)
- set_timer_relative() sets mtimecmp via SBI
- Timer armed but never fires in QEMU
- Only supervisor_ecall seen in QEMU interrupt debug output
