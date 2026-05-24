# TrainOS V21–V30 演进路线图

**日期**: 2026-05-18
**状态**: 规划稿
**目标**: 10 个大版本将 TrainOS 从微内核原型推进为生产级操作系统，在架构、安全、性能三个维度全面超越 Linux。

---

## 调研基础

### 顶会论文 (OSDI/SOSP/EuroSys/ASPLOS 2024–2026)

| 论文 | 会议 | 关键思路 | 对 TrainOS 的启示 |
|------|------|---------|-------------------|
| seL4 Verification Extensions | SOSP'25 | 将形式化验证扩展至用户态服务 | V21: 对 IPC 路径进行模型检查 |
| Theseus: Intralingual OS | OSDI'24 | 语言级状态管理，编译时资源追踪 | 利用 Rust 的所有权系统做编译期资源验证 |
| io_uring 可扩展性分析 | EuroSys'24 | 共享 SQ 多核争用优化 | V22: 每 CPU 独立提交队列 |
| CHERI 产业化路线 | ASPLOS'25 | 硬件 capability 与软件 capability 协同 | V27: 软件 capability + 未来硬件迁移路径 |
| eBPF 内核扩展安全 | OSDI'24 | 形式化验证 eBPF 验证器本身 | V24: 沙箱化内核扩展 |
| WASM 在 OS 内核中 | EuroSys'25 | WASM 作为内核扩展安全隔离层 | V28: WASM 运行时作为标准服务 |
| NUMA-Aware Microkernel | SOSP'25 | 微内核在 256 核上的 NUMA 扩展 | V25: 每 NUMA 节点独立就绪队列 |
| AI 工作负载调度 | ASPLOS'26 | GPU kernel 调度、张量内存管理 | V29: AI 加速器支持 |
| Disaggregated Memory | OSDI'25 | 远程内存池化、RDMA 页交换 | V26: 分布式共享内存 |
| Unikernel 复兴 | EuroSys'26 | Library OS 在云原生中的优势 | V28: 单地址空间模式 |

### 开源项目演进

| 项目 | 近期里程碑 | 可借鉴特性 |
|------|-----------|-----------|
| **Redox OS** | 2025: 完整的 Rust 用户态, 自举编译 | 微内核 + Rust 用户态的完整范例 |
| **seL4** | 2025: MCS (Mixed-Criticality Scheduling), 多核验证 | 时间保护、预算调度、形式化验证流程 |
| **Fuchsia** | 2025: Starnix (Linux ABI 兼容), 生产部署 | capability 模型、组件框架 |
| **Linux** | 2025: io_uring 3.0, Btrfs 成熟, Rust 内核模块 | io_uring 设计演进、BPF CO-RE |
| **FreeBSD** | 2025: Capsicum 完整部署, ZFS 增强 | capability 沙箱模型 |
| **Firecracker** | 2025: 快照/恢复、GPU 直通 | 轻量级虚拟化 |
| **Kata Containers** | 2025: 机密计算、TDX/SEV 支持 | 容器 + 虚拟化融合 |
| **WASMtime/WAMR** | 2025: 组件模型、内核级 WASM | WASM 标准化 |
| **Tock OS** | 2025: 生产级嵌入式部署 | Rust 内核的威胁模型 |
| **Asterinas** | 2025: Linux ABI 兼容的 Rust 内核 | Linux 兼容层思路 |

### 主流操作系统演进方向

1. **Linux**: eBPF 可编程性、io_uring 异步 I/O、Rust in kernel、Btrfs/XFS 竞争、cgroups v2/namespaces 成熟
2. **macOS**: M-series 统一内存、Rosetta 2 转译、虚拟化框架
3. **Windows**: WSL2 成熟、VBS/虚拟化安全、Rust 重写核心组件
4. **FreeBSD**: Capsicum sandbox、ZFS、bhyve 虚拟化
5. **Fuchsia/Google**: capability 安全模型、组件化架构、Starnix Linux 兼容

---

## TrainOS V21–V30: 十大版本规划

### 总体架构演进

```
当前 (V20)                     V30 目标
─────────────                  ─────────
微内核 (~3500 LOC)             微内核 (~5000 LOC)
IPC 消息传递                    IPC + 共享内存 + RDMA
64优先级调度器                  NUMA感知 + EEVDF + 实时
Sv39 虚拟内存                  Sv48 + 大页 + 内存池化
能力系统 (软件)                能力系统 (硬件协同CHERI)
POSIX 子集 (83 syscall)        完整 POSIX + Linux ABI 兼容
静态服务                       WASM 运行时 + eBPF 扩展
单机                          分布式 IPC + 远程内存
QEMU/machina 仿真              裸机硬件 + 虚拟化宿主
```

---

## V21 — 形式化基础与安全加固

**主题**: 为微内核建立形式化模型，加固安全边界

### 调研依据
- seL4 MCS 调度验证 (SOSP'25)
- Rust 类型系统在 OS 安全中的应用 (EuroSys'24)
- Theseus 编译期资源追踪

### 具体任务

#### 21.1 内核不变量形式化检查
- [ ] 实现调度器不变量: `priority_bitmap` ↔ `ready_queues` 一致性
- [ ] 实现内存不变量: buddy 空闲列表一致性、引用计数正确性
- [ ] 实现 IPC 不变量: endpoint 等待队列无环路
- [ ] 每一百次定时器中断执行全量不变量检查

#### 21.2 Capability 安全增强
- [ ] `sys_mint` 深度验证: 子 cap 权限 ⊆ 父 cap 权限
- [ ] cap 传递审计日志 (`CAP_AUDIT` 位)
- [ ] cap 泄漏检测: 退出时清理所有 cap
- [ ] 实现 `/proc/cap` (capability 查看)

#### 21.3 内存安全加固
- [ ] 内核堆溢出检测 (canary + 释放后使用检测)
- [ ] 用户-内核缓冲区边界强制检查
- [ ] 页表权限最低化 (W^X 策略)
- [ ] 内核栈溢出保护 (guard page)

#### 21.4 系统调用审计
- [ ] 每个进程可配置的 syscall 过滤器 (seccomp 风格)
- [ ] `/proc/syscalls` 查看系统调用频率
- [ ] 敏感操作审计日志 (kill, mmap, 权限变更)

**验收标准**: 全部不变量检查通过；cap 审计日志可读；W^X 强制执行

---

## V22 — 高性能异步 I/O

**主题**: io_uring 风格的异步 I/O 子系统

### 调研依据
- Linux io_uring 可扩展性研究 (EuroSys'24)
- 微内核零拷贝 I/O 路径 (SOSP'25)
- 共享内存 IPC 优化

### 具体任务

#### 22.1 io_uring 内核实现
- [ ] 每进程提交队列 (SQ) 和完成队列 (CQ) 环形缓冲区
- [ ] `sys_io_uring_setup(entries)` → ring 创建
- [ ] `sys_io_uring_enter(nr, min_complete, flags)` → 提交+收割
- [ ] 支持 IORING_OP_READ/WRITE/OPEN/CLOSE/STAT
- [ ] 内核-用户共享内存的环形缓冲区映射

#### 22.2 零拷贝数据路径
- [ ] 共享内存页面传递 (替换 64 字节 IPC 载荷限制)
- [ ] `VM_SHARED` 映射用于大块数据传输
- [ ] splice/tee 等价操作 (页面转移，无拷贝)

#### 22.3 块设备层优化
- [ ] 请求合并 (相邻扇区批处理)
- [ ] 多队列 blk-mq (每 CPU 提交队列)
- [ ] I/O 调度器框架 (noop/cfq/deadline 可插拔)

**验收标准**: io_uring 读写吞吐量 > 同步读写的 2x；大文件传输延迟 < 同步 IPC 的 50%

---

## V23 — 虚拟化与宿主能力

**主题**: KVM 风格虚拟化，运行客户操作系统

### 调研依据
- Firecracker 微虚拟机设计
- seL4 作为 Hypervisor 的架构
- Kata Containers 机密计算

### 具体任务

#### 23.1 Hypervisor 模式
- [ ] H 扩展 (Hypervisor) CSR 操作
- [ ] 两阶段地址转换 (VS 模式 + G 阶段)
- [ ] VM 创建/销毁/暂停/恢复接口

#### 23.2 虚拟 I/O
- [ ] VirtIO 后端: 虚拟机 ↔ 宿主服务通信
- [ ] 半虚拟化时钟 (PV timer)
- [ ] 半虚拟化中断 (APLIC + IMSIC 仿真)

#### 23.3 轻量级虚拟机
- [ ] 单内核镜像 + 应用级隔离
- [ ] 快照/恢复 (VM 状态序列化)
- [ ] 实时迁移 (内存脏页追踪)

**验收标准**: 能够在 TrainOS 上运行另一个 TrainOS 实例；快照恢复时间 < 100ms

---

## V24 — 可编程内核扩展 (eBPF-like)

**主题**: 安全的用户态内核扩展机制

### 调研依据
- eBPF 验证器形式化 (OSDI'24)
- WASM 作为内核扩展 (EuroSys'25)
- Linux eBPF 演进 (BPF CO-RE, 类型化 BPF)

### 具体任务

#### 24.1 沙箱化扩展框架
- [ ] 扩展注册 (`sys_register_extension`)
- [ ] 字节码验证器 (无后门、无无限循环、无非法访存)
- [ ] 扩展调用接口: `probe_enter`/`probe_exit`/`timer`/`ipc_hook`

#### 24.2 扩展应用
- [ ] 系统调用追踪扩展 (strace 等价)
- [ ] 包过滤扩展 (tcpdump/nftables 等价)
- [ ] 性能监控扩展 (perf 等价)
- [ ] 资源限制扩展 (cgroup 风格)

#### 24.3 安全隔离
- [ ] 扩展独立地址空间 (不能访问内核数据)
- [ ] 扩展时间片限制 (防止 CPU 垄断)
- [ ] 扩展内存配额

**验收标准**: 扩展崩溃不导致内核崩溃；扩展验证器拒绝无效程序；追踪扩展能捕获所有 syscall

---

## V25 — 多核可扩展性 (NUMA + 大规模并行)

**主题**: 在 256+ 核上实现线性扩展的调度和内存管理

### 调研依据
- NUMA-Aware Microkernel (SOSP'25)
- Linux EEVDF 调度器设计
- Barrelfish 多核 OS 架构

### 具体任务

#### 25.1 NUMA 感知调度器
- [ ] 每 NUMA 节点独立就绪队列
- [ ] 任务迁移策略: 负载均衡 + NUMA 亲和性
- [ ] CPU 拓扑发现 (设备树解析)
- [ ] EEVDF (Earliest Eligible Virtual Deadline First) 替代固定优先级

#### 25.2 可扩展同步
- [ ] RCU (Read-Copy-Update) 替代 spinlock 热路径
- [ ] 每 CPU 计数器 (消除共享 cache line 争用)
- [ ] MCS 锁 (避免缓存一致性风暴)

#### 25.3 内存子系统的分片
- [ ] 每 NUMA 节点独立的 buddy 分配器
- [ ] 页面迁移 API
- [ ] 本地页面优先分配策略

**验收标准**: 64 核 IPC 吞吐量 > 单核的 50x；NUMA 本地内存访问延迟 < 远程的 1/5

---

## V26 — 分布式能力

**主题**: 多节点 IPC、远程内存、集群调度

### 调研依据
- Disaggregated Memory (OSDI'25)
- RDMA 在微内核中的应用
- Barrelfish 分布式能力

### 具体任务

#### 26.1 分布式 IPC
- [ ] 节点间端点发现 (`sys_endpoint_publish`/`sys_endpoint_lookup`)
- [ ] 远程消息传递 (RDMA 或 TCP)
- [ ] 分布式 cap 传递 (节点间 mint/copy)

#### 26.2 远程内存池化
- [ ] 远程页面分配 (`sys_remote_alloc`)
- [ ] 页面迁移触发 (访问模式检测)
- [ ] 内存池管理器服务

#### 26.3 集群调度
- [ ] 跨节点负载均衡
- [ ] 进程迁移 (检查点/恢复)
- [ ] 全局 PID 命名空间

**验收标准**: 两节点远程 IPC 延迟 < 本地 IPC 的 10x；远程页面访问功能正常

---

## V27 — 安全深度防御

**主题**: CHERI 硬件协同、ASLR、Stack Canary、完整沙箱

### 调研依据
- CHERI 产业化路线 (ASPLOS'25)
- FreeBSD Capsicum
- Linux Landlock / seccomp

### 具体任务

#### 27.1 软件 CHERI 模拟
- [ ] 128 位 fat pointer 格式 (地址 + 权限 + 边界)
- [ ] 指针算术时的权限检查
- [ ] `/proc/cheri` 状态查看

#### 27.2 ASLR + PIE
- [ ] 内核地址随机化 (KASLR)
- [ ] 用户态 PIE (位置无关可执行文件)
- [ ] 栈随机化 + 堆随机化

#### 27.3 完整沙箱
- [ ] Landlock 风格文件路径沙箱
- [ ] seccomp 风格系统调用过滤
- [ ] 网络沙箱 (per-process net namespace)
- [ ] 用户命名空间隔离 (非 root 用户 mapping)

**验收标准**: CHERI 指针越界访问被拒绝；ASLR 熵 > 30 位；沙箱内进程无法逃逸

---

## V28 — 通用运行时 (WASM/WASI)

**主题**: WebAssembly 作为一等公民的应用程序运行时

### 调研依据
- WASM 组件模型 (WASI Preview 2)
- WASM 作为内核扩展 (EuroSys'25)
- WASM 系统接口标准化

### 具体任务

#### 28.1 WASM 运行时服务
- [ ] 集成 WASM 解释器/JIT (WAMR 或 wasmtime 移植)
- [ ] WASI 系统接口实现: `wasi:io`, `wasi:filesystem`, `wasi:sockets`
- [ ] WASM 组件注册 (`/wasm/` 虚拟文件系统)

#### 28.2 单地址空间执行模式
- [ ] 单地址空间进程 (libOS 模型)
- [ ] 直接函数调用 (无 IPC) 的性能优化
- [ ] 编译时隔离 (Rust borrow checker 保证安全)

#### 28.3 跨语言互操作
- [ ] C ABI 兼容层 (libc 包装)
- [ ] Rust ↔ WASM 互调用
- [ ] Python/JS 运行时 (通过 WASM)

**验收标准**: 能运行 WASI 兼容的 WASM 程序；WASM 程序启动延迟 < 1ms

---

## V29 — AI 原生操作系统

**主题**: GPU/张量加速器管理、AI 工作负载调度

### 调研依据
- AI 工作负载调度 (ASPLOS'26)
- CUDA 驱动的 OS 集成
- 内存-计算融合

### 具体任务

#### 29.1 GPU 驱动框架
- [ ] GPU MMIO 映射 + 命令提交
- [ ] GPU 内存管理 (GART/GTT)
- [ ] 中断处理 (MSI-X)

#### 29.2 AI 工作负载调度
- [ ] GPU 时间片调度
- [ ] 张量内存预取
- [ ] 多进程 GPU 共享 (MPS 等价)

#### 29.3 NPU/TPU 支持
- [ ] 张量加速器驱动接口
- [ ] 模型加载/卸载
- [ ] 推理请求队列

**验收标准**: GPU 驱动可枚举设备；能够提交并完成 CUDA kernel；多进程 GPU 共享无竞态

---

## V30 — 生产就绪与 Linux 超越

**主题**: 完整的 Linux ABI 兼容、生产部署、自举编译

### 调研依据
- Asterinas Linux ABI 兼容框架
- Fuchsia Starnix
- Redox 自举编译

### 具体任务

#### 30.1 完整 POSIX 合规
- [ ] 全部 POSIX.1-2017 系统调用 (>300)
- [ ] 信号量、消息队列、共享内存 IPC
- [ ] 终端控制 (termios)
- [ ] 完整 poll/select/epoll 语义

#### 30.2 Linux ABI 兼容层
- [ ] 系统调用翻译层 (TrainOS syscall ↔ Linux syscall 映射)
- [ ] `/proc` 完整实现 (pid, mount, net, sys 等)
- [ ] `/sys` 设备模型
- [ ] elf64 动态链接器 (ld-linux.so)
- [ ] 运行未修改的 Linux 静态/动态二进制文件

#### 30.3 自举编译
- [ ] Rust 工具链移植 (rustc + cargo 在 TrainOS 上运行)
- [ ] 自编译内核
- [ ] 自编译所有服务

#### 30.4 生产部署
- [ ] 真实硬件启动 (SiFive HiFive, VisionFive 2, K230)
- [ ] 安装程序/磁盘分区
- [ ] systemd 等价的服务管理器
- [ ] 网络配置 (DHCP, DNS)
- [ ] 包管理器 (PKG 完整实现)

**验收标准**: 能运行 `bash`, `gcc`, `python` 未修改的 Linux 二进制文件；自举编译完成；在真实 RISC-V 硬件上启动

---

## 优先级与依赖关系

```
V21 形式化基础 ←─────────────────────────────── 当前 V20 之后的第一优先
  ├─→ V22 异步I/O ← 依赖共享内存基础设施
  ├─→ V23 虚拟化 ← 独立
  ├─→ V24 内核扩展 ← 依赖 V21 的安全基础
  ├─→ V25 NUMA调度 ← 依赖硬件拓扑发现
  ├─→ V26 分布式 ← 独立
  ├─→ V27 安全加固 ← 依赖 V21 + V24
  ├─→ V28 WASM运行时 ← 独立
  ├─→ V29 AI原生 ← 依赖 V23 虚拟化 + V25 NUMA
  └─→ V30 生产就绪 ← 依赖全部
```

三个可并行推进的轨道:
- **轨道 A (安全)**: V21 → V24 → V27
- **轨道 B (性能)**: V22 → V25 → V26
- **轨道 C (生态)**: V23 → V28 → V29

最终在 V30 汇合。

---

## 里程碑时间表

| 版本 | 预计周期 | 关键产出 |
|------|---------|---------|
| V21 | 2 周 | 形式化不变量、seccomp 过滤器 |
| V22 | 2 周 | io_uring 基础实现 |
| V23 | 3 周 | 虚拟机创建/快照 |
| V24 | 2 周 | eBPF 风格扩展框架 |
| V25 | 3 周 | NUMA 调度 + RCU |
| V26 | 3 周 | 分布式 IPC + 远程内存 |
| V27 | 2 周 | ASLR + 沙箱 |
| V28 | 2 周 | WASM/WASI 运行时 |
| V29 | 3 周 | GPU 驱动 + AI 调度 |
| V30 | 4 周 | Linux ABI + 自举 |

---

## 成功指标

| 维度 | V20 当前值 | V30 目标值 | Linux 参考值 |
|------|-----------|-----------|-------------|
| syscall 数量 | 83 | >300 | ~350 |
| 内核 LOC | ~3500 | ~5000 | ~30M |
| IPC 延迟 (最短) | ~100 周期 | <50 周期 | N/A (上下文切换 ~1μs) |
| TCP 吞吐 | 未测试 | >1 Gbps | 40 Gbps |
| 文件系统持久性 | 有 (VFS V3) | 完整 ext2 兼容 | ext4/xfs/btrfs |
| 多核扩展 | 2 核 | 64 核 (线性至 32 核) | ~4000 核 |
| POSIX 合规 | ~30% | >95% | ~98% |
| 安全性 | Capability (软件) | Capability + CHERI + ASLR | SELinux/AppArmor |
| 硬件支持 | machina 仿真 | 3+ 真实 RISC-V 主板 | 数十种架构 |
| 自举 | 否 | 是 | 是 |
