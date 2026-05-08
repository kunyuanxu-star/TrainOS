# TrainOS System Call Reference

## Convention
- `a7` = syscall number
- `a0-a5` = arguments
- `a0` = return value (0 = success, usize::MAX = error)

## System Calls

| # | Name | Args | Returns | Description |
|---|------|------|---------|-------------|
| 0 | exit | code:i32 | - | Terminate process |
| 1 | putchar | char:u8 | - | SBI console output (forwarded) |
| 2 | getchar | - | char | SBI console input (forwarded) |
| 3 | spawn | elf_ptr, elf_len | pid | Spawn process from ELF |
| 4 | fork | - | child_pid | COW fork current process |
| 5 | getpid | - | pid | Get current PID |
| 6 | yield | - | 0 | Yield CPU (stays ready) |
| 10 | ep_create | - | ep_id | Create IPC endpoint |
| 11 | send | ep, opcode, data_ptr, len | 0/error | Send message |
| 12 | recv | ep, buf_ptr, buf_len | sender+opcode | Receive message (blocking) |
| 20 | mmio_map | phys, size | vaddr | Map MMIO into process PT |
| 21 | mmio_read32 | phys | value | Read 32-bit MMIO (kernel proxy) |
| 22 | mmio_write32 | phys, val | 0 | Write 32-bit MMIO (kernel proxy) |
| 30 | mint | src_idx, rights | slot_idx | Derive capability |
| 31 | copy | src_idx, dst_pid, dst_idx | 0 | Copy cap to another process |
| 32 | move | src_idx, dst_pid, dst_idx | 0 | Move cap to another process |
| 33 | delete | slot_idx | 0 | Delete capability |
| 34 | cap_stats | - | packed_stats | Query cap counts |
| 40 | proclist | buf_ptr, buf_len | count | List processes |
| 41 | kill | pid | 0 | Kill process |
| 42 | meminfo | - | pages | Allocated page count |
| 43 | perf_stats | - | packed_stats | IPC/ctx_sw counters |
| 44 | uptime | - | ticks | System uptime in ticks |
| 45 | blk_write | sector, buf, len | bytes | Write block via VirtIO |
| 46 | blk_read | sector, buf, len | bytes | Read block via VirtIO |
| 50 | open | path, flags, mode | fd | POSIX open |
| 51 | read | fd, buf, count | bytes | POSIX read |
| 52 | write | fd, buf, count | bytes | POSIX write |
| 53 | close | fd | 0 | POSIX close |
