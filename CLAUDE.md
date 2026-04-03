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
- Kernel page table with identity mappings (0x80200000-0x80400000) created during init
- Context switch in scheduler with trap frame handling
- Basic fork (sys_clone) implementation with COW semantics
- TaskControlBlock with kernel stack + trap frame allocation
- Trap handling, timer interrupts, syscall dispatch
- VFS, RAM filesystem
- Basic syscalls: read, write, getpid, sched_yield, exit, clone
- Sv39 page table infrastructure with COW support
- ELF binary loader infrastructure
- RISC-V toolchain installed (xPack v12.3.0-2)
- User programs compiled (hello, shell, vi) for RISC-V

**Working**: Debug mode runs successfully with full boot sequence, idle task loops on wfi, ELF parsing works.

**Issues**:
1. **SATP=0 (bare mode)** - Kernel is running without Sv39 MMU enabled. Kernel page table is created with identity mappings but Sv39 enable hangs during SATP write.
2. **Sv39 enable hangs** - When enable_sv39() writes to SATP, system hangs. The identity mapping exists but the SATP write or subsequent instruction fetch causes issues. Suspected: VPN index calculation may exceed 9-bit range.
3. **sie CSR write hangs** - Writing to sie (Supervisor Interrupt Enable) causes QEMU to hang. Using direct CLINT MMIO for timer instead.
4. **Timer interrupt not firing** - Even with CLINT directly programmed, sie.STIE cannot be set due to sie write hang.
5. **User program loading deferred** - Full user address space creation requires enabling Sv39 properly first.

### Recent Fixes (2026-04-03)

**Page table pool fix**:
- PT pool moved from 0x80000000 (PMP3 read-only) to 0x80080000 (PMP6 RWX)
- Added allocated_frames bitmap to PageTablePool to properly track allocations
- General allocator and PT pool now use non-overlapping PA ranges

**Sv39 enable investigation**:
- Page table structure created: root at 0x80080000, level1 at 0x80081000, level2 at 0x80082000
- Kernel identity mapping: VA 0x80200000-0x80400000 mapped to PA 0x80200000-0x80400000
- SATP write causes hang - possibly due to VPN[0] index calculation (0x80200000 >> 30 = 512, but max is 511)

### Timer Issue Details (2026-04-02)

**Problem**: Writing to sie CSR (0x104) causes QEMU to hang completely.

**Investigation**:
- `csrr sie, 0x104` works fine (reads sie = 0x0)
- `csrw sie, 0x22` (setting STIE+SSIE) hangs
- `csrs sie, 0x22` (atomic set) also hangs
- Same issue with any sie write

**Workaround Found**: Use direct CLINT MMIO access instead of sie CSR:
```rust
// Direct CLINT access at 0x2004000 (mtimecmp for hart 0)
let clint_mtimecmp: *mut u64 = 0x2004000 as *mut u64;
let mtime: u64 = core::ptr::read_volatile(0x200bff8 as *const u64); // mtime
core::ptr::write_volatile(clint_mtimecmp, mtime.wrapping_add(100_000));
```

### Bare Mode Issue (2026-04-02)

**Problem**: SATP register reads as 0, indicating MMU is in "bare" mode (no Sv39 page tables).

**Investigation**:
- SATP = 0 means no Sv39 translation is active
- In bare mode, VA = PA directly (no MMU translation)
- Page tables created in this mode have no effect on memory access
- PMP (Physical Memory Protection) is still active and restricts access to certain addresses

**Discovery**: Writing to PA 0x80000000 causes a store fault, but writing to 0x80071000 succeeds.

**Analysis of PMP configuration**:
- PMP 0: OFF (no protection)
- PMP 1-2: TOR, RWX at 0x80000000 (covers [0, 0x80000000))
- PMP 3: TOR, R at 0x80026000 (covers [0x80000000, 0x80026000))
- PMP 4: TOR, NONE at 0x80035000 (covers [0x80026000, 0x80035000))
- PMP 5: TOR, RW at 0x80071000 (covers [0x80035000, 0x80071000))
- PMP 6: TOR, RWX at 0x88000000 (covers [0x80071000, 0x88000000))

**Current Solution**: General allocator starts at base_page = 0x80071 (PA 0x80071000), which is in the RWX PMP region.

### Page Table Fix (2026-04-02)

**Previous Issue**: RustSBI page table only had limited RAM identity-mapped, causing intermediate page table allocation to fail when creating user address spaces.

**Fix Applied**: Page table allocator now uses fixed identity-mapped region at 0x80000000-0x80040000 (256KB = 64 page table frames).

This ensures all page table frames are always accessible via the identity mapping that RustSBI sets up.

## Recent Fixes

**Page table allocator fix (2026-04-02)**:
- Changed from dynamic allocation to fixed identity-mapped region
- Page table pool now at PA 0x80000000-0x80040000
- All intermediate page tables now accessible

**Stack size limit (2026-04-01)**:
- 2MB stack (512 pages) fails during allocation
- 1.99MB (508 pages) works, 1.95MB (504 pages) works
- 256KB (64 pages) works reliably

**return_to_user register offsets (2026-03-30)**:
- Fixed bug in `os/src/process/context.rs` where assembly was loading registers with incorrect offsets
- TrapFrame layout: ra(0), sp(8), gp(16), tp(24), t0(32), ...

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
[process] init → [clint] timer init (direct MMIO) → [fs] VFS init →
[run] first process → [sched] scheduler →
[sched] Idle loop - user mode loading pending verification
```

## Architecture

**Memory Layout**:
- 0x80000000: DRAM base (physical)
- 0x80000000-0x80040000: Page table pool (identity-mapped)
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
| `os/src/memory/mod.rs` | Memory subsystem init, page table allocator |
| `os/src/drivers/interrupt.rs` | CLINT timer (direct MMIO), PLIC interrupts |
| `os/src/vfs/mod.rs` | Virtual File System |
| `os/src/fs/ramfs.rs` | RAM filesystem |
| `os/src/elf.rs` | ELF binary loader |

## RustSBI Integration

**Built from source**: RustSBI 0.4.0 (prototyper) was successfully built from the main repository.

**Issues with new RustSBI**:
- SBI_SET_TIMER causes hang (unlike older firmware)
- sie CSR write causes hang
- Direct CLINT MMIO works as workaround

**Memory region**: 0x80000000 - 0x88000000 (128MB as reported by RustSBI)

## Next Steps (Priority Order)

1. **Enable Sv39 properly** - Kernel currently runs in bare mode (SATP=0). Need to enable Sv39 with proper identity mapping for kernel region.
2. **Verify user address space loading** - Once Sv39 is enabled, test page table creation and user program loading.
3. **Debug sie CSR write hang** - Understand why sie write hangs in QEMU (separate from bare mode issue).
4. **Timer interrupt in QEMU** - Enable timer interrupts despite sie write issue.
5. **Complete sys_execve implementation** - Load ELF into user address space once Sv39 is working.
5. **Test user mode return** - Verify return_to_user works correctly
6. **Fix release mode** - spin::Mutex optimization issue

## Timer Interrupt Issue (UNRESOLVED)

**Status**: Timer cannot be enabled via sie.STIE due to sie write hang.

**Current workaround**: Direct CLINT MMIO access for timer, but interrupts still not enabled because sie.STIE cannot be set.

**Analysis**:
- sie CSR reads work (returns 0x0 initially)
- Any write to sie (csrw, csrs) causes QEMU to hang
- This affects both STIE (timer) and SSIE (software interrupt)
- PLIC interrupts also go through sie, but we haven't tested those

## RISC-V Toolchain (INSTALLED)

**xPack RISC-V Embedded GCC v12.3.0-2** installed at:
- `downloads/xpack-riscv-none-elf-gcc-12.3.0-2/`

**User programs** (built for riscv64gc-unknown-none-elf):
- Can be built via `cargo build --target riscv64gc-unknown-none-elf --release -p user`

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
- BIOS: rustsbi-qemu-new.bin (v0.4.0 built from source)
- CLINT: 0x2000000 (verified in PMP configuration)
- Timebase: 10 MHz (default)

### Syscall Implementation
- sys_clone: Creates child process with COW address space
- sys_execve: Stub - needs full implementation to load ELF
- sys_exit: Halts the process (loops on wfi)
- sys_sched_yield: Signals schedule request (but timer needed for actual preemption)
