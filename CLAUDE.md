# TrainOS - Claude Code Context

## Project Overview
TrainOS is an educational operating system written in Rust for RISC-V 64-bit architecture (rv64gc). Uses RustSBI as boot firmware, runs in QEMU virt machine.

**Goal**: Surpass Linux in kernel architecture, security, performance, and developer experience.

## Current Status (2026-04-02)

### Phase 1: Make It Runnable (In Progress)

**Completed**:
- Boot sequence runs successfully through all stages
- Context switch in scheduler with trap frame handling
- Basic fork (sys_clone) implementation with COW semantics
- TaskControlBlock with kernel stack + trap frame allocation
- Trap handling, timer interrupts, syscall dispatch
- VFS, RAM filesystem
- Basic syscalls: read, write, getpid, sched_yield, exit, clone
- Sv39 page table with COW support
- ELF binary loader (infrastructure complete, user mode loading blocked by page table issue)
- RISC-V toolchain installed (xPack v12.3.0-2)
- User programs compiled (hello, shell, vi) for RISC-V

**Working**: Debug mode runs successfully with full boot sequence, idle task loops on wfi.

**Issues**:
1. **Timer interrupt does not fire in QEMU with RustSBI-QEMU** - OS runs idle on wfi
2. **User program loading fails** - map_kernel causes panic when trying to map user pages into kernel page table

**Investigation Notes (2026-04-02)**:
- ELF header validation works correctly
- ELF header field reading works (e_entry, e_phoff, e_phentsize, e_phnum all read successfully)
- Panic occurs specifically when calling `map_kernel()` to create page table mappings
- The kernel page table from RustSBI may not support modifications, or there's an incompatibility in how we're trying to use it

### Recent Fixes

**Stack size limit found (2026-04-01)**:
- 2MB stack (512 pages) fails during allocation
- 1.99MB (508 pages) works, 1.95MB (504 pages) works
- 256KB (64 pages) works reliably

**return_to_user register offsets (2026-03-30)**:
- Fixed bug in `os/src/process/context.rs` where assembly was loading registers with incorrect offsets
- TrapFrame layout: ra(0), sp(8), gp(16), tp(24), t0(32), ... but assembly was loading gp from offset 8, tp from 16, etc.

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
[run] first process → [sched] scheduler →
[sched] Idle loop - user mode loading pending fix
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

## Timer Interrupt Issue (UNRESOLVED)

**Status**: Timer interrupt does NOT fire in QEMU with RustSBI-QEMU v0.1.1

**Attempts to fix**:
1. ✅ Updated from RustSBI-QEMU v0.2.0-alpha.10 to v0.1.1 - No change
2. ✅ Tried direct CLINT access (set_mtimecmp) - No change
3. ✅ Tried SBI_SET_TIMER via ecall - No change
4. ✅ Verified STIE bit is set in sie register - Confirmed
5. ✅ Verified mideleg has stimer delegated (0x1666) - Confirmed
6. ✅ Tried different timebase frequencies (10MHz, 100MHz) - No change
7. ✅ Tried ACLINT option (-machine aclint=true) - No change
8. ✅ Tried RTC configuration options - No change
9. ✅ QEMU debug output shows only supervisor_ecall, NO timer interrupts

**Analysis**:
- The timer is being set correctly (mtimecmp is written)
- Interrupts are properly delegated to supervisor mode (mideleg.stimer = 1)
- Timer interrupt enable bit is set (sie.stie = 1)
- But the timer interrupt never fires in QEMU

**Root Cause**: Likely QEMU's CLINT timer emulation issue when using RustSBI as firmware.
- QEMU may not be correctly emulating the timer hardware
- Or there may be a timing issue where the timer fires before the OS is ready to handle it

**Workaround**: OS runs idle task on wfi, but timer-based preemption doesn't work.

## Next Steps (Priority Order)

1. **Investigate map_kernel panic** - The kernel page table modification causes panics, need to understand why
2. **Debug timer issue** - Try different QEMU versions or OpenSBI firmware
3. **Complete sys_execve implementation** - Load ELF into user address space once page table issue is fixed
4. **Test user mode return** - Verify return_to_user works correctly
5. **Fix release mode** - spin::Mutex optimization issue

## User Address Space Loading Issue (2026-04-01)

**Symptom**: ELF loading panics during page table mapping when trying to create a user address space.

**Root Cause**:
- When creating a user page table, we need to allocate intermediate page tables (level-1, level-2)
- These page tables are allocated at physical addresses (e.g., 0x80800000+)
- We then try to access them by casting PA to a pointer and dereferencing
- BUT: the kernel page table from RustSBI does NOT have all physical memory mapped
- Only specific PA ranges are identity-mapped (likely 0x80000000-0x80400000)
- When we try to access PA 0x80800000+ via a raw pointer, the MMU tries to translate it through the kernel page table, fails, and we get a fault

**Evidence**:
- First ELF segment (VA 0x10000, RO) maps successfully
- Second ELF segment (VA 0x1130c, R+E) fails during map()
- The first mapping creates the intermediate page tables, which works
- The second mapping reuses existing page tables but fails at a different level
- The failure is specifically in `map()` when accessing intermediate page table pages

**Detailed Trace**:
1. UserAddressSpace::new() creates PageTableManager::new() with fresh root
2. First segment mapping: creates level-1 and level-2 page tables - WORKS
3. Second segment mapping: tries to access level-2 entry - FAILS

**Why First Mapping Works**:
- The root page table was already mapped by RustSBI at a known PA
- When we access it, the kernel page table has the mapping

**Why Second Mapping Fails**:
- Level-1 and level-2 page tables were allocated at PAs like 0x80800000
- These PAs are NOT in the kernel's page table
- When we try to access them via `&mut *levelX_ptr`, the MMU lookup fails

**Possible Solutions**:
1. **Identity-map all RAM during boot**: Set up a proper kernel page table with full RAM mapped
2. **Use pre-allocated page table pool**: Allocate page tables from a pre-mapped region
3. **Build page tables before using them**: Use a two-phase approach where we first build the page table structure using a temporary mapping, then activate it
4. **Use kernel page table directly**: Instead of separate user page table, map user pages into kernel page table (but this breaks user/kernel isolation)

**Status**: Blocked - requires redesigning how page tables are allocated/accessed

## RISC-V Toolchain (INSTALLED)

**xPack RISC-V Embedded GCC v12.3.0-2** installed at:
- `downloads/xpack-riscv-none-elf-gcc-12.3.0-2/`

**User programs** (built for riscv64gc-unknown-none-elf):
- `target/riscv64gc-unknown-none-elf/release/hello` (ELF)
- `target/riscv64gc-unknown-none-elf/release/hello.bin` (raw binary, 6.6KB)

**Toolchain usage**:
```bash
export PATH="$PWD/downloads/xpack-riscv-none-elf-gcc-12.3.0-2/bin:$PATH"
riscv-none-elf-gcc --version
riscv-none-elf-objcopy -O binary input.elf output.bin
```

## Development Notes

### Debug vs Release
- Debug mode: All boot stages complete, scheduler runs, idle task runs on wfi
- Release mode: Hang in process::init(), likely spin::Mutex issue

### RISC-V Toolchain (xPack v12.3.0-2)
- Toolchain installed at `downloads/xpack-riscv-none-elf-gcc-12.3.0-2/`
- Binaries: riscv-none-elf-gcc, riscv-none-elf-as, riscv-none-elf-ld, riscv-none-elf-objcopy, etc.
- User programs can be built with: `cargo build --target riscv64gc-unknown-none-elf --release -p user`

### QEMU Configuration
- Machine: virt
- BIOS: rustsbi-qemu.bin (v0.1.1)
- CLINT: 0x2000000 (verified in PMP configuration)
- Timebase: 10 MHz (default)

### Syscall Implementation
- sys_clone: Creates child process with COW address space
- sys_execve: Stub - needs full implementation to load ELF
- sys_exit: Halts the process (loops on wfi)
- sys_sched_yield: Signals schedule request (but timer needed for actual preemption)
