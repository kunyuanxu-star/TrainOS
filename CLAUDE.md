# TrainOS — Claude Code Context

## Project Overview
TrainOS is a microkernel operating system in Rust for RISC-V 64-bit (rv64gc).
Uses RustSBI as boot firmware, runs on machina emulator.

**Goal**: Surpass Linux in kernel architecture, security, and performance.

## Iron Rules
1. Runtime: RustSBI (M-mode firmware) + machina (RISC-V JIT emulator).
2. Architecture: RISC-V 64-bit (rv64gc), Sv39 virtual memory.
3. Language: Rust nightly (no_std kernel, no_std user-space services).

## Current Status (2026-05-07)

### V1 Complete
- Boot: BSS clear, console via SBI ecall
- Physical memory: Buddy allocator (orders 0-12, 4KB-16MB blocks)
- Virtual memory: Sv39 page tables, 2MB superpage kernel identity map, MMU enable
- Trap handling: Assembly entry/exit with full register save/restore
- Timer: CLINT timer interrupts at 10ms tick
- Process model: Single-threaded Process, ELF64 loader, per-process page tables
- Scheduler: 64 priority levels, bitmap O(1) picking, priority-based preemption
- User mode: init process spawns, prints "TrainOS ready" via ecall->SBI, loops

### Architecture

**Microkernel** -- kernel provides:
- Physical memory allocation (buddy)
- Sv39 virtual memory (map, unmap, page walk)
- Synchronous IPC routing (endpoint-based message passing)
- Priority scheduling with inheritance
- Trap/interrupt dispatch

User-space services (drivers, filesystem, network, POSIX) communicate via IPC.

## Build & Run

```bash
# Build kernel (includes embedded init service)
cd TrainOS && cargo build --release -p kernel

# Run on machina (from testOS/)
./machina/target/release/machina \
  -M riscv64-ref \
  -bios machina/pc-bios/rustsbi-riscv64-machina-fw_dynamic.bin \
  -kernel TrainOS/target/riscv64gc-unknown-none-elf/release/kernel \
  -nographic
```

## Key Files

| File | Purpose |
|------|---------|
| `kernel/src/main.rs` | Entry point, boot sequence, init spawn |
| `kernel/src/console.rs` | SBI console debug output |
| `kernel/src/mem/layout.rs` | Physical/virtual address constants |
| `kernel/src/mem/buddy.rs` | Buddy allocator for physical pages |
| `kernel/src/mem/sv39.rs` | Sv39 page table types, walk, map, MMU enable |
| `kernel/src/mem/heap.rs` | Kernel heap bump allocator |
| `kernel/src/trap/asm.rs` | Trap entry/exit assembly |
| `kernel/src/trap/mod.rs` | Trap dispatch, CLINT timer, TrapFrame |
| `kernel/src/proc/process.rs` | Process struct |
| `kernel/src/proc/thread.rs` | Thread, TaskContext |
| `kernel/src/proc/switch.rs` | Context switch + user_trap_return assembly |
| `kernel/src/proc/elf.rs` | ELF64 loader |
| `kernel/src/proc/mod.rs` | Process manager (spawn, process table) |
| `kernel/src/sched/mod.rs` | 64-priority scheduler |

## Next Steps
- Capability system (access control for all resources)
- IPC (endpoint send/recv/call/reply)
- Multi-process execution
- User-space drivers (VirtIO block/net)
