# TrainOS Architecture Guide — V17.0

## Microkernel Design

TrainOS follows the minimalist microkernel philosophy. The kernel provides exactly four mechanisms and nothing more:

1. **Capability System** — Access control tokens (CNode, Mint/Copy/Move/Revoke/Delete)
2. **IPC Router** — Synchronous message passing (endpoints, 64-byte payload, cap transfer)
3. **Scheduler** — 64 priority levels, bitmap O(1), SMP-aware spinlock, priority inheritance
4. **Memory Manager** — Buddy allocator, Sv39 page tables, COW fork, mmap/brk

All other functionality (filesystem, network, device drivers, shell, POSIX) runs as user-space services communicating via IPC.

## Boot Sequence

1. machina loads RustSBI at 0x80000000
2. RustSBI loads kernel ELF at 0x80200000, jumps to `_start` in S-mode
3. `_start`: clear BSS, set per-HART stack, call `rust_main`
4. `rust_main`: clear BSS, init memory (buddy + Sv39 + heap), init CLINT/trap, init cap/ipc, enable MMU, spawn embedded services, start scheduler

## Memory Layout

- 0x80000000-0x80020000: RustSBI firmware
- 0x80020000-0x80200000: Device tree / reserved
- 0x80200000-0x80800000: Kernel (text, rodata, data, bss, heap)
- 0x80800000-0x88000000: User pages (buddy-managed, 128MB total)

## Virtual Memory (Sv39, per-process)

```
User space (VPN2 0..255, low 256GB):
  0x00000000_0000 - 0x00000040_0000   user program (text, rodata, data, bss)
  0x00000040_0000 - 0x3FFFFFFF_FFFF   heap (brk), mmap regions, shared memory

Kernel space (VPN2 256..511, high 256GB):
  0xFFFFFFC0_0000_0000 - end          identity-mapped physical DRAM
```

## Syscall Convention

- `a7` = syscall number
- `a0-a5` = arguments
- `a0` = return value (0 = success, `usize::MAX` = error)
- ecall from U-mode traps to S-mode kernel
- 83 syscalls total (see [syscalls.md](syscalls.md))

## IPC Protocol

### Well-Known Endpoints

| EP | Service | Purpose |
|----|---------|---------|
| 1 | init | System init, IPC receiver |
| 2 | fs (VFS) | File system service with procfs |
| 3 | net | Network stack (port-based datagram routing) |
| dynamic | tcp | TCP reliable stream protocol |

### Message Format
64-byte payload + optional cap transfers. Short messages only.

### VFS Operations (via EP 2)

| Opcode | Name | Format |
|--------|------|--------|
| 2 | READ | `[reply_ep:2][path_len:1][path:N]` → `[data:N]` |
| 3 | WRITE | `[reply_ep:2][path_len:1][path:N][data_len:1][data:N]` → `OK` |
| 4 | APPEND | `[reply_ep:2][path_len:1][path:N][data_len:1][data:N]` → `OK` |
| 5 | DELETE | `[reply_ep:2][path_len:1][path:N]` → `OK` |
| 6 | LIST | `[reply_ep:2][path_len:1][path:N]` → `[entries:N]` |
| 7 | STAT | `[reply_ep:2][path_len:1][path:N]` → `[size:1, is_dir:1]` |

### /proc Virtual Files
- `/proc/uptime` — system uptime in milliseconds
- `/proc/meminfo` — memory allocation statistics
- `/proc/perf` — IPC/context-switch performance counters
- `/proc/version` — TrainOS version string
- `/proc/proc` — process listing
- `/proc/self` — current process ID

### NET Operations (via EP 3)

| Opcode | Name | Format |
|--------|------|--------|
| 1 | REGISTER | `[port:2, listener_ep:2]` |
| 2 | SEND | `[port:2, len:1, data:N]` → routed to listener |

### TCP Operations (via TCP endpoint)

| Opcode | Name | Description |
|--------|------|-------------|
| 1 | LISTEN(port) | Start listening on a TCP port |
| 2 | CONNECT(port) | Connect to remote port (returns conn_id) |
| 3 | SEND(conn_id, data) | Send data on established connection |
| 4 | RECV(conn_id) | Receive data (blocks until available) |
| 5 | CLOSE(conn_id) | Close connection (FIN handshake) |

### TCP Internal Protocol (between TCP service and NET)

| Opcode | Name | Description |
|--------|------|-------------|
| 0x10 | SYN | Connection request (port + seq) |
| 0x11 | SYN-ACK | Connection acknowledgment |
| 0x12 | ACK | Data acknowledgment with window |
| 0x13 | DATA | Data segment with sequence number |
| 0x14 | FIN | Connection close request |
| 0x15 | FIN-ACK | Close acknowledgment |

## Process Model

- Single-threaded per process (v1)
- Per-process page table root (Sv39)
- Per-process capability space (CNode)
- Per-process fd table (64 slots)
- UID/GID with simple permission model
- UTS/PID namespace isolation

## Scheduler

- 64 priority levels (0=idle, 63=highest)
- Bitmap O(1) find-highest-nonempty
- FIFO within same priority level
- Priority inheritance for IPC senders/receivers
- SMP-aware: per-HART pick counts, IPI for remote wakeup
- Timer interrupt: 10ms tick via CLINT
- User time accounting per tick, system time on syscall entry

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| 64 priority levels | Fits in u64 bitmap for O(1) scheduling |
| 10ms time slice | Standard choice: responsiveness vs context-switch |
| 64-byte IPC payload | Fits in cache line; larger transfers via shared memory |
| Sv39 (not Sv48) | Universal RISC-V support; upgrade path designed in |
| Buddy allocator | Good fragmentation/performance for 128MB |
| Embedded service binaries | Avoids filesystem dependency at boot |
| Single-threaded v1 | Simpler scheduler and IPC; add multi-threading later |
