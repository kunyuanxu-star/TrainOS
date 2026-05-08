# TrainOS Architecture Guide

## Microkernel Design

TrainOS follows the minimalist microkernel philosophy. The kernel provides exactly four mechanisms and nothing more:

1. **Capability System** — Access control tokens (CNode, Mint/Copy/Move/Revoke/Delete)
2. **IPC Router** — Synchronous message passing (endpoints, 64-byte payload, cap transfer)
3. **Scheduler** — 64 priority levels, bitmap O(1), SMP-aware spinlock, priority inheritance
4. **Memory Manager** — Buddy allocator, Sv39 page tables, COW fork

All other functionality (filesystem, network, device drivers, shell, POSIX) runs as user-space services communicating via IPC.

## Boot Sequence

1. machina loads RustSBI at 0x80000000
2. RustSBI loads kernel ELF at 0x80200000, jumps to _start in S-mode
3. _start: read HART ID, set per-HART stack, jump to rust_main
4. rust_main: clear BSS, init memory (buddy + Sv39 + heap), init CLINT/trap, init cap/ipc, enable MMU, spawn services, start scheduler

## Memory Layout

- 0x80000000-0x80020000: RustSBI firmware
- 0x80020000-0x80200000: Device tree / reserved
- 0x80200000-0x80800000: Kernel (text, rodata, data, bss, heap)
- 0x80800000-0x88000000: User pages (buddy-managed)

## Syscall Convention

- a7 = syscall number
- a0-a5 = arguments  
- a0 = return value (0 = success, usize::MAX = error)
- ecall from U-mode traps to S-mode kernel

## IPC Protocol

### Well-Known Endpoints
| EP | Service | Purpose |
|----|---------|---------|
| 1 | init | System init, IPC receiver |
| 2 | fs | File system service |
| 3 | net | Network stack (port routing) |

### Message Format
64-byte payload + optional cap transfers. Short messages only.

### FS Operations (via EP 2)
| Opcode | Name | Description |
|--------|------|-------------|
| 2 | READ | Read stored data |
| 3 | WRITE | Write data to store |
| 4 | APPEND | Append data to store |
