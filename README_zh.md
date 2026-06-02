# TrainOS

[English](README.md) | [中文](README_zh.md)

一个使用 Rust 编写的 RISC-V 64 位 (rv64gc) 微内核操作系统。运行于 RustSBI 固件之上，在 QEMU 虚拟机中执行。

**目标**：在内核架构、安全性和性能方面全面超越 Linux — 完全由 AI 设计与实现。

**当前版本**: V38.0 | **系统调用**: 362+ | **内核规模**: ~20,000 行 | **许可证**: MIT

---

## 架构

TrainOS 是一个**微内核** — 内核机制极度精简，一切皆用户态服务：

| 子系统 | 能力 |
|--------|------|
| **能力系统** | CNode 能力节点，支持铸造/复制/移动/撤销/删除，父权限校验，审计日志 |
| **IPC 通信** | 同步消息传递，优先级继承，跨节点分布式 IPC |
| **调度器** | NUMA 感知，EEVDF 截止时间调度，64 优先级每节点队列，SMP |
| **内存管理** | 伙伴分配器，Sv39/Sv48 页表，事务化 MMU，写时复制 fork，W^X 强制执行 |
| **安全系统** | seccomp 过滤器，CHERI 软件能力，ASLR/KASLR，PMP/ePMP TEE 飞地 |
| **虚拟化** | RISC-V H 扩展，二级地址转换，VM 生命周期管理，VirtIO 后端 |

---

## 功能矩阵

### 内核服务

| 类别 | 功能 |
|------|------|
| **进程** | spawn、fork(COW)、exec、exit、kill、waitpid、signal、prctl、priority |
| **内存** | mmap、munmap、mprotect、brk、shm_map、madvise、mincore、mlock、mseal、mTHP |
| **文件系统** | open、read、write、close、stat、lseek、dup、getcwd、symlink、readlink、fsync、flock、fallocate、sendfile、ioctl(termios) |
| **Socket** | socket、bind、listen、accept、connect、sendto、recvfrom、getsockopt、setsockopt、shutdown |
| **IPC** | ep_create、send、recv、call、reply + System V 信号量、消息队列 |
| **时间** | nanosleep、clock_gettime、gettimeofday、settimeofday、POSIX 定时器 |
| **Poll** | poll、ppoll、pselect6、epoll_create/ctl/wait |
| **I/O** | io_uring 零拷贝、RWF_UNCACHED/ATOMIC、cachestat、per-CPU blk-mq |

### 高级子系统

| 子系统 | 说明 |
|--------|------|
| **io_uring** | 异步 I/O，每进程 SQ/CQ 环形缓冲区，共享内存映射，零拷贝 splice |
| **eBPF 扩展** | 沙箱化字节码验证器，12 指令解释器，4 类钩子，eBPF+WASM 混合架构 |
| **WASM 运行时** | 36 指令解释器，WASI preview2（21 个宿主函数），系统调用即宿主函数（55 映射） |
| **NUMA** | 每节点就绪队列，EEVDF，负载均衡，每 CPU 计数器，MCS 锁，RCU |
| **分布式 IPC** | 节点发现，远程消息传递，分布式能力传递，远程内存池化 |
| **GPU/AI** | GPU 驱动，AI 工作负载调度器（MPS），张量运算，模型注册，P/D 分离，KV-cache |
| **虚拟化** | RISC-V H 扩展 CSR，G 级 MMU，VM 生命周期，VirtIO 后端，PV 定时器，VS-AIA，快照 |
| **TEE** | AP-TEE 兼容飞地，RATS 远程证明，多区域隔离，安全存储 |
| **GUI** | 帧缓冲驱动，窗口管理器（32 窗口），组件工具包，GUI 服务（EP 9） |

### RISC-V ISA 扩展

| 类别 | 扩展 |
|------|------|
| **向量与 AI** | RVV 1.0（惰性上下文切换，向量 memcpy） |
| **中断与定时器** | AIA（APLIC+IMSIC）、Sstc（直接定时器）、VS-AIA（虚拟化中断） |
| **内存与分页** | Sv48/Sv57、Svnapot（64KB 页）、Svpbmt（内存类型）、Svinval（TLB）、Sspmp（S 态 PMP） |
| **缓存与加密** | Zicbom/Zicboz（缓存操作）、Zkr（熵源）、Zkne（AES）、Zknh（SHA）、Zks（SM3/SM4） |
| **安全与 IOMMU** | ePMP（增强 PMP）、RISC-V IOMMU、指针掩码（Ssnpm） |
| **性能** | Sscofpmf（PMU 29 事件）、Sdtrig（硬件调试）、Smstateen |
| **优化** | B 扩展（Zbb/Zbs/Zbkb）、Zicond（条件移动）、Zihintpause |

### 安全加固

| 机制 | 实现 |
|------|------|
| **W^X** | 页表强制执行，违规自动修复 |
| **ASLR/KASLR** | PCG 随机化栈/mmap/PIE，内核滑动（>30 位熵） |
| **栈/堆金丝雀** | Guard 页 + 魔数值验证 |
| **seccomp/CHERI/沙箱** | 每进程系统调用过滤 + 128 位胖指针 + 路径/网络/UID 沙箱 |
| **TEE 证明** | SHA-512 度量 + Ed25519 签名 + RATS 挑战-应答 |

### 生产就绪

| 领域 | 功能 |
|------|------|
| **Linux ABI** | 120+ 系统调用映射，标志/errno 转换，动态链接器（RISC-V RELA） |
| **/proc + /sys** | cpuinfo、meminfo、mounts、stat、loadavg、每进程 maps/status/cmdline/fd |
| **服务管理** | 依赖启动、自动重启、DHCP/DNS/包管理器 |
| **硬件** | QEMU virt、SiFive HiFive、StarFive VisionFive 2、Canaan K230 |

---

## 构建与运行

```bash
# 前置条件
rustup toolchain install nightly
rustup target add riscv64gc-unknown-none-elf
rustup component add rust-src

# 构建全部
cd TrainOS && make all

# 在 QEMU 上运行（2 CPU）
make run
```

或手动运行：
```bash
qemu-system-riscv64 -machine virt -smp 2 -nographic \
  -bios rustsbi-qemu-new.bin \
  -kernel target/riscv64gc-unknown-none-elf/release/kernel
```

---

## 演进时间线

```
V1-V12   V13-V20    V21-V30      V31-V34    V35-V38
███████  █████████  ████████████  █████████  █████████
 基础     功能完善    路线图驱动     调研驱动    Linux追平
  内核    81+ sc     10版本        4版本       +RISC-V
  IPC     35+服务    15,050行      5,700行     20+ ISA扩展
```

| 阶段 | 版本 | 方法论 | 关键产出 |
|------|------|--------|---------|
| **基础** | V1-V12 | 增量迭代 | Boot、MMU、调度器、IPC、文件系统、网络、VirtIO |
| **功能** | V13-V20 | 功能驱动 | TCP、VFS、命名空间、35+ 服务、POSIX |
| **路线图** | V21-V30 | 10 版本 / 4 波次 | 形式化验证、io_uring、虚拟化、eBPF、NUMA、分布式 IPC、CHERI/ASLR、WASM、GPU/AI、Linux ABI |
| **调研驱动** | V31-V34 | CCF-A 论文驱动 | 事务化 MMU（CortenMM SOSP'25）、WASM 混合（WABI）、TEE（TEEM³）、P/D 调度器（OSDI'24） |
| **Linux 追平** | V35-V38 | Linux + RISC-V 调研 | PREEMPT_LAZY、Proxy Exec、mseal、mTHP、RWF_ATOMIC、RVV 1.0、AIA、Sv48、加密、PMU、GUI |

### 调研基础（V31-V34）

V31-V34 基于 27 篇 CCF-A 会议论文的系统性调研（SOSP/OSDI/EuroSys/ASPLOS/USENIX ATC 2024-2026）。详见[调研报告](os-ccfa-research-2024-2026/report.md)。

---

## 项目结构

```
TrainOS/
├── kernel/src/                  # 内核（~20,000 行）
│   ├── cap/                     # 能力系统
│   ├── ipc/                     # IPC 端点与消息
│   ├── syscall/                 # 系统调用分发（362+）
│   ├── mem/                     # 伙伴分配器、Sv39/Sv48、事务化 MMU、mTHP、缓存操作
│   ├── trap/                    # 陷阱分发、PMU、调试、Sstc、AIA
│   ├── proc/                    # 进程、线程、ELF 加载器
│   ├── sched/                   # EEVDF 调度器、NUMA 感知
│   ├── security/                # W^X、seccomp、能力审计、TEE、远程证明
│   ├── crypto/                  # AES、SHA-2/3、SM3/SM4（Zk 扩展）
│   ├── iouring/                 # io_uring 异步 I/O
│   ├── extension/               # eBPF 风格内核扩展
│   ├── hypervisor/              # H 扩展、VM、VirtIO、VS-AIA
│   ├── numa/                    # NUMA、每 CPU 计数器、MCS、RCU
│   ├── distributed/             # 分布式 IPC
│   ├── wasm/                    # WASM 解释器、WASI、宿主调用
│   ├── ai/                      # GPU 驱动、AI 调度器、张量运算
│   ├── compat/                  # Linux ABI、/proc、/sys、动态链接器
│   ├── device/                  # 帧缓冲、窗口管理器、组件、blk-mq
│   └── main.rs                  # 启动序列
├── services/                    # 用户态服务
├── lib/tros/                    # 用户态系统调用库
├── docs/                        # 设计文档、实现计划、路线图
├── os-ccfa-research-2024-2026/  # CCF-A 会议论文调研（12 项深度调研）
├── .ide/Dockerfile              # CNB 云原生开发环境
├── .cnb.yml                     # CNB 云端 IDE 工作区配置
└── Makefile
```

---

## 核心设计决策

| 决策 | 理由 |
|------|------|
| 纯 Rust (`no_std`) | 编译期内存安全，消除 C 语言的未定义行为 |
| 微内核架构 | 最小化可信计算基（TCB），故障隔离，便于形式化验证 |
| 基于能力的权限模型 | 细粒度访问控制，无环境权限 |
| RISC-V 专属 | 简洁开放指令集，蓬勃发展的生态 |
| Sv39 + Sv48 虚拟内存 | 512GB → 256TB 地址空间 |
| AI 协同设计 | 全部代码与架构由 AI 协同设计与实现 |
| 双平台 | GitHub + CNB 云原生开发 |

---

## 文档

- [V21-V30 路线图](docs/specs/2026-05-18-trainos-v21-v30-roadmap.md)
- [操作系统 CCF-A 调研报告](os-ccfa-research-2024-2026/report.md)
- [CLAUDE.md](CLAUDE.md) — AI Agent 上下文

---

## 许可证

MIT
