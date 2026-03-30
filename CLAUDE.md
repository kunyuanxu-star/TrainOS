# TrainOS - Claude Code Context

## Project Overview
TrainOS is an educational operating system written in Rust for RISC-V 64-bit architecture (rv64gc). It uses RustSBI as the boot firmware and runs in QEMU virt machine.

**Goal**: Build an OS that surpasses Linux in four dimensions:
1. Kernel Architecture & Extensibility
2. Security & Isolation
3. Performance & Concurrency
4. Developer Experience

## Current Status (2026-03-30)

### Phase 1: Make It Runnable (In Progress)

**Completed**:
- Timer interrupt initialization (clint_init)
- Working context switch in scheduler
- Basic fork (sys_clone) implementation
- Task control block with kernel stack allocation
- Trap frame setup for context switching
- Basic system calls (read, write, getpid, sched_yield)
- VFS and RAM filesystem
- TCP/IP network stack (stub)

**Debug Mode**: Working correctly. All boot stages complete, scheduler runs.

**Release Mode**: Has issues - hangs after "[process] Init start". Likely a compiler optimization issue with spin::Mutex or memory ordering. Issue persists even with opt-level=1.

### Build & Run

```bash
# Debug mode (works)
cargo build -p os
cargo run -p os

# Release mode (has hang issue)
cargo build --release -p os
cargo run --release -p os
```

### Debug Mode Output (Working)
```
Boot 1
memory init start
After memory init
SXCIEBoot 3
[process] Init start
[process] Init OK
Boot 4
[clint] Initializing CLINT timer
[clint] Timer interrupts enabled
OK
Boot 5
[fs] Initializing file system...
[vfs] VFS initialized
[ramfs] RAM filesystem initialized
Boot 6
[run] Starting first process
[sched] Starting scheduler
[sched] Task created
```

## Next Steps (Priority Order)

1. **Fix release mode hang** - Debug spin::Mutex or inline asm issues
2. **Implement user mode return** - Need proper sret from trap handler
3. **Complete ELF loading** - Parse and load ELF binaries
4. **Implement proper COW fork** - Page table copying
5. **User space programs** - Build hello, shell, vi

## Key Files

| File | Purpose |
|------|---------|
| `os/src/boot.rs` | Entry point, boot stages |
| `os/src/process/mod.rs` | Task manager, scheduler, context switch |
| `os/src/process/task.rs` | TaskControlBlock, kernel stack |
| `os/src/process/context.rs` | TrapFrame, TaskContext, assembly |
| `os/src/trap/mod.rs` | Trap handling, timer interrupts |
| `os/src/syscall/mod.rs` | System call dispatcher |
| `os/src/memory/Sv39.rs` | Page table, COW support |

## Architecture

**Memory Layout**:
- 0x80000000: DRAM base (physical)
- 0x80200000: Kernel text start
- Sv39 user space: 0x0000000000000000 - 0x00003FFFFFFFFFFF (128GB)

**Key Constants**:
- PAGE_SIZE: 4096 bytes
- KERNEL_STACK_SIZE: 1 page (4096 bytes)
- MAX_TASKS: 256

## Development Notes

### spin::Mutex Issues
The release mode hang seems to be related to spin::Mutex usage. Debug mode works fine. Possible causes:
- Memory ordering issues
- Lock elision optimization
- Static initialization order

### Context Switch Flow
1. Timer interrupt fires
2. trap handler saves state to current trap frame
3. schedule_preempt() called
4. Save current task state, fetch next task
5. Copy next task's trap frame to current
6. Return from trap via sret

### Known Issues
1. Release mode hang in process::init() - debug mode works
2. SMP init outputs corrupted text (SXCIEBoot 3)
3. No proper user mode return yet
4. ELF loading is stub
