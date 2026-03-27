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
│       └── trap/        # Trap/interrupt handling
└── user/                # User space programs
    └── src/
        └── main.rs      # Hello world program
```

## Features

### Implemented

- **Bootstrapping**: Assembly entry point with stack setup
- **Console I/O**: SBI-based putchar for text output
- **Memory Management**: Sv39 page table structures and bitmap-based physical page allocator
- **Process Management**: Task control block, scheduler infrastructure
- **Trap Handling**: Exception and interrupt handling with proper stvec/sstatus setup
- **System Calls**: Framework for write, read, fork, exec, exit, getpid, sched_yield

### In Development

- Full context switching between tasks
- User program loading and execution
- Timer interrupt handling for preemption
- Virtual memory activation (currently uses identity mapping)
- File system implementation with disk I/O

## System Call Interface

TrainOS implements the following system calls:

| ID  | Name        | Description                    |
|-----|-------------|--------------------------------|
| 0   | read        | Read from file descriptor      |
| 1   | write       | Write to file descriptor       |
| 2   | open        | Open a file                    |
| 3   | close       | Close a file descriptor        |
| 4   | fork        | Create a child process         |
| 5   | exec        | Execute a program             |
| 6   | wait        | Wait for child process         |
| 7   | exit        | Terminate current process      |
| 8   | getpid      | Get current process ID         |
| 9   | getppid     | Get parent process ID         |
| 10  | sched_yield | Yield CPU to scheduler        |

## Memory Layout

For QEMU virt machine:

- `0x80000000` - DRAM base (physical memory start)
- `0x80200000` - Kernel text start (linked base address)
- `0x80300000` - Kernel end (symbol `end`)
- `0x80400000`+ - Available for physical page allocation

Virtual address space (Sv39):
- 256GB total addressable space
- 4KB pages with 3-level page table

## Development Status

This is an educational project. The following are on the development roadmap:

1. **Process Scheduling**: Implement round-robin scheduling with context switching
2. **User Mode Execution**: Set up proper page tables and run user programs
3. **Timer Interrupts**: Implement preemption via supervisor timer interrupt
4. **Virtual Memory**: Activate Sv39 page table and implement memory mapping
5. **File System**: Implement disk I/O and file operations
6. **System Calls**: Complete all syscall implementations

## Contributing

This is a student project for learning operating system development. Contributions in the form of bug fixes, documentation improvements, and feature implementations are welcome.

## License

This project is for educational purposes.

## Acknowledgments

Built with inspiration from various RISC-V OS tutorials and the RISC-V SBI specification.
