# TrainOS 微内核架构设计

**版本**: 1.0
**日期**: 2026-04-09
**状态**: 已批准

---

## 一、设计目标

构建一个**微内核操作系统**，所有驱动、文件系统、网络栈都是独立的用户空间服务进程。内核只保留最核心的功能：

- 进程调度
- 物理内存管理
- 虚拟内存管理（Sv39）
- IPC 通道机制
- Capability 安全验证
- 系统调用入口

---

## 二、架构概览

```
┌─────────────────────────────────────────────────────────────┐
│                         用户空间                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │init 服务 │  │  FS 服务  │  │驱动服务   │  │网络服务   │  │
│  │ (pid=1) │  │ (pid=2)  │  │ (pid=3)  │  │ (pid=4)  │  │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘  │
│       │              │              │              │         │
│       └──────────────┴──────────────┴──────────────┘         │
│                           │ IPC 通道                         │
└───────────────────────────┼───────────────────────────────────┘
                            │  syscall: send/recv/call
┌───────────────────────────┼───────────────────────────────────┐
│                     内核空间                                    │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐            │
│  │  调度器    │  │  内存管理   │  │   IPC      │            │
│  │ Scheduler  │  │   Sv39     │  │  Channel   │            │
│  └────────────┘  └────────────┘  └────────────┘            │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐            │
│  │  Trap 处理 │  │  Capability│  │  系统调用  │            │
│  │  Interrupt │  │   Cap      │  │  Syscall   │            │
│  └────────────┘  └────────────┘  └────────────┘            │
└───────────────────────────────────────────────────────────────┘
```

---

## 三、服务进程设计

### 3.1 服务列表

| 服务 | PID | 职责 | 直接访问硬件 |
|------|-----|------|-------------|
| Init | 1 | 系统启动，spawn 其他服务，进程树管理 | 否 |
| Driver | 2 | VirtIO-Block, VirtIO-Net 驱动 | 是（MMIO） |
| FS | 3 | 文件系统操作（ramfs） | 否（通过 Driver） |
| Shell | 4 | 用户命令行界面 | 否（通过 FS） |

### 3.2 服务启动顺序

```
内核启动
    │
    ▼
start_kernel() → 创建 init 进程 (pid=1)
    │
    ▼
init 进程执行 /bin/driver_server
    │
    ▼
init 进程执行 /bin/fs_server
    │
    ▼
init 进程执行 /bin/shell
    │
    ▼
进入交互式使用
```

---

## 四、IPC 机制

### 4.1 通道抽象

```rust
// 端点 - 代表一个通信目标
struct Endpoint {
    pid: Pid,           // 目标进程 ID
    port: PortId,       // 端口号
}

// 能力 - 端点的权限包装
struct Cap {
    id: Uuid,
    endpoint: Endpoint,
    rights: CapRights,
}

// 消息
struct IpcMessage {
    from: Pid,
    to: Pid,
    port: PortId,
    data: Vec<u8>,
    reply_port: Option<PortId>,
}
```

### 4.2 系统调用接口

| syscall | 参数 | 返回 | 描述 |
|---------|------|------|------|
| sys_endpoint_create | - | (PortId, Cap) | 创建新端点，返回端口号和 capability |
| sys_send | pid, port, data | 0/-E | 发送消息（非阻塞） |
| sys_recv | port, buf | size/-E | 接收消息（阻塞直到有消息） |
| sys_call | pid, port, data | response/-E | 同步 RPC（send + recv） |
| sys_cap_grant | cap, target_pid | 0/-E | 将 capability 授予其他进程 |
| sys_cap_revoke | cap_id | 0/-E | 撤销 capability |

### 4.3 消息格式

```
+----------------+----------------+----------------+
| Header (16B)  | Data (可变)    |                |
+----------------+----------------+----------------+
| from: u32      |                |                |
| to: u32        |  payload      |  ...           |
| port: u32     |                |                |
| size: u32     |                |                |
| reply_port: u32|               |                |
+----------------+----------------+----------------+
```

---

## 五、Capability 安全模型

### 5.1 Cap 结构

```rust
struct Cap {
    id: Uuid,              // 不可伪造的唯一标识符
    port: PortId,          // 关联的端点
    rights: CapRights,     // 权限位掩码
}

bitflags! {
    struct CapRights: u32 {
        const READ    = 1 << 0;  // 可读取
        const WRITE   = 1 << 1;  // 可写入
        const EXECUTE = 1 << 2;  // 可执行
        const GRANT   = 1 << 3;  // 可授予他人
    }
}
```

### 5.2 权限验证

内核在每次 IPC 操作前验证：
1. 发送方是否持有指向目标端点的 capability
2. 该 capability 是否包含 SEND_RIGHT（对于 sys_send）或 RECV_RIGHT（对于 sys_recv）

### 5.3 能力继承

- **fork()**: 子进程继承父进程的所有 capability
- **exec()**: 保持 capability 不变
- **exit()**: 释放所有持有的 capability

---

## 六、RISC-V 特定设计

### 6.1 PMP 区域划分

| 区域 | 大小 | 属性 | 用途 |
|------|------|------|------|
| 0x80000000-0x80200000 | 2MB | RWX | 内核代码+数据 |
| 0x80400000-0x80800000 | 4MB | RWX | 服务进程（共享物理页表区域） |
| 0x10000000-0x10010000 | 64KB | RW | VirtIO-MMIO（驱动直接访问） |
| 用户空间 | - | RWX per-process | 每个服务独立地址空间 |

### 6.2 驱动服务直接 MMIO 访问

VirtIO-MMIO 寄存器在 0x10000000 开始。Driver 服务进程启动时，内核：
1. 将 VirtIO-MMIO 区域加入进程的 PMP
2. 分配单独的页表用于访问该区域
3. 驱动服务通过直接 MMIO 访问设备，无需内核中转

### 6.3 定时器中断处理

定时器中断始终由内核处理：
1. CLINT 在 0x02000000（QEMU virt）
2. 内核设置 sie.STIE 启用定时器中断
3. 定时器到期 → trap → 内核调度器选择下一个进程
4. 通过 IPI（如果需要）唤醒其他 CPU

---

## 七、内存管理

### 7.1 物理内存布局

```
0x80000000 +------------------+
           |    内核空间      |
0x80400000 +------------------+
           |   服务进程堆     |
0x80800000 +------------------+
           |   页表池         |
0x80C00000 +------------------+
           |   空闲           |
           |                  |
0x88000000 +------------------+  <- 最大物理内存（假设128MB）
```

### 7.2 虚拟地址空间

**内核空间**（高地址）：
```
0xFFFFFFC000000000 +------------------+
                   |    内核代码       |
                   |    内核堆        |
                   |    进程结构      |
                   +------------------+
```

**用户空间**（每个服务进程独立）：
```
0x0000000000000000 +------------------+
                   |    代码段         |
                   |    数据段         |
                   |    堆            |
                   |    栈            |
                   +------------------+
```

---

## 八、进程管理

### 8.1 进程创建

```rust
struct Process {
    pid: Pid,
    page_table: PhysicalAddr,  // Sv39 页表物理地址
    capability_table: Vec<Cap>,
    state: ProcessState,
    mailbox: Vec<IpcMessage>,  // 消息队列
}
```

### 8.2 init 进程职责

1. 创建 Driver 服务（通过 sys_spawn）
2. 等待 Driver 服务就绪（IPC 握手）
3. 创建 FS 服务，传递 Driver 服务的 capability
4. 等待 FS 服务就绪
5. 创建 Shell 服务，传递 FS 服务的 capability
6. 进入循环：等待 Shell 退出，尝试重启

---

## 九、文件系统的服务化

### 9.1 FS 服务接口

```rust
// FS 服务处理的端口
const FS_PORT: PortId = 1;

// 文件操作请求
enum FsRequest {
    Open { path: String, flags: u32 },
    Read { fd: u32, buf: Vec<u8>>,
    Write { fd: u32, data: Vec<u8>>,
    Close { fd: u32 },
    Stat { path: String },
}

// 文件操作响应
enum FsResponse {
    Ok { data: Vec<u8> },
    Err { errno: i32 },
}
```

### 9.2 驱动服务接口

```rust
// 驱动服务处理的端口
const DRIVER_PORT: PortId = 2;

// 块设备请求
enum DriverRequest {
    BlockRead { sector: u64, buf: Vec<u8>>,
    BlockWrite { sector: u64, data: Vec<u8>>,
    NetSend { data: Vec<u8>>,
    NetRecv,
}
```

---

## 十、实现阶段

### Phase 1: 微内核基础（当前）
1. 重构内核，移除驱动/FS 代码
2. 实现 IPC 通道机制（sys_send/recv）
3. 实现 init 进程
4. 实现 Driver 服务（VirtIO-Block 访问）
5. 实现 FS 服务（基于 RAM FS）

### Phase 2: 调度与内存
1. 实现抢占式调度器
2. 实现完整的进程生命周期管理
3. 实现 Capability 继承（fork/exec）

### Phase 3: 扩展服务
1. 实现 VirtIO-Net 驱动服务
2. 实现网络协议栈服务
3. 实现 procfs/sysfs 服务

### Phase 4: 安全与隔离
1. 实现完整的 Capability 检查
2. 实现 Namespace 隔离
3. 实现 Seccomp/BPF 过滤

---

## 十一、设计决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 架构 | 微内核 | 安全性、可靠性、灵活性 |
| IPC 同步方式 | 消息传递 + 阻塞 recv | 简单、明确 |
| 驱动位置 | 用户空间服务 | 内核只做调度和内存管理 |
| 定时器 | 内核处理 | 必须在内核处理才能调度 |
| Capability 传递 | 显式 grant | 防止意外权限扩散 |

---

## 十二、已批准

- 2026-04-09: 用户批准初始设计方案
