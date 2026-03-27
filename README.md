# TrainOS

A Rust-based operating system kernel for the RISC-V 64-bit architecture.

## Overview

TrainOS is an educational operating system written from scratch in Rust. It targets the RISC-V Sv39 virtual memory architecture and is designed to run in QEMU with RustSBI firmware.

**Note**: TrainOS is an ongoing project in active development. Many features are still being implemented.

## Architecture

- **Architecture**: RISC-V 64-bit (rv64gc)
- **Virtual Memory**: Sv39 (3-level page table, 4KB pages)
- **Target**: QEMU virt machine with RustSBI
- **Language**: Rust (2021 edition, no_std)
- **Build Tool**: Cargo

## Building

### Prerequisites

- Rust nightly or stable with RISC-V support
- `riscv64gc-unknown-none-elf` target installed
- QEMU with RISC-V support

### Build Kernel

```bash
cargo build --release -p os
```

### Build User Programs

```bash
cargo build --release -p user --target riscv64gc-unknown-none-elf
```

### Run in QEMU

```bash
cargo run --release -p os
```

This runs the kernel in QEMU with:
- Machine: `virt`
- BIOS: RustSBI
- Output: Nographic (console)

## Project Structure

```
TrainOS/
├── Cargo.toml           # Workspace configuration
├── rust-toolchain.toml  # Rust toolchain settings
├── os/                  # Kernel crate
│   ├── Cargo.toml
│   ├── build.rs         # Build script
│   ├── linker.ld        # Custom linker script
│   └── src/
│       ├── main.rs      # Kernel entry point
│       ├── boot.rs      # Bootstrapping and trap entry
│       ├── console.rs   # SBI-based console output
│       ├── memory/      # Memory management (Sv39, allocator)
│       ├── process/     # Process/task management
│       ├── fs/          # File system (EasyFS structures)
│       ├── syscall/     # System call handling
│       ├── trap/        # Trap/interrupt handling
│       └── smp/         # SMP multi-core support
│           ├── cpu.rs   # Per-CPU structures
│           ├── hart.rs  # HART management
│           └── ipi.rs   # Inter-processor interrupts
├── user/                # User space programs
│   └── src/
│       └── main.rs      # Hello world program
└── README.md
```

## Features

### Implemented

- **Bootstrapping**: Assembly entry point with stack setup
- **Console I/O**: SBI-based putchar for text output
- **Memory Management**: Sv39 page table structures and bitmap-based physical page allocator
- **Process Management**: Task control block, scheduler infrastructure, process ID allocation
- **Trap Handling**: Exception and interrupt handling with proper stvec/sstatus setup
- **System Calls**: Linux-compatible syscall interface with common operations
- **SMP Support**: Per-CPU data structures, HART management, IPI infrastructure
- **File System Structures**: EasyFS superblock, inode, and directory entry definitions

### Linux Syscall Compatibility

TrainOS implements Linux-compatible syscall numbers for easier porting:

| Syscall | Number | Description |
|---------|--------|-------------|
| read | 63 | Read from file descriptor |
| write | 64 | Write to file descriptor |
| exit | 93 | Terminate current process |
| getpid | 172 | Get current process ID |
| getppid | 173 | Get parent process ID |
| brk | 214 | Change data segment size |
| mmap | 222 | Memory map |
| munmap | 215 | Unmap memory |
| clone | 220 | Create a new process |
| sched_yield | 124 | Yield CPU to scheduler |

### In Development

- **Copy-on-Write Fork**: Efficient process creation by sharing page tables until write
- **Context Switching**: Full task switching with saved registers
- **User Mode Execution**: Set up proper page tables and run user programs
- **Timer Interrupts**: Preemption via supervisor timer interrupt
- **Virtual Memory**: Activate Sv39 page table and implement memory mapping
- **File System**: Disk I/O and file operations using EasyFS structures
- **Async Runtime**: no_std compatible async/await infrastructure

## Memory Layout

For QEMU virt machine:

- `0x80000000` - DRAM base (physical memory start)
- `0x80200000` - Kernel text start (linked base address)
- `0x80300000` - Kernel end (symbol `end`)
- `0x80400000`+ - Available for physical page allocation and heap

Virtual address space (Sv39):
- 256GB total addressable space
- 4KB pages with 3-level page table

## SMP Architecture

TrainOS supports multi-core processors through:

- **Per-CPU Data**: Each CPU core has its own local data structure
- **HART Management**: Hardware thread detection and management
- **IPI Support**: Inter-processor interrupts for communication
- **Thread-Local Storage**: Per-core data isolation

## System Call Interface

The syscall module (`os/src/syscall/`) provides:

- `syscall/mod.rs`: Main syscall dispatcher with Linux-compatible numbers
- `syscall/memory.rs`: Memory-related syscalls (mmap, munmap, mprotect, brk)
- `syscall/task.rs`: Process/thread management structures
- `syscall/fs.rs`: File operations infrastructure

## Development Status

This is an educational project. The current roadmap:

1. **In Progress**: COW fork implementation for efficient process creation
2. **In Progress**: Full context switching between tasks
3. **Planned**: User mode execution with proper page tables
4. **Planned**: Timer interrupt handling for preemption
5. **Planned**: Async runtime for event-driven programming

## Contributing

This is a student project for learning operating system development. Contributions in the form of bug fixes, documentation improvements, and feature implementations are welcome.

## License

This project is for educational purposes.

## Acknowledgments

Built with inspiration from various RISC-V OS tutorials and the RISC-V SBI specification.
