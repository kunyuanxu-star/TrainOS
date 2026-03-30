# TrainOS Development Plan

**Goal: Surpass Linux** — by excelling in four dimensions simultaneously:
1. 内核架构与可扩展性 (Kernel Architecture & Extensibility)
2. 安全与隔离 (Security & Isolation)
3. 性能与并发 (Performance & Concurrency)
4. 开发者体验 (Developer Experience)

---

## Phase 0: 基础稳固 (Foundation Stabilization) — 现状

**当前状态**: 代码框架完整，但核心功能大量 stub，无法实际运行用户程序。

**已实现**:
- Sv39 三级页表，COW 支持
- VFS 层（RAM fs, Dev fs）
- TCP/IP 协议栈（eth/ipv4/tcp/udp/arp/dns）
- VirtIO block/net 驱动
- PCI 总线驱动
- SMP 多核基础设施
- ELF 格式解析
- pthread 风格原语（Mutex, Cond, Barrier, RwLock）

**缺失/Stub**:
- Timer 中断处理（空函数）
- 任务抢占调度（无 timer 触发）
- fork() — 内存不复制，不创建子任务
- execve() — 不加载 ELF 到用户页表
- 用户态返回（无 sret 到用户空间）
- 文件读写（stdin/stdout 以外均返回 -1）
- mmap — 线性地址分配，非 VMA 管理
- futex/waitid — 进程/线程同步

---

## Phase 1: 让系统跑起来 (Make It Runnable)

### 1.1 抢占式调度器 (Preemptive Scheduler)

**目标**: 实现 timer 中断触发任务切换

**任务**:
- [ ] 配置 RISC-V SIE 启用时钟中断
- [ ] 实现 `plic::init()` 和 `timer::init()` — 设置 SIP 寄存器
- [ ] 实现 `trap/mod.rs` 中 `SupervisorTimer` 中断处理
- [ ] 在 timer 中断中调用 `scheduler::schedule()` 触发任务切换
- [ ] 重构 scheduler: 移除固定 64 任务数组，改用 `Vec`（需要 `alloc`）
- [ ] 实现多级反馈队列（MLFQ），取代简单 FIFO
- [ ] 实现 `SCHED_SETAFFINITY` / `GETCPU` per-CPU 就绪队列
- [ ] 实现 `sched_setparam/getparam` 优先级调度
- [ ] 实现 CFS（Completely Fair Scheduler）替代 MLFQ

**文件**:
- `os/src/process/scheduler.rs` — 完全重写
- `os/src/trap/mod.rs` — timer 中断处理
- `os/src/drivers/interrupt.rs` — PLIC 配置

### 1.2 进程创建完整实现 (Working fork/exec)

**目标**: 进程可以真正创建和切换

**任务**:
- [ ] 实现 `sys_clone`: 创建子任务的 TCB + 复制 trap frame
- [ ] 实现 COW fork — 复制父进程页表，设置写保护
- [ ] 实现 `copy_page_table()` — 复制用户页表
- [ ] 实现 `sys_execve`: 解析 ELF，创建新地址空间，加载段
- [ ] 实现 `do_trampoline()` — 从内核返回用户态的入口
- [ ] 实现 `sret` 返回用户空间（设置 sp、pc、satp）
- [ ] 实现 `wait4` / `waitid` — 父进程等待子进程退出
- [ ] 实现 `exit` / `exit_group` — 进程退出，释放资源
- [ ] 实现僵尸进程（Zombie）状态和 `wait` 回收

**文件**:
- `os/src/syscall/task.rs` — clone/execve/exit/wait
- `os/src/process/context.rs` — trap frame 布局和切换
- `os/src/memory/Sv39.rs` — 页表复制

### 1.3 完整系统调用实现 (Complete Syscall Implementation)

**目标**: 基本 POSIX 兼容，用户程序可运行

**任务**:
- [ ] 实现 `sys_read` 从 VFS inode 读取（不只是 stdin）
- [ ] 实现 `sys_write` 到 VFS inode（不只是 stdout）
- [ ] 实现 `openat`/`close` 完整文件操作
- [ ] 实现 `pipe2` — 内核管道缓冲区，不是 stdin/stdout 模拟
- [ ] 实现 `dup2`/`dup3` 完整 fd 复制
- [ ] 实现 `lseek` — 文件偏移
- [ ] 实现 `fstat` — 从 inode 获取文件元数据
- [ ] 实现 `poll`/`select` — I/O 多路复用（阻塞）
- [ ] 实现 `epoll` — 替换 poll 的可扩展版本
- [ ] 实现 `alarm`/`timer_create` —  POSIX 定时器
- [ ] 实现 `nanosleep` — 真正阻塞进程

**文件**:
- `os/src/syscall/fs.rs` — 文件 syscalls
- `os/src/syscall/fd.rs` — fd 表管理
- `os/src/syscall/task.rs` — 时间/同步 syscalls

### 1.4 虚拟内存完整实现 (Complete Virtual Memory)

**目标**: mmap/mprotect/munmap 真正管理 VMA

**任务**:
- [ ] 实现 `VmArea` 结构 — 描述虚拟内存区域
- [ ] 实现 `process.vmareas` — 按地址排序的 VMA 列表
- [ ] 实现 `sys_mmap`: 查找空闲区域，分配物理页，建立页表
- [ ] 实现 `sys_munmap`: 解除映射，回收物理页
- [ ] 实现 `sys_mprotect`: 修改 VMA 权限，更新页表
- [ ] 实现 `sys_madvise`: MADV_DONTNEED、MADV_WILLNEED 等
- [ ] 实现 `sys_mremap`: 扩展/收缩映射
- [ ] 实现 `MAP_SHARED`/`MAP_PRIVATE` 语义
- [ ] 实现 `msync` — 同步内存映射到磁盘

**文件**:
- `os/src/syscall/memory.rs` — 完整重写
- `os/src/process/task.rs` — 添加 VMA 结构

---

## Phase 2: 架构与可扩展性 (Architecture & Extensibility)

### 2.1 微内核架构 (Microkernel Architecture)

**目标**: 将驱动、文件系统作为独立服务，通过 IPC 通信

**任务**:
- [ ] 定义 IPC 通道抽象 (`Channel`, `Endpoint`)
- [ ] 实现 `sys_send` / `sys_recv` — 消息传递 syscall
- [ ] 实现 `sys_call` — 同步 RPC 调用
- [ ] 实现 `Endpoint` 表 — 端口号到处理函数的映射
- [ ] 实现 `CapNamespace` — capability 命名空间隔离
- [ ] 将 VirtIO 驱动移出内核，作为独立驱动服务进程
- [ ] 将文件系统服务（ramfs/ext2）作为独立进程
- [ ] 实现 `driver:fs` 分层 — 驱动服务注册到 FS 服务
- [ ] 实现 `sys_mmap` 与驱动服务的页表交互

**文件**:
- `os/src/ipc/` — 新增 IPC 模块
- `os/src/servers/` — 新增服务器进程

### 2.2 动态模块加载 (Dynamic Module Loading)

**目标**: 运行时加载内核模块

**任务**:
- [ ] 定义模块 ABI (`TrainOS Module Interface`)
- [ ] 实现 `sys_insmod` — 加载 ELF 模块到内核内存
- [ ] 实现 `sys_rmmod` — 卸载模块
- [ ] 实现 `sys_lsmod` — 列出已加载模块
- [ ] 实现模块签名验证（可选：加载时验签）
- [ ] 实现模块依赖解析

**文件**:
- `os/src/module/` — 新增模块系统

### 2.3 可扩展调度器框架 (Extensible Scheduler Framework)

**目标**: 调度器作为可插拔模块

**任务**:
- [ ] 定义 `Scheduler Trait` — `enqueue()`, `dequeue()`, `pick_next()`
- [ ] 实现 `CfsScheduler`, `MlFqScheduler`, `RtScheduler` 替换默认
- [ ] 实现 `sys_sched_setscheduler` 动态切换调度器
- [ ] 实现 `sched_attr` — POSIX 调度属性
- [ ] 实现实时调度类（SCHED_RR, SCHED_FIFO）

**文件**:
- `os/src/sched/` — 调度器框架

---

## Phase 3: 安全与隔离 (Security & Isolation)

### 3.1 Capability 安全模型 (Capability-Based Security)

**目标**: 超越 Linux 的 capability 安全设计

**任务**:
- [ ] 定义 `Cap` — 不可伪造的权限令牌（基于 UUID）
- [ ] 实现 `CapTable` — 每个进程的 capability 表
- [ ] 实现 `Cap::check()` — 权限验证
- [ ] 实现 `sys_capget` / `sys_capset` — Linux-compatible
- [ ] 实现 `sys_cap_enter` — capability 模式切换
- [ ] 实现 `sys_cap_new` — 创建新 capability
- [ ] 实现 capability 继承（fork 时复制，exec 时精简）
- [ ] 实现 `Sealed capability` — 限制 capability 传递
- [ ] 废弃 `uid/gid` 模型（保留兼容），迁移到纯 capability

**文件**:
- `os/src/security/cap.rs` — 新增

### 3.2 Namespace 隔离 (Linux-style Namespaces)

**目标**: 容器化支持

**任务**:
- [ ] 实现 `sys_unshare` — 创建新 namespace
- [ ] 实现 `CLONE_NEWNS` — Mount namespace
- [ ] 实现 `CLONE_NEWPID` — PID namespace
- [ ] 实现 `CLONE_NEWUTS` — UTS namespace (hostname)
- [ ] 实现 `CLONE_NEWIPC` — IPC namespace
- [ ] 实现 `CLONE_NEWNET` — Network namespace
- [ ] 实现 `CLONE_NEWUSER` — User namespace
- [ ] 实现 `/proc/self/ns/` 伪文件系统

**文件**:
- `os/src/ns/` — 新增 namespace 模块

### 3.3 Seccomp 与沙箱 (Seccomp & Sandboxing)

**目标**: 细粒度系统调用过滤

**任务**:
- [ ] 实现 `sys_prctl(PR_SET_SECCOMP, ...)`
- [ ] 实现 `SECCOMP_MODE_STRICT` — 白名单 syscalls
- [ ] 实现 `SECCOMP_MODE_FILTER` — BPF 程序过滤
- [ ] 实现简易 BPF 虚拟机（不用宿主机的 BPF）
- [ ] 实现 `sys_ptrace` 完整支持（父母跟踪子进程）
- [ ] 实现 `landlock` — Linux 5.13+ 的沙箱机制

**文件**:
- `os/src/security/seccomp.rs`
- `os/src/security/bpf.rs`

---

## Phase 4: 性能与并发 (Performance & Concurrency)

### 4.1 细粒度锁与无锁数据结构 (Fine-Grained Locking & Lock-Free)

**目标**: 多核扩展性

**任务**:
- [ ] 审计所有 `spin::Mutex` 使用，识别热点
- [ ] 实现 `PerCpu<T>` — per-CPU 数据结构，无锁读取
- [ ] 实现 `Arc<Thread>` — 原子引用计数的线程
- [ ] 实现 `MpscQueue<T>` — 多生产者单消费者无锁队列（调度器用）
- [ ] 实现 `RwLock<T>` 细粒度版本（写优先/读优先可选）
- [ ] 实现 `Seqlock<T>` — 读多写少场景
- [ ] 实现 `AtomicSlice` — 原子操作切片
- [ ] 移除 `global_asm!` 中的全局锁依赖

**文件**:
- `os/src/sync/` — 新增同步原语

### 4.2 io_uring 风格异步 I/O (io_uring-Style Async I/O)

**目标**: 超越 epoll 的 I/O 模型

**任务**:
- [ ] 实现 `io_uring` 环形缓冲区结构
- [ ] 实现 `sys_io_uring_setup` — 创建 ring
- [ ] 实现 `sys_io_uring_enter` — 提交/收集操作
- [ ] 实现 `IORING_OP_READV` / `IORING_OP_WRITEV`
- [ ] 实现 `IORING_OP_OPENAT` / `IORING_OP_CLOSE`
- [ ] 实现零拷贝 — `iov` 直接 DMA
- [ ] 实现 `IORING_FEAT_NODROP` — 不丢请求
- [ ] 实现 `poll_table` — 块设备轮询

**文件**:
- `os/src/io/uring.rs` — 新增

### 4.3 实时调度 (Real-Time Scheduling)

**目标**: 软实时支持

**任务**:
- [ ] 实现 `SCHED_FIFO` — 先进先出实时调度
- [ ] 实现 `SCHED_RR` — 时间片轮转实时调度
- [ ] 实现 `SCHED_DEADLINE` — EDF 最早截止期优先
- [ ] 实现 `prio` 属性 — 用户可指定优先级
- [ ] 实现抢占式调度 — 高优先级任务可抢占低优先级
- [ ] 实现 latency 分析工具

**文件**:
- `os/src/sched/rt.rs`

### 4.4 块设备层优化 (Block Layer Optimization)

**目标**: 高性能存储 I/O

**任务**:
- [ ] 实现请求合并（merge adjacent requests）
- [ ] 实现 blk-mq（多队列）— 每 CPU 队列
- [ ] 实现 `requestplugging` — 批处理合并
- [ ] 实现 `elevator` — 调度算法（cfq/deadline/noop）
- [ ] 实现 `DIO`（直接 I/O）绕过页缓存

**文件**:
- `os/src/block/` — 新增块设备层

---

## Phase 5: 开发者体验 (Developer Experience)

### 5.1 完整测试框架 (Testing Framework)

**目标**: 内置内核测试

**任务**:
- [ ] 实现 `ktest!` 宏 — 内核内单元测试
- [ ] 实现 `ktest_case!` — 可配置的测试用例
- [ ] 实现 `cargo ktest` — 运行内核测试（QEMU 内或仿真）
- [ ] 实现内存分配器测试（buddy/slab 分配正确性）
- [ ] 实现 VFS 压力测试（并发 open/close/rename）
- [ ] 实现调度器公平性测试（CFS 权重验证）
- [ ] 实现网络栈测试（TCP 连接/数据传输）
- [ ] 实现 Syzkaller-style 模糊测试框架

**文件**:
- `os/src/test/` — 新增测试框架

### 5.2 Profiling 与 Tracing

**目标**: 性能分析工具

**任务**:
- [ ] 实现 `oprofile` 兼容的采样分析
- [ ] 实现 `perf` 子集 — `perf stat`, `perf record`
- [ ] 实现 `ftrace` — 函数级追踪（静态插桩）
- [ ] 实现动态追踪 (`sys_enter`, `sys_exit` probes)
- [ ] 实现 `trace_printk` — 内核日志到 trace buffer
- [ ] 实现 `/sys/kernel/tracing/` 接口（Linux 兼容）
- [ ] 实现内存分配追踪（slub 分配/释放 trace）

**文件**:
- `os/src/profiling/` — 新增

### 5.3 Panic 与错误处理增强 (Panic & Error Handling)

**目标**: 最友好的开发者错误信息

**任务**:
- [ ] 实现 `backtrace` — 捕获并打印调用栈
- [ ] 实现 `panic_info` — 包含寄存器、页表、当前任务信息
- [ ] 实现 `panic_handler` 发送到调试终端（而不只是 SBI putchar）
- [ ] 实现 `assert!` 宏带上下文（文件和行 + 条件值）
- [ ] 实现 Oops 信息（类似 Linux 的 kernel panic）
- [ ] 实现 `/proc/kmsg` — 内核日志环形缓冲区
- [ ] 实现 `dmesg` syscall 读取内核日志

**文件**:
- `os/src/panic.rs` — 增强
- `os/src/console.rs` — 添加日志级别

### 5.4 调试器支持 (Debugger Support)

**目标**: GDB 调试内核和用户程序

**任务**:
- [ ] 实现 `sys_ptrace(PTRACE_TRACEME)` — 允许父进程跟踪
- [ ] 实现 `sys_ptrace(PTRACE_GETREGS, ...)` — 读取寄存器
- [ ] 实现 `sys_ptrace(PTRACE_SETREGS, ...)` — 写寄存器
- [ ] 实现 `sys_ptrace(PTRACE_PEEKTEXT, ...)` — 读内存
- [ ] 实现 `sys_ptrace(PTRACE_POKETEXT, ...)` — 写内存
- [ ] 实现 `sys_ptrace(PTRACE_CONT, ...)` — 继续执行
- [ ] 实现 `sys_ptrace(PTRACE_SINGLESTEP, ...)` — 单步
- [ ] 配置 QEMU+GDB 调试脚本

**文件**:
- `os/src/debug.rs` — 新增

---

## Phase 6: 文件系统完整实现 (Complete Filesystem)

### 6.1 磁盘文件系统 (Disk Filesystem)

**目标**: 支持持久化存储

**任务**:
- [ ] 完成 `easyfs.rs` — 实现 ext2 的简化版本
- [ ] 实现 ext2 目录项解析
- [ ] 实现 ext2 inode 分配（bitmap）
- [ ] 实现 ext2 数据块分配（bitmap）
- [ ] 实现 ext2 日志（可选，Phase 7）
- [ ] 实现 `mount` syscall — 挂载文件系统
- [ ] 实现 `statfs` / `fstatfs` — 文件系统统计
- [ ] 实现 `umount2` — 卸载文件系统

**文件**:
- `os/src/fs/easyfs.rs` — 完整 ext2
- `os/src/syscall/fs.rs` — mount 相关

### 6.2 虚拟文件系统增强 (VFS Enhancements)

**目标**: 完整的 VFS 层

**任务**:
- [ ] 实现 `procfs` — `/proc` 伪文件系统（进程信息）
- [ ] 实现 `sysfs` — `/sys` 伪文件系统（内核对象）
- [ ] 实现 `tmpfs` — 内存文件系统，支持 `mmap`
- [ ] 实现 `devpts` — `/dev/pts` 伪终端
- [ ] 实现 `bind mount` — 绑定挂载
- [ ] 实现 `overlayfs` — 联合文件系统
- [ ] 实现 `FUSE` — 用户空间文件系统接口

**文件**:
- `os/src/fs/procfs.rs`
- `os/src/fs/sysfs.rs`
- `os/src/fs/tmpfs.rs`
- `os/src/fs/overlayfs.rs`

---

## Phase 7: 网络增强 (Network Enhancements)

### 7.1 Socket API 完整实现

**任务**:
- [ ] 实现 `socketpair` — 完整
- [ ] 实现 `getsockname` / `getpeername`
- [ ] 实现 `shutdown(SHUT_RDWR)` — 关闭读写
- [ ] 实现 `unix domain socket` — 本地套接字
- [ ] 实现 `epoll` 监听 socket 事件

### 7.2 TCP 可靠性增强

**任务**:
- [ ] 实现 TCP SACK（Selective Acknowledgment）
- [ ] 实现 TCP TFO（TCP Fast Open）
- [ ] 实现 TCP BBR 拥塞控制
- [ ] 实现 TCP keepalive
- [ ] 实现 `SO_KEEPALIVE` socket 选项

### 7.3 网络安全

**任务**:
- [ ] 实现 `AF_PACKET` 原始套接字
- [ ] 实现 `AF_NETLINK` — 内核/用户通信
- [ ] 实现基础 netfilter 框架（iptables 风格）

---

## Phase 8: 迈向生产 (Production Readiness)

### 8.1 电源管理 (Power Management)

**任务**:
- [ ] 实现 ACPI 解析（简化版）
- [ ] 实现 S3 睡眠状态
- [ ] 实现 CPU 空闲状态（C-states）
- [ ] 实现 CPU 频率调节（DVFS）

### 8.2 容器运行时 (Container Runtime)

**任务**:
- [ ] 实现 `runc` 兼容的容器启动
- [ ] 实现 `cgroups` v2 — 资源限制
- [ ] 实现 overlay 文件系统（容器根文件系统）
- [ ] 实现网络 namespace 网络配置

### 8.3 完整性验证

**任务**:
- [ ] 实现 dm-verity — 块设备完整性
- [ ] 实现 IMA/EVM — 文件完整性测量
- [ ] 实现安全启动（Secure Boot）链

---

## 优先级与依赖关系

```
Phase 1 (让系统跑起来)
├── 1.1 抢占式调度 ← 依赖 trap/timer
├── 1.2 进程创建 ← 依赖 1.1
├── 1.3 完整 syscalls ← 依赖 1.2
└── 1.4 虚拟内存 ← 依赖 1.2

Phase 2 (架构)
├── 2.1 微内核 ← 依赖 Phase 1 全部
├── 2.2 模块加载 ← 依赖 2.1
└── 2.3 调度框架 ← 依赖 1.1

Phase 3 (安全)
├── 3.1 Capability ← 依赖 2.1
├── 3.2 Namespace ← 依赖 2.1
└── 3.3 Seccomp ← 依赖 Phase 1

Phase 4 (性能)
├── 4.1 细粒度锁 ← 依赖 Phase 1
├── 4.2 io_uring ← 依赖 Phase 1 + 4.1
└── 4.3 实时调度 ← 依赖 2.3

Phase 5 (开发者体验) ← 可并行

Phase 6 (文件系统)
├── 6.1 磁盘 FS ← 依赖 Phase 1
└── 6.2 VFS 增强 ← 依赖 6.1

Phase 7 (网络) ← 可部分并行

Phase 8 (生产)
```

---

## 立即行动 (Immediate Actions)

**第一步**: 完成 Phase 1.1 和 1.2，使系统能够真正创建进程并切换。

这是整个项目的基础——没有可工作的进程调度，其他一切都是空谈。

**关键文件需要修改**:
1. `os/src/trap/mod.rs` — 添加 timer 中断处理
2. `os/src/process/scheduler.rs` — 实现真正的任务切换
3. `os/src/syscall/task.rs` — 实现 fork/exec 完整逻辑
4. `os/src/process/context.rs` — 完善 trap frame 和 context switch
