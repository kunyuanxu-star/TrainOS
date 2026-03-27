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
│       ├── fs/          # File system (VFS, devfs, easyfs)
│       ├── syscall/     # System call handling (Linux-compatible)
│       │   ├── mod.rs   # Main dispatcher
│       │   ├── memory.rs # Memory syscalls
│       │   ├── fs.rs    # File syscalls
│       │   ├── task.rs  # Task-related syscalls
│       │   └── net.rs   # Socket syscalls
│       ├── trap/        # Trap/interrupt handling
│       ├── smp/         # SMP multi-core support
│       │   ├── cpu.rs   # Per-CPU structures
│       │   ├── hart.rs  # HART management
│       │   └── ipi.rs   # Inter-processor interrupts
│       ├── net/         # TCP/IP network stack
│       │   ├── mod.rs   # Core types (NetBuffer, MacAddr, IpAddr)
│       │   ├── eth.rs   # Ethernet II framing
│       │   ├── ipv4.rs  # IPv4 packet handling
│       │   ├── tcp.rs   # TCP streaming protocol
│       │   ├── udp.rs   # UDP datagram protocol
│       │   ├── arp.rs   # Address Resolution Protocol
│       │   └── dns.rs   # DNS client
│       ├── thread/      # Threading support (pthread-like)
│       │   └── mod.rs   # Thread, ThreadTable, Mutex, Cond, Barrier
│       └── drivers/     # Device drivers
│           ├── mod.rs   # Driver framework
│           ├── virtio/   # VirtIO core
│           ├── virtio_blk.rs # Block device driver
│           ├── virtio_net.rs  # Network device driver
│           ├── pci.rs    # PCI bus driver
│           └── interrupt.rs # Interrupt handling
├── user/                # User space programs
│   └── src/
│       └── main.rs      # Hello world program
└── README.md
```

## Features

### Implemented

- **Bootstrapping**: Assembly entry point with stack setup
- **Console I/O**: SBI-based putchar for text output
- **Memory Management**:
  - Sv39 virtual memory with 3-level page tables and 4KB pages
  - Physical page allocator (bitmap-based)
  - Copy-on-Write (COW) page support for efficient fork()
  - Virtual address translation and mapping
- **Process Management**:
  - Task control block with task ID, status, stack pointers
  - TaskManager for managing multiple tasks
  - Processor management with per-CPU state
  - Process ID allocation
- **Context Switching**: Full task switching with saved/restored callee-saved registers (s0-s11, sp, ra)
- **Trap Handling**: Exception and interrupt handling with proper stvec/sstatus setup
- **System Calls**: Linux-compatible syscall interface with extensive operations
- **SMP Support**: Per-CPU data structures, HART management, IPI infrastructure
- **Threading (pthread-like)**:
  - Thread creation and management (ThreadId, ThreadStatus, ThreadTable)
  - pthread attributes (stack size, guard size, scheduling policy)
  - Synchronization primitives: Mutex, Condition Variable, Barrier, RWLock
  - Thread-local storage (ThreadLocalKey)
  - Once primitive for one-time initialization
- **Networking (TCP/IP Stack)**:
  - Ethernet II framing (eth.rs)
  - IPv4 packet handling with fragmentation support (ipv4.rs)
  - TCP streaming protocol with connection state machine (tcp.rs)
  - UDP datagram protocol (udp.rs)
  - ARP for address resolution (arp.rs)
  - DNS client for domain name resolution (dns.rs)
  - Socket syscalls for Linux-compatible network API
- **File System**:
  - VFS (Virtual File System) layer with unified inode/file interface
  - Device file system (devfs) with /dev/null, /dev/zero, /dev/random, /dev/console
  - EasyFS structures (superblock, inode, directory entries)
- **Device Drivers**:
  - VirtIO block device driver (virtio-blk) for storage
  - VirtIO network device driver (virtio-net) for networking
  - PCI bus driver for device discovery
  - Interrupt handling infrastructure (PLIC support)

### Linux Syscall Compatibility

TrainOS implements Linux-compatible syscall numbers for easier porting:

| Category | Syscalls |
|----------|----------|
| Process | exit, exit_group, getpid, gettid, getppid, clone, wait4, waitid, execve, ptrace |
| Memory | brk, mmap, munmap, mprotect, mremap, madvise, mlock, munlock, msync |
| I/O | read, write, readv, writev, poll, select, sendfile, pipe2, dup, dup3, pread64, pwrite64 |
| File | openat, close, linkat, unlinkat, mkdirat, readlinkat, renameat2, truncate, ftruncate |
| Socket | socket, bind, connect, listen, accept, sendto, recvfrom, shutdown, setsockopt, getsockopt, getsockname, getpeername, socketpair |
| Epoll | epoll_create, epoll_ctl, epoll_wait |
| Signal | sigaction, sigprocmask, sigreturn, kill, tkill, sigaltstack |
| Time | nanosleep, clock_gettime, clock_getres, gettimeofday, settimeofday |
| Process Control | sched_yield, sched_setparam, sched_getparam, set_tid_address, futex, sysinfo |
| Process Group | getpgrp, setpgid, getpgid, getsid |
| Resource | getrusage, prlimit64 |
| Misc | ioctl, fcntl, prctl, syslog, getcpu |

### VFS Layer

The VFS layer provides a unified interface for different file systems:

- `VfsInode`: Trait for inode operations (attr, read_at, write_at, open, close)
- `VfsFile`: Trait for open file operations (read, write, seek, close)
- `VfsFilesystem`: Trait for mounted file systems (name, root_inode, sync)
- `FileType`: File type enumeration (RegularFile, Directory, CharDevice, etc.)
- `FileAttr`: File attribute structure with inode information
- `DirEntry`: Directory entry structure

### Sv39 Page Table

Complete Sv39 implementation with:

- 3-level page table structure (512 entries per level)
- PTEFlags: valid, read, write, execute, user, global, accessed, dirty
- VPN/PPN manipulation functions
- Virtual address translation
- COW page detection and breaking
- Kernel page table with identity mapping for 0x80000000+

## Memory Layout

For QEMU virt machine:

- `0x80000000` - DRAM base (physical memory start)
- `0x80200000` - Kernel text start (linked base address)
- `0x80300000` - Kernel end (symbol `end`)
- `0x80400000`+ - Available for physical page allocation and heap

Virtual address space (Sv39):
- 256GB total addressable space (48-bit virtual addresses)
- 4KB pages with 3-level page table
- User space: 0x0000000000000000 - 0x0000003FFFFFFFFF (128GB)
- Kernel space: 0xFFFFFFC000000000+ (upper half)

### Sv39 Address Format

```
63      54 53    45 44    36 35    27 26     18 17     9 8      0
|--------|--------|--------|--------|--------|--------|--------|--------|
   unused    VPN[2]    VPN[1]    VPN[0]   page offset
              |          |         |
              +-----+----+---------+
                    |
              Points to PPN[2] of next level
```

### Page Table Entry Format

```
63      54 53    28 27    19 18    10 9     8 7       1 0
|--------|--------|--------|--------|--------|--------|--------|--------|
   PPN[2]    PPN[1]    PPN[0]   reserved   RSW    flags
```

PTE Flags:
- V (valid), R (read), W (write), X (execute), U (user), G (global)
- A (accessed), D (dirty)

## SMP Architecture

TrainOS supports multi-core processors through:

- **Per-CPU Data**: Each CPU core has its own local data structure (`PerCpu`)
- **HART Management**: Hardware thread detection and management (`Hart` struct)
- **IPI Support**: Inter-processor interrupts for communication (`IpiMsg`)
- **Thread-Local Storage**: Per-core data isolation via hartid indexing

### Per-CPU Structure

```rust
struct PerCpu {
    hartid: usize,        // Hardware thread ID
    current_task: Option<TaskId>,  // Currently running task
    kernel_sp: usize,    // Kernel stack pointer
    user_sp: usize,      // User stack pointer
}
```

### HART States

- **Offline**: Hart is not yet started
- **Running**: Hart is running the kernel
- **Idle**: Hart has no work and is halted

## Network Stack Architecture

TrainOS implements a TCP/IP network stack with the following layers:

### Protocol Stack

```
+-------------------+
|     Socket API    |  (sys_socket, sys_connect, etc.)
+-------------------+
         |
+-------------------+
|  TCP / UDP / ICMP |  (transport layer)
+-------------------+
         |
+-------------------+
|      IPv4         |  (network layer)
+-------------------+
         |
+-------------------+
|    Ethernet       |  (data link layer)
+-------------------+
         |
+-------------------+
|    VirtIO-Net     |  (device driver)
+-------------------+
```

### Network Buffer (NetBuffer)

The `NetBuffer` structure manages packet data with header push/pull operations:

- `push_header()`: Prepend header (used when building outgoing packets)
- `pull_header()`: Remove header (used when parsing incoming packets)
- `header()`: Get header bytes
- `payload()`: Get payload bytes

### Protocol Modules

| Module | File | Description |
|--------|------|-------------|
| Ethernet | `net/eth.rs` | EthHeader, EthFrame, eth_input, eth_output |
| IPv4 | `net/ipv4.rs` | IpHeader, IP fragmentation, routing |
| TCP | `net/tcp.rs` | TcpHeader, TcpState, connection state machine |
| UDP | `net/udp.rs` | UdpHeader, datagram multiplexing |
| ARP | `net/arp.rs` | ArpHeader, ArpCache for MAC resolution |
| DNS | `net/dns.rs` | DnsClient, DNS query/response parsing |

### Default Network Configuration

- **Interface**: eth0
- **MAC Address**: 52:54:00:12:34:56
- **IP Address**: 10.0.2.15
- **Subnet Mask**: 255.255.255.0
- **Gateway**: 10.0.2.1
- **MTU**: 1500 bytes

### Socket Syscalls

TrainOS provides Linux-compatible socket syscalls:

| Syscall | Number | Description |
|---------|--------|-------------|
| socket | 198 | Create socket |
| bind | 203 | Bind to port |
| connect | 208 | Connect to remote |
| listen | 201 | Listen for connections |
| accept | 202 | Accept connection |
| sendto | 206 | Send datagram |
| recvfrom | 207 | Receive datagram |
| shutdown | 210 | Shutdown socket |

## Threading Architecture

TrainOS provides a pthread-like threading interface:

### Thread Management

- `ThreadId`: Unique thread identifier
- `ThreadStatus`: Ready, Running, Blocked, Exited
- `ThreadTable`: Global thread registry (MAX_THREADS = 64)
- `thread_create()`, `thread_exit()`, `thread_join()`, `thread_yield()`

### Synchronization Primitives

| Primitive | Description |
|-----------|-------------|
| `PthreadMutex` | Spin-based mutual exclusion |
| `PthreadCond` | Condition variable for signaling |
| `PthreadBarrier` | Barrier for thread synchronization |
| `PthreadRwLock` | Read-write lock |
| `Once` | One-time initialization |

All primitives use spin-based locking for the no_std environment.

## System Call Interface

The syscall module (`os/src/syscall/`) provides:

- `syscall/mod.rs`: Main syscall dispatcher with Linux-compatible numbers
- `syscall/memory.rs`: Memory-related syscalls (mmap, munmap, mprotect, brk)
- `syscall/task.rs`: Process/thread management structures
- `syscall/fs.rs`: File operations infrastructure

## Development Status

This is an educational project. The current roadmap:

1. **Completed**: Sv39 page table with COW support
2. **Completed**: Full context switching between tasks
3. **Completed**: VFS layer with device file system
4. **Completed**: Extensive Linux syscall implementation
5. **Completed**: TCP/IP network stack with socket API
6. **Completed**: pthread-like threading interface
7. **In Progress**: User mode execution with proper page tables
8. **Planned**: Timer interrupt handling for preemption
9. **Planned**: Disk-based file system (EasyFS)
10. **Planned**: Async runtime for event-driven programming

## Contributing

This is a student project for learning operating system development. Contributions in the form of bug fixes, documentation improvements, and feature implementations are welcome.

## License

This project is for educational purposes.

## Acknowledgments

Built with inspiration from various RISC-V OS tutorials and the RISC-V SBI specification.
