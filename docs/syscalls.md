# TrainOS System Call Reference — V17.0 (81+ syscalls)

## Convention
- `a7` = syscall number
- `a0-a5` = arguments
- `a0` = return value (0 = success, `usize::MAX` = error)
- `ecall` from U-mode traps to S-mode kernel

## Core Syscalls (0-7)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 0 | exit | code: i32 | — | Terminate current process |
| 1 | putchar | char: u8 | — | SBI console output (forwarded to M-mode) |
| 2 | getchar | — | char | SBI console input (forwarded from M-mode) |
| 3 | spawn | elf_ptr, elf_len | pid | Spawn new process from ELF data in user memory |
| 4 | fork | — | child_pid | COW fork: duplicate current process address space |
| 5 | getpid | — | pid | Get current process ID |
| 6 | yield | — | 0 | Yield CPU (stays ready, allows other threads to run) |
| 7 | exec | path_ptr | 0 | Replace current process with ELF loaded from VFS |

## IPC Syscalls (10-14)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 10 | ep_create | — | ep_id | Create a new IPC endpoint |
| 11 | send | ep, opcode:u16, data_ptr, len | 0/err | Send message to endpoint (non-blocking) |
| 12 | recv | ep, buf_ptr, buf_len | (opcode<<24)\|sender | Receive message from endpoint (blocking) |
| 13 | call | ep, opcode, data_ptr, len | result | Send + block for reply (RPC) |
| 14 | reply | ep, data_ptr, len | 0/err | Reply to a pending call |

## MMIO / Shared Memory Syscalls (20-25)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 20 | mmio_map | phys, size | vaddr | Map physical MMIO region into process address space |
| 21 | unmap | vaddr, size | 0 | Unmap virtual address range |
| 22 | map_mmio | phys, size | vaddr | Kernel-mediated MMIO mapping with debug output |
| 23 | mmio_read32 | phys | value | Read 32-bit from physical address (kernel proxy) |
| 24 | mmio_write32 | phys, val | 0 | Write 32-bit to physical address (kernel proxy) |
| 25 | shm_map | target_pid, vaddr | shared_va | Share memory page with another process |

## Capability Syscalls (30-34)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 30 | mint | src_slot, rights:u8 | new_slot | Derive capability with reduced rights |
| 31 | copy | src_slot, dst_pid, dst_slot | 0 | Copy cap to another process's CNode |
| 32 | move | src_slot, dst_pid, dst_slot | 0 | Move cap between CNodes (source becomes Null) |
| 33 | delete | slot | 0 | Delete capability from slot |
| 34 | cap_stats | — | packed | (total:16, used:16, ep:16, mem:16) packed into u64 |

## Block I/O & System Info (40-46)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 40 | blk_read | sector, buf, len | bytes | Read disk sector via VirtIO block device |
| 41 | proclist | buf, buf_len | count | Fill buffer with process info (pid:4, prio:1, state:1 per entry) |
| 42 | kill | pid | 0 | Kill process by PID |
| 43 | meminfo | — | pages | Number of allocated physical pages |
| 44 | perf_stats | — | packed | (sends:20, recvs:20, ctx_sw:24) packed into u64 |
| 45 | blk_write | sector, buf, len | bytes | Write disk sector via VirtIO block device |
| 46 | uptime | — | ticks | System uptime in timer ticks (10ms each) |

## POSIX I/O Syscalls (50-57)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 50 | open | path_ptr, flags, mode | fd | Open file by path (returns fd >= 3, 0-2 = stdio) |
| 51 | read | fd, buf, count | bytes | Read from fd (files→VFS, sockets→IPC, stdin→SBI) |
| 52 | write | fd, buf, count | bytes | Write to fd (files→VFS, stdout/stderr→SBI console) |
| 53 | close | fd | 0 | Close file descriptor |
| 54 | stat | fd, buf_ptr | size | Get file metadata (returns size in bytes) |
| 55 | lseek | fd, offset, whence | new_offset | Seek within file (SEEK_SET=0, SEEK_CUR=1) |
| 56 | dup | fd | new_fd | Duplicate file descriptor |
| 57 | getcwd | buf, size | 0 | Get current working directory (returns "/") |

## User / Permissions (60-64)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 60 | getuid | — | uid | Get user ID (0=root) |
| 61 | setuid | uid | 0 | Set user ID (root only) |
| 62 | chmod | path, mode | 0 | Change file mode (simplified) |
| 63 | signal | sig, handler | 0 | Register signal handler |
| 64 | waitpid | pid, status_ptr, options | child_pid | Wait for child process to exit |

## Process Syscalls (65-71)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 65 | getppid | — | ppid | Get parent process ID |
| 66 | gettid | — | tid | Get thread ID (same as PID for now) |
| 67 | nanosleep | req_ptr, rem_ptr | 0 | Sleep for specified nanoseconds (approximate) |
| 68 | clock_gettime | clk_id, ts_ptr | 0 | Get clock time (0=REALTIME, 1=MONOTONIC) |
| 69 | umask | mask:u16 | old_mask | Set file creation mask |
| 70 | setsid | — | sid | Create new session (returns PID as SID) |
| 71 | sysinfo | buf_ptr | 0 | Fill struct with uptime, loads, memory, proc count |

## Filesystem Syscalls (72-82)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 72 | pipe | fds_ptr | 0 | Create pipe (fds[0]=read, fds[1]=write) |
| 73 | fcntl | fd, cmd, arg | result | File descriptor control (F_DUPFD=0, F_GETFD=1, etc) |
| 74 | ioctl | fd, req, arg | 0 | Device I/O control |
| 75 | getdents64 | fd, buf, len | bytes | Get directory entries from VFS |
| 76 | mkdir | path, mode | 0 | Create directory |
| 77 | rmdir | path | 0 | Remove directory |
| 78 | unlink | path | 0 | Delete file |
| 79 | rename | old_path, new_path | 0 | Rename file (read old→write new→delete old) |
| 80 | chdir | path | 0 | Change working directory |
| 81 | access | path, mode | 0/err | Check file accessibility |
| 82 | truncate | path, length | 0 | Truncate file to length |

## Memory Management (83-86)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 83 | mmap | addr, len, prot, flags, fd, off | mapped_addr | Map anonymous memory pages |
| 84 | munmap | addr, len | 0 | Unmap memory range |
| 85 | mprotect | addr, len, prot | 0 | Change page protection |
| 86 | brk | addr | new_brk | Set program break (0 = query current) |

## Socket Syscalls (90-96)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 90 | socket | domain, type, proto | fd | Create socket (returns endpoint id) |
| 91 | bind | fd, addr, len | 0 | Bind socket to address |
| 92 | listen | fd, backlog | 0 | Listen for connections |
| 93 | accept | fd | new_fd | Accept incoming connection |
| 94 | connect | fd, addr, len | 0 | Connect to remote socket |
| 95 | sendto | fd, buf, len, flags, addr, alen | bytes | Send data to address |
| 96 | recvfrom | fd, buf, len, flags | bytes | Receive data from socket |

## Epoll Syscalls (100-102)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 100 | epoll_create | size | epfd | Create epoll instance |
| 101 | epoll_ctl | epfd, op, fd, events | 0 | Control epoll (1=ADD, 2=DEL, 3=MOD) |
| 102 | epoll_wait | epfd, events, max, timeout | count | Wait for I/O events |

## Namespace Syscalls (110-113)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 110 | unshare | flags | 0 | Disassociate namespace(s) |
| 111 | sethostname | name_ptr, len | 0 | Set hostname in UTS namespace |
| 112 | gethostname | buf_ptr, len | bytes | Get hostname from UTS namespace |
| 113 | setns | fd, nstype | 0 | Reassociate with a namespace |

## CPU Affinity (114-115)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 114 | sched_setaffinity | pid, size, mask_ptr | 0 | Set CPU affinity mask |
| 115 | sched_getaffinity | pid, size, mask_ptr | 0 | Get CPU affinity mask |

## Resource Usage (116-117)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 116 | times | buf_ptr | ticks | Get process times (utime, stime, cutime, cstime) |
| 117 | getrusage | who, buf_ptr | 0 | Get resource usage (0=SELF, 1=CHILDREN) |

## Device Driver (118-120)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 118 | register_drv | name_ptr, type, probe_ep | drv_id | Register device driver |
| 119 | unregister_drv | drv_id | 0 | Unregister device driver |
| 120 | list_drvs | buf, len | bytes | List registered drivers |

## System (121-122)

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 121 | sync | — | 0 | Sync filesystem caches |
| 122 | reboot | magic, cmd | — | Reboot/halt/poweroff (magic=0xfee1dead) |

## Error Codes

All syscalls return `usize::MAX` (0xFFFFFFFF_FFFFFFFF) on error. Specific error information is conveyed through human-readable kernel log messages via `println!()`.

## Priority Inheritance for IPC

The kernel automatically applies priority inheritance: when a high-priority process sends to a low-priority receiver, the receiver inherits the sender's priority until the receive completes.
