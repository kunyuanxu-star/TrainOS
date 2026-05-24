# 操作系统 CCF-A 会议论文调研报告

**调研日期**: 2026-05-24 | **范围**: SOSP/OSDI/EuroSys/ASPLOS/USENIX ATC 2024-2026
**调研项数**: 12 | **字段数**: 25

## 执行摘要

本次调研覆盖 **12** 篇/项 CCF-A 会议论文及技术趋势，其中 **8** 项对 TrainOS 具有高参考价值。

### 核心发现

1. **微内核产业化已验证** — HongMeng (OSDI'24) 在数亿设备上证明了微内核架构的可行性，三级隔离+灵活组合是可直接借鉴的核心设计
2. **Rust OS 形式化验证取得突破** — CortenMM (SOSP'25 Best Paper) 用 Verus 证明了并发 MMU 操作的正确性，是 TrainOS 同生态的直接对标
3. **WASM 作为 eBPF 替代方案** — EuroSys'25 和 CMU 2025 分别从用户态和内核态探索 WASM 在 OS 中的角色，共识趋向 eBPF(热路径)+WASM(复杂逻辑)混合架构
4. **AI+OS 协同是最大新兴方向** — OSDI'24 有 8 篇 LLM 推理论文，P/D 分离架构天然适合微内核
5. **机密计算+微内核是自然组合** — TEEM³ (ASPLOS'26) 基于 M³ 微内核实现异构 TEE，TrainOS 可沿此路线扩展

## 目录

| # | 论文/项目 | 会议 | 主题 | 相关性 | 路线图映射 |
|---|----------|------|------|--------|-----------|
| 1 | [LLM推理与OS协同（AI-OS Collaboration Trend，2024-2026多会议趋势方向）](#aios_trend) | 多会议综合趋势：OSDI 2024（8篇LLM推理论文）、SOSP 2024（2篇）、ASPLOS 2024/2025/2026（GPU调度与AI系统）、EuroSys 2024/2025（推理优化）、USENIX ATC 2024/2025 2024 | AI+OS协同 | 高 | V29_ai_os |
| 2 | [CONFIDENTIAL](#confidential) |   |  |  |  |
| 3 | [CortenMM: Efficient Memory Management with Strong Correctnes...](#cortenmm) | SOSP 2025 2025 | 内存管理 | 高 | V21_verification |
| 4 | [EAGLE_TEEM3](#eagle_teem3) |   |  |  |  |
| 5 | [EBPFUN](#ebpfun) |   |  |  |  |
| 6 | [EBPFVER](#ebpfver) |   |  |  |  |
| 7 | [FineMem: Breaking the Allocation Overhead vs. Memory Waste D...](#finemem) | OSDI 2025 (19th USENIX Symposium on Operating Systems Design and Implementation) 2025 | 分解内存 | 高 | V27_defense_depth |
| 8 | [Microkernel Goes General: Performance and Compatibility in t...](#hongmeng) | OSDI 2024 2024 | 微内核设计 | 高 | cross_cutting |
| 9 | [An Empirical Study of Rust-for-Linux: The Success, Dissatisf...](#rfl) | USENIX ATC 2024 2024 | Rust语言级 | 高 | cross_cutting |
| 10 | [SquirrelFS: using the Rust compiler to check file-system cra...](#sqfs) | OSDI 2024 2024 | 文件/存储 | 高 | V21_verification |
| 11 | [Empowering WebAssembly with Thin Kernel Interfaces](#wabi) | EuroSys 2025 2025 | WebAssembly | 高 | V28_wasm |
| 12 | [Safe Kernel Extensibility and Instrumentation With WebAssemb...](#wasmext) | CMU Technical Report (CMU-CS-25-123) 2025 | 内核扩展 | 高 | cross_cutting |

## 详细内容

### AIOS_TREND

**基本信息**

- **Title**: LLM推理与OS协同（AI-OS Collaboration Trend，2024-2026多会议趋势方向）
- **Conference**: 多会议综合趋势：OSDI 2024（8篇LLM推理论文）、SOSP 2024（2篇）、ASPLOS 2024/2025/2026（GPU调度与AI系统）、EuroSys 2024/2025（推理优化）、USENIX ATC 2024/2025
- **Year**: 2024
- **Authors**: OSDI 2024代表性论文作者包括：Yinmin Zhong（北京大学，DistLLM）、Yao Fu（爱丁堡大学，ServerlessLLM）、Chaoyi Jiang（上海交通大学，Parrot）、Shengyu Liu（北京大学，DistLLM）、Bingyang Wu（北京大学，dLoRA）等
- **Institution**: 主要研究机构：北京大学、上海交通大学（IPADS）、清华大学、阿里巴巴、Microsoft Research、UC Berkeley、UC San Diego、爱丁堡大学、首尔大学、乔治亚理工、CMU、斯坦福大学、Duke大学
- **Status**: trend

**技术方向**

- **Paper Theme**: ai_os
- **Key Idea**: 2024-2026年操作系统顶级会议（OSDI/SOSP/ASPLOS/EuroSys）爆发性涌入LLM推理系统研究，标志着OS社区研究重心的重大转向。核心主题包括：（1）GPU计算资源的高效调度与复用（Llumnix、dLoRA）；（2）KV-cache内存管理与分页优化（InfiniGen、vLLM开创的PagedAttention）；（3）推理serving系统的Prefill-Decode分离架构（DistServe、Sarathi-Serve）；（4）长上下文推理与弹性并行（LoongServe）；（5）多租户公平调度与服务质量保障（Fairness in Serving LLMs）。
- **Keywords**: LLM推理服务, GPU调度, KV-cache内存管理, Prefill-Decode分离, 弹性并行推理
- **Technical Contribution Type**: analysis_improvement

**TrainOS相关性**

- **Relevance**: high
- **Applicable Techniques**: - P/D分离架构（DistServe、Sarathi-Serve）：将推理的Prefill和Decode阶段分离到不同GPU上独立优化。TrainOS V29可设计AI加速器的分段调度器，将计算密集的Prefill和访存密集的Decode映射到不同的硬件资源或调度策略
- Chunked-Prefill + 无停顿批处理（Sarathi-Serve）：将长Prompt切分为小块，与Decode交错执行，消除传统批处理中Prefill导致的Decode停顿。TrainOS可为AI推理服务设计分时调度原语，在微内核层面支持交错执行
- KV-cache分页管理（vLLM PagedAttention思想 + InfiniGen预测预取）：将KV-cache按逻辑页管理，结合CPU-GPU分层存储和注意力模式预测预取。TrainOS可在V22-V25的NUMA/分层内存管理基础设施上构建KV-cache感知的虚拟内存子系统
- 弹性序列并行（LoongServe ESP）：动态调整序列在不同GPU间的并行度，适应可变长度请求。TrainOS V24（内核扩展）可设计AI特定系统调用，支持GPU/NPU资源的弹性绑定和解绑
- 动态调度与在线迁移（Llumnix）：通过请求的实时迁移在多实例间平衡负载。TrainOS V26（分布式IPC）提供的进程迁移能力可作为底层基础设施，支撑AI推理服务的动态扩缩容
- Serverless LLM冷启动优化（ServerlessLLM）：利用分层存储（HDD/内存）加速模型检查点加载。TrainOS V25 NUMA感知存储管理器可以为分层AI模型缓存提供高效的数据放置策略
- GPU-CPU异构推理（PowerInfer模型激活稀疏性）：根据神经元激活的幂律分布，在GPU/CPU间动态分配计算。TrainOS V29可设计CPU-GPU统一内存管理接口，支持稀疏计算模式下的数据传输优化
- 多LoRA适配器动态编排（dLoRA）：在实例级和集群级动态管理多个微调适配器的加载和批处理。TrainOS V24/V28可在内核扩展中支持WASM沙箱化的LoRA推理任务调度
- 语义变量抽象（Parrot）：暴露应用级请求依赖关系给调度器，实现DAG级别的推理优化。TrainOS V26 IPC的消息模型中可嵌入类似语义标签，使内核调度感知应用级意图
- KV-cache压缩与量化：通过内存压缩技术减少KV-cache占用（如KVT、SparQ等）。TrainOS V21/V27可引入内存压缩引擎作为安全加固和资源优化的交叉技术
- **Roadmap Mapping**: V29_ai_os
- **Compatibility With Rust**: rust_compatible
- **Implementation Readiness**: needs_prototyping

**影响力与成熟度**

- **Maturity**: research
- **Open Source**: yes
- **Repo Url**: Sarathi-Serve: https://github.com/microsoft/sarathi-serve; vLLM: https://github.com/vllm-project/vllm; ServerlessLLM: https://github.com/ServerlessLLM/ServerlessLLM
- **Citations Impact**: highly_cited
- **Ecosystem Relevance**: ecosystem_defining

**行动指南**

- **Time Sensitivity**: highly_time_sensitive
- **Engagement Level**: prototype
- **Risks And Caveats**: - AI系统研究领域迭代极快（以月为单位），当前OSDI 2024论文中的技术可能在12个月内被新一代方案部分替代，跟踪投入需要持续资源
- 大多数LLM推理系统围绕NVIDIA GPU和CUDA生态设计，TrainOS的RISC-V QEMU环境目前无法运行真实GPU推理负载，所有原型验证需依赖模拟器或交叉编译
- 微内核IPC延迟可能成为AI推理中频繁的GPU内存管理调用的瓶颈，需要设计轻量级的GPU服务调用路径
- KV-cache管理和GPU调度的核心逻辑通常运行在GPU driver/kernel态，TrainOS的微内核架构意味着需要将部分调度逻辑下放到特权级服务或设计新的交互模型
- TrainOS V29的目标是AI原生OS，但AI硬件生态（GPU/TPU/NPU）目前主要由x86/CUDA主导，RISC-V AI加速器生态尚在早期（如Tenstorrent、Esperanto），硬件可用性是关键风险
- 当前趋势论文主要解决LLM推理问题，未来2-3年OS-AI协同可能转向AI Agent、具身智能、AI for System等新方向，路线图需要保持灵活性
- **Suggested Next Step**: （1）成立AI OS跟踪小组：每周跟踪OSDI/SOSP/ASPLOS/EuroSys最新AI论文，建立论文库和代码仓库索引。（2）优先原型验证P/D分离架构：在TrainOS用户空间中模拟实现一个最小化的Prefill-Decode分离调度原型，验证微内核架构下AI推理调度延迟是否可接受。（3）KV-cache管理原型：基于TrainOS V22之后的NUMA/分层内存基础设施，原型KV-cache的分页管理方案，评估RISC-V Sv39页表对GPU虚拟内存的支持能力。（4）与vLLM/SGLang社区建立联系：了解LLM推理引擎的调度和内存管理需求，为TrainOS V29的AI服务接口设计提供输入。（5）编写AI OS调研报告：系统梳理OSDI'24-SOSP'24-ASPLOS'25/26中AI相关论文，提炼可用于TrainOS的10+项具体技术，形成V29技术储备库。

**不确定字段**: effort_estimate

---

### CONFIDENTIAL

**其他信息**

- **fields**: - **title**: Confidential Computing & TEE (2024-2026趋势综述)
- **conference**: 多会议综合 (ASPLOS'24-'26, EuroSys'25, SIGMETRICS'25, USENIX Security'24, OSDI'25)
- **year**: 2024-2026
- **authors**: 多作者综合趋势，关键贡献者包括: Misono等 (SIGMETRICS 2025, AMD SEV-SNP/Intel TDX实证分析); Jiacheng Shi, Yang Yu, Jinyu Gu, Yubin Xia (上海交通大学IPADS, CKI/EuroSys 2025); Nils Asmussen, Sebastian Haas, Carsten Weinhold, Michael Roitzsch等 (Barkhausen Institut, TEEM³/ASPLOS 2026); Chuqi Zhang等 (NUS/Intel, Erebor/EuroSys 2025); Till Miemietz等 (Barkhausen Institut/TU Dresden, MettEagle/OSDI 2025)
- **institution**: 多机构: 上海交通大学IPADS; 慕尼黑工业大学; Barkhausen Institut; TU Dresden; Georgia Tech/Intel Labs; National University of Singapore; Imperial College London; Columbia University
- **status**: trend
- **paper_theme**: confidential_computing
- **key_idea**: 机密计算与可信执行环境(TEE)是2024-2026年操作系统和体系结构领域最活跃的研究方向之一。硬件TEE技术(Intel TDX、AMD SEV-SNP、Arm CCA)进入成熟部署阶段，软件-硬件协同设计的安全容器方案(CKI)、微内核容器(MettEagle)、核心独立TEE(TEEM³)等创新推动容器与TEE深度融合。同时针对TEE的物理攻击(TEE.fail)和侧信道攻击(WeSee, HECKLER)持续涌现，推动TCB最小化和形式化验证研究。
- **keywords**: 机密计算, 可信执行环境, Intel TDX, AMD SEV-SNP, Arm CCA, 安全容器, 硬件-软件协同设计, 微内核, TCB最小化
- **technical_contribution_type**: survey
- **relevance**: high
- **applicable_techniques**:   - 1. 微内核+TEE融合架构: TEEM³展示M³微内核架构可实现核心独立、协作式TEE，为TrainOS的微内核安全架构提供直接参考
  - 2. CKI的PKS隔离方案: 利用Protection Keys构建新特权级的思路可借鉴到RISC-V平台的PMP/ePMP隔离机制设计
  - 3. 安全容器轻量化方案: 避免二阶段地址转换(EPT/NPT)开销的设计思路适用于TrainOS的进程隔离
  - 4. TCB最小化方法论: 所有TEE研究的核心趋势——减少可信计算基，与微内核哲学高度一致
  - 5. SVSM/Secure VM Service Module: AMD SEV-SNP的VMPL特权级分离机制可供TrainOS能力系统参考
  - 6. 机密虚拟机性能优化: TDX/SEV-SNP的性能特征分析为TrainOS的VM/容器安全方案设计提供基线数据
  - 7. 跨TEE协作通信: TEEM³和Mica(EuroSys)展示的多TEE安全协作模式可适配TrainOS的IPC机制
  - 8. RISC-V TEE标准化: Keystone开源框架和AP-TEE标准为TrainOS在RISC-V上实现TEE提供路径
- **roadmap_mapping**: V27_defense_depth
- **compatibility_with_rust**: rust_compatible
- **implementation_readiness**: needs_prototyping
- **maturity**: production
- **open_source**: partial
- **repo_url**: https://github.com/Barkhausen-Institut/M3 (M³架构); https://github.com/keystone-enclave/keystone (RISC-V TEE参考)
- **citations_impact**: highly_cited
- **ecosystem_relevance**: important_trend
- **effort_estimate**: 不确定（多方案整合约48-96人周，取决于选型范围）
- **time_sensitivity**: moderately_time_sensitive
- **engagement_level**: prototype
- **risks_and_caveats**:   - 1. 硬件TEE方案依赖特定CPU特性: TDX(Intel)、SEV-SNP(AMD)、CCA(Arm)各自绑定特定架构，TrainOS基于RISC-V需要替代方案(如Keystone或自研PMP/ePMP隔离)
  - 2. 物理攻击威胁: TEE.fail(2025)展示了DDR5确定性AES-XTS加密缺乏Merkle树完整性保护的漏洞，所有服务器级TEE均受影响
  - 3. 侧信道攻击: WeSee(#VC注入)、HECKLER(中断注入)等攻击表明现有TEE硬件仍有未修复的安全间隙
  - 4. VMEXIT性能开销: TDX的VMEXIT延迟最高可达基线的6.8倍，影响I/O密集型工作负载
  - 5. 技术碎片化: 多种TEE方案互不兼容，TrainOS需选择合适的TEE基线或设计抽象层
  - 6. 成熟度差异: Arm CCA尚未有硬件可用，RISC-V TEE生态(AP-TEE)仍在标准化阶段
- **suggested_next_step**:   - 1. 深入研究TEEM³的M³架构设计，评估其核心独立TEE方案在RISC-V+TrainOS微内核上的可行性
  - 2. 调研RISC-V Keystone开源TEE框架，评估作为TrainOS TEE原型基线的可能性
  - 3. 跟踪RISC-V AP-TEE标准化进展，确保与未来RISC-V TEE生态兼容
  - 4. 设计TrainOS安全沙箱原型，融合微内核IPC隔离(已有能力)与轻量级TEE隔离
  - 5. 研究CKI的PKS隔离方案，探索TrainOS在RISC-V上使用PMP/ePMP实现类似内核态隔离
  - 6. 与Barkhausen Institut团队(M³/TEEM³)建立联系，了解微内核TEE最佳实践

**不确定字段**: paper_theme: 作为趋势综述，论文主题归类为confidential_computing较为合适，但涵盖多个子主题, title: 趋势综述无单一论文标题，使用综合描述性标题, conference: 跨会议综合，非单一会议论文, authors: 无单一作者集合，列出关键贡献者, effort_estimate: 多种技术方案整合的工作量难以精确估计, repo_url: 部分方案开源(M³, Keystone)，部分未开源(TDX, SEV-SNP为厂商闭源)

---

### CORTENMM

**基本信息**

- **Title**: CortenMM: Efficient Memory Management with Strong Correctness Guarantees
- **Conference**: SOSP 2025
- **Year**: 2025
- **Authors**: 第一作者：Junyang Zhang（北京大学 & 中关村实验室）；通讯作者：Diyu Zhou（北京大学）、Hongliang Tian（蚂蚁集团）
- **Institution**: 北京大学、中关村实验室、蚂蚁集团、CertiK、UCLA、密歇根理工大学
- **Status**: confirmed

**技术方向**

- **Paper Theme**: memory_management
- **Key Idea**: 摒弃传统操作系统中VMA树+页表的两层内存抽象，设计单层（One-Level）架构直接在硬件页表上操作，消除软件同步开销和状态双重维护，配合事务化MMU接口和Verus形式化验证工具证明并发页表操作的正确性，在真实场景中性能最高达Linux的26倍，获得SOSP 2025最佳论文。
- **Keywords**: 单层内存管理, 形式化验证, 事务化MMU接口, Verus验证器, 并发正确性
- **Technical Contribution Type**: novel_design

**TrainOS相关性**

- **Relevance**: high
- **Applicable Techniques**: - 单层内存抽象设计：TrainOS可摒弃传统VMA树软件抽象，直接在Sv39页表上构建内存管理子系统，消除双层抽象的同步开销和内存浪费
- 事务化MMU接口：设计事务化的页表操作原语（begin/commit/abort），保证并发页表操作在发生竞争时自动回滚，无需复杂锁协议。TrainOS可在IPC消息中嵌入事务化内存操作
- Verus形式化验证方法论：使用Rust + Verus对并发MMU操作进行形式化验证，已发现页表节点过早释放等并发bug。TrainOS可用相同工具链验证其capability系统的内存安全
- 单层架构下Rust所有权模型与形式化验证的结合模式：将页表节点视为拥有唯一所有权的资源，通过Rust类型系统在编译期消除释放后使用等内存安全漏洞
- vostd验证框架：Asterinas团队开源的ostd形式化验证代码库，TrainOS可直接参考其验证模式验证自身的内存管理原语
- **Roadmap Mapping**: V21_verification
- **Compatibility With Rust**: native_rust
- **Implementation Readiness**: ready_for_adoption

**影响力与成熟度**

- **Maturity**: prototype
- **Open Source**: partial
- **Repo Url**: https://github.com/TELOS-syslab/CortenMM-Artifact
- **Citations Impact**: best_paper
- **Ecosystem Relevance**: ecosystem_defining

**行动指南**

- **Time Sensitivity**: highly_time_sensitive
- **Engagement Level**: engage_community
- **Risks And Caveats**: - 单层设计假设x86/ARM/RISC-V的MMU设计已趋于统一且稳定，但未来ISA演进（如CHERI扩展）可能导致设计需要调整
- 事务化MMU接口可能与TrainOS现有的capability系统和IPC消息模型存在设计冲突，需要仔细集成
- Verus形式化验证工具链仍在快速演进中，可能无法直接支持TrainOS使用的nightly Rust工具链版本
- 论文主要验证在Asterinas的framekernel架构上，在微内核架构下（尤其是服务间IPC传递内存相关操作）可能需要额外适配工作
- 消除VMA软件抽象层后，部分依赖软件元数据的高级功能（如mremap、透明大页、用户态页错误处理）可能需要设计替代的元数据管理方案
- CortenMM的验证覆盖约2K行可执行代码和6K行规约，要达到完整验证需要数倍的投入
- 单层架构中所有内存策略从内核移至用户态，可能增加用户态服务之间的IPC复杂度
- **Suggested Next Step**: 立即阅读论文全文并克隆CortenMM工件仓库（https://github.com/TELOS-syslab/CortenMM-Artifact）进行实验验证。与Asterinas社区通过GitHub Issues/Discussions建立联系，了解单层设计在RISC-V 64位平台上的实际部署情况和适配经验。评估在TrainOS内存管理服务中实现单层内存管理原型的可行性，优先使用Verus验证核心页表操作并发正确性。安排1-2名团队成员深入研读CortenMM和vostd代码，评估直接复用验证基础设施的可能性。

**不确定字段**: effort_estimate

---

### EAGLE_TEEM3

**其他信息**

- **fields**: - **title**: Eagle (CKI): A Hardware-Software Co-Design for Efficient Secure Containers + TEEM³: Core-Independent and Cooperating Trusted Execution Environments
- **conference**: Eagle: EuroSys 2025 (与ASPLOS 2025联合举办) / TEEM³: ASPLOS 2026
- **year**: 2025 / 2026
- **authors**: Eagle (CKI): Jiacheng Shi, Yang Yu, Jinyu Gu, Yubin Xia (上海交通大学IPADS) / TEEM³: Nils Asmussen, Sebastian Haas, Carsten Weinhold, Nicholas Gordon, Stephan Gerhold (TU Dresden), Friedrich Pauls, Nilanjana Das, Michael Roitzsch (Barkhausen Institut)
- **institution**: Eagle (CKI): 上海交通大学IPADS / TEEM³: Barkhausen Institut (德累斯顿), TU Dresden
- **status**: partial_confirmed
- **paper_theme**: confidential_computing
- **key_idea**: Eagle (CKI): 利用Intel PKS(Protection Keys for Supervisor)在内核态中构建新特权级，避免使用虚拟化硬件(EPT/NPT)运行安全容器，通过硬件-软件协同设计实现VM级隔离性能提升最高72%(嵌套云场景)。TEEM³: 基于M³微内核架构实现核心独立、可协作的TEE，将TEE从CPU扩展到AI加速器等异构硬件，显著降低可信计算基(TCB)复杂度。两者共同代表容器+TEE融合的最新方向：前者专注安全容器的高效隔离，后者专注TEE的通用化和微内核化。
- **keywords**: 安全容器, 硬件-软件协同设计, PKS, 内核隔离, 微内核TEE, 核心独立TEE, M³架构, TCB最小化, 可信执行环境, 异构加速器
- **technical_contribution_type**: novel_design
- **relevance**: high
- **applicable_techniques**:   - 1. PKS式特权级扩展: TrainOS可在RISC-V上利用PMP/ePMP/S-mode物理内存保护机制实现类似CKI的内核态隔离层
  - 2. 单阶段地址转换: CKI避免二阶段地址转换的设计证明虚拟化硬件对容器隔离是不必要的过度设计，TrainOS微内核可天然避免此类开销
  - 3. Kernel Security Monitor (KSM)模式: 每个安全域内共置一个最小特权监视器处理敏感操作的模式适用于TrainOS的能力系统设计
  - 4. M³架构的TCU隔离: 硬件TCU(Trusted Communication Unit)实现的核心间隔离可直接启发TrainOS多核安全通信设计
  - 5. 核心独立TEE: TEEM³展示TEE不必绑定到特定CPU-core，TrainOS可将TEE扩展到RISC-V AI加速器或协处理器
  - 6. 协作式TEE: 多TEE安全协作的模式适用于TrainOS微内核的服务间安全通信，可结合现有IPC机制
  - 7. 轻量级中断保护: CKI的IST和自动PKS切换防御中断滥用的机制可适配到RISC-V的中断处理设计
  - 8. 快速切换门(PKS Switch Gate): 宿主机-客户内核-监视器之间的三级高效切换路径设计对微内核IPC优化有重要参考价值
- **roadmap_mapping**: V27_defense_depth
- **compatibility_with_rust**: rust_compatible
- **implementation_readiness**: needs_prototyping
- **maturity**: research
- **open_source**: partial
- **repo_url**: M³/TEEM³: https://github.com/Barkhausen-Institut/M3 / CKI: 论文PDF可从ACM DL获取(DOI: 10.1145/3689031.3717473)
- **citations_impact**: too_early
- **ecosystem_relevance**: important_trend
- **effort_estimate**: Eagle(CKI)方案移植到TrainOS-RISC-V约24-48人周(需PMP/ePMP适配); TEEM³方案借鉴约16-32人周(M³架构学习与概念验证)
- **time_sensitivity**: highly_time_sensitive
- **engagement_level**: prototype
- **risks_and_caveats**:   - 1. 架构依赖性: CKI的PKS是x86特有功能，RISC-V无直接等价机制，需使用PMP/ePMP/S-mode页表保护替代，效果需验证
  - 2. Eagle/CKI名称未确认: 搜索未找到以"Eagle"命名的ASPLOS/EuroSys 2025论文，该论文官方系统名为CKI(Container Kernel Isolation)。"Eagle"可能是任务作者使用的内部代号或笔误(可能混淆MettEagle/OSDI 2025)
  - 3. 硬件扩展需求: CKI需要PKS硬件扩展(指令隔离)，在RISC-V上需设计等效的硬件支持
  - 4. M³架构依赖性: TEEM³基于M³硬件架构(含TCU)，在标准RISC-V平台上需纯软件模拟或硬件扩展
  - 5. 成熟度: CKI和TEEM³均为研究原型，未在生产环境验证
  - 6. 性能数据有限: TEEM³论文暂无公开详细性能评测数据
  - 7. MettEagle相关: Barkhausen团队同时发表了MettEagle(OSDI 2025)微内核容器论文，与Eagle(CKI)是不同的独立工作，需注意区分
- **suggested_next_step**:   - 1. 阅读CKI论文全文(ACM DL DOI: 10.1145/3689031.3717473)，理解PKS隔离机制的完整设计
  - 2. 研究TEEM³论文(ACM DL DOI: 10.1145/3779212.3790232)及M³架构(开源GitHub仓库)，评估微内核TEE设计理念
  - 3. 在TrainOS原型中设计PMP-based隔离方案，对标CKI的KSM模式
  - 4. 与Barkhausen Institut团队建立技术联系(特别是M³和TEEM³团队)，获取第一手设计经验
  - 5. 评估TrainOS现有IPC机制如何演进为协作式TEE通信(参考TEEM³的跨TEE协作)
  - 6. 在RISC-V QEMU上搭建Keystone TEE原型作为对照基线
  - 7. 设计TrainOS V27安全沙箱方案，结合CKI的单阶段地址转换思路和微内核IPC能力
  - 8. 考虑联系上海交通大学IPADS团队(夏虞斌教授)了解CKI在非x86平台的移植可能性

**不确定字段**: title: "Eagle"系统名称为任务给定的非官方名称，论文官方系统名为CKI(Container Kernel Isolation)，实际标题为"A Hardware-Software Co-Design for Efficient Secure Containers", conference: Eagle(CKI)实际发表于EuroSys 2025(与ASPLOS 2025联合举办)，非ASPLOS 2025单独发表, citations_impact: 2025-2026新论文引用量尚低，无法评估长期影响, effort_estimate: 工作量估算基于对类似RISC-V安全原型的经验推测，需实际验证, repo_url: CKI论文未明确标注开源仓库地址; M³/TEEM³开源但需要进一步确认TEEM³本身代码是否已发布

---

### EBPFUN

**其他信息**

- **meta**: - **paper_id**: EBPFUN
- **research_date**: 2026-05-24
- **status**: confirmed
- **fields**: - **title**: Revealing the Unstable Foundations of eBPF-Based Kernel Extensions
- **conference**: EuroSys 2025
- **year**: 2025
- **authors**: Shawn Wanxiang Zhong（第一作者，威斯康星大学麦迪逊分校）、Jing Liu（微软研究院）、Andrea C. Arpaci-Dusseau、Remzi H. Arpaci-Dusseau
- **institution**: 威斯康星大学麦迪逊分校（University of Wisconsin-Madison）
- **status**: confirmed
- **paper_theme**: kernel_extension
- **key_idea**: 系统性地揭示eBPF程序与内核镜像之间的依赖不匹配问题。提出DepSurf工具，自动检测eBPF程序对内核函数、结构体、跟踪点的依赖与内核实际提供的内容之间的差异。分析25个内核镜像（跨度8年、5种架构、5种配置、14种编译器版本）和53个真实eBPF程序，发现83%的程序受依赖不匹配影响。
- **keywords**: eBPF兼容性, 内核扩展, 依赖分析, 编译优化, DepSurf, 稳定性
- **technical_contribution_type**: empirical_study
- **relevance**: high
- **applicable_techniques**: （1）依赖表面分析（Dependency Surface Analysis）：从编译后的内核镜像中提取eBPF程序可用的所有内核构造（函数、结构体、跟踪点），为TrainOS内核扩展API的稳定性评估提供方法论；（2）依赖集分析（Dependency Set Analysis）：分析扩展程序实际依赖的内核构造并与依赖表面进行交叉比对，发现不匹配；（3）跨版本兼容性检查方法：论文分析8年间内核变化规律的方法可直接用于TrainOS内核扩展接口的版本兼容性测试；（4）编译优化感知的稳定性分析：论文揭示编译器内联和签名变换对eBPF程序稳定性的影响，TrainOS设计时应考虑编译器对扩展API的影响。
- **roadmap_mapping**: V24_kernel_ext
- **compatibility_with_rust**: neutral
- **implementation_readiness**: needs_adaptation
- **maturity**: research
- **open_source**: yes
- **repo_url**: https://github.com/ShawnZhong/DepSurf
- **citations_impact**: too_early
- **ecosystem_relevance**: important_trend
- **effort_estimate**: 8-16人周（基于DepSurf方法论为TrainOS内核扩展构建兼容性检查工具，需要适配TrainOS的扩展接口和架构）
- **time_sensitivity**: moderately_time_sensitive
- **engagement_level**: read_only
- **risks_and_caveats**: （1）DepSurf主要针对Linux eBPF生态分析，TrainOS的自定义内核扩展架构不同（微内核设计、Rust语言、RISC-V架构），其具体分析结果（如函数签名变化率）不能直接迁移；（2）论文方法对内核镜像进行二进制分析（依赖BTF/CTF调试信息），TrainOS缺乏类似调试基础设施，需要开发新的依赖提取方法；（3）论文揭示的问题具有重要警示意义——内核扩展API若不精心设计稳定接口，将导致大量兼容性问题，TrainOS V24应从架构层面设计稳定的扩展API，避免重蹈Linux eBPF的覆辙；（4）论文主要聚焦问题发现，未提供完整的解决方案，需要TrainOS团队自行设计API稳定性保障机制。
- **suggested_next_step**: （1）详细阅读论文全文和DepSurf开源代码（MIT协议），理解依赖不匹配的具体分类和检测方法；（2）基于论文的发现，制定TrainOS V24内核扩展API的设计原则：接口应精简化、版本化、向后兼容；（3）参考DepSurf的依赖分析思路，为TrainOS构建扩展程序的依赖检查工具，在编译时和加载时检测API不匹配；（4）在设计TrainOS扩展API时，特别注意避免论文揭示的三类不匹配：API演进导致的变更、配置选项导致的API条件存在、编译器优化导致的API签名变化；（5）重点考虑Rust的trait系统和类型安全如何在编译期捕获这类不匹配，利用Rust语言优势从源头减少运行时兼容性问题。

**不确定字段**: citations_impact

---

### EBPFVER

**其他信息**

- **meta**: - **paper_id**: EBPFVER
- **research_date**: 2026-05-24
- **status**: confirmed
- **fields**: - **title**: Validating the eBPF Verifier via State Embedding
- **conference**: OSDI 2024
- **year**: 2024
- **authors**: Hao Sun（第一作者，ETH Zurich）、Zhendong Su（通讯作者，ETH Zurich）
- **institution**: ETH Zurich（瑞士苏黎世联邦理工学院）
- **status**: confirmed
- **paper_theme**: kernel_extension
- **key_idea**: 提出状态嵌入（State Embedding）技术，通过将eBPF程序的具体执行状态嵌入到程序中，利用eBPF验证器自身来检验其抽象近似是否正确。若验证器未能检测到嵌入的sink操作，则表明存在逻辑错误。该技术一个月内发现15个未知逻辑错误，其中2个可被利用进行本地权限提升。
- **keywords**: eBPF验证器, 状态嵌入, 逻辑错误检测, 内核安全, 抽象解释, SEV
- **technical_contribution_type**: tool_methodology
- **relevance**: high
- **applicable_techniques**: （1）状态嵌入验证方法：将程序具体执行状态反注回程序，利用验证器自我检验近似正确性，可用于TrainOS V24内核扩展验证器的正确性测试；（2）灰盒验证器测试方法：不依赖验证器内部实现细节，仅通过观察验证器状态来生成测试用例，降低测试成本；（3）近似正确性检查：通过构造sink条件并观察验证器是否拒绝，发现验证器的过近似和欠近似问题；（4）基于具体状态profile的测试生成：执行已验证过的程序并采集具体寄存器状态，反哺到程序中进行交叉验证。
- **roadmap_mapping**: V24_kernel_ext
- **compatibility_with_rust**: neutral
- **implementation_readiness**: needs_adaptation
- **maturity**: research
- **open_source**: no
- **repo_url**: 不确定（Hao Sun的GitHub账户无公开仓库，论文中SEV工具未找到开源代码）
- **citations_impact**: highly_cited
- **ecosystem_relevance**: important_trend
- **effort_estimate**: 16-24人周（基于论文重新实现SEV的核心状态嵌入算法和sink生成逻辑，并进行TrainOS适配）
- **time_sensitivity**: moderately_time_sensitive
- **engagement_level**: read_only
- **risks_and_caveats**: （1）SEV工具未开源，无法直接使用或二次开发，需要基于论文描述从零实现；（2）状态嵌入技术高度依赖eBPF验证器的具体架构（抽象解释+状态近似），TrainOS的内核扩展验证器如果采用不同验证方法（如符号执行、模型检查），则需要重新设计嵌入策略；（3）论文发现的15个bug反映Linux eBPF验证器的设计缺陷——验证器过于复杂导致难以保证正确性，TrainOS设计验证器时应追求简单、可形式化验证；（4）状态嵌入方法可能产生假阴性（存在bug但未触发），不能作为验证器正确性的完整保证。
- **suggested_next_step**: （1）详细阅读论文全文（USENET开放获取），重点理解状态嵌入的核心算法和sink条件的构造方法；（2）分析TrainOS V24内核扩展验证器的架构设计，确定使用抽象解释还是其他验证策略；（3）若采用抽象解释方案，参考SEV的嵌入技术设计TrainOS验证器的自动化测试框架；（4）关注Hao Sun后续工作"Approximation Enforced Execution"（USENIX Security 2025）和"Prove It to the Kernel"（SOSP 2025），这两项工作延续了状态嵌入思路并进化为更完整的方案；（5）在设计验证器时吸取Linux eBPF验证器的教训：保持验证器核心简单，避免复杂的状态跟踪逻辑，优先考虑可形式化验证的设计。

**不确定字段**: repo_url, citations_impact

---

### FINEMEM

**基本信息**

- **Title**: FineMem: Breaking the Allocation Overhead vs. Memory Waste Dilemma in Fine-Grained Disaggregated Memory Management
- **Conference**: OSDI 2025 (19th USENIX Symposium on Operating Systems Design and Implementation)
- **Year**: 2025
- **Status**: partial_confirmed

**技术方向**

- **Paper Theme**: disaggregated_memory
- **Key Idea**: 提出细粒度（4KB/2MB）远程内存分配器，利用RDMA单边原语实现无锁远程内存分配，通过预注册内存区域+RDMA Memory Window（MW）隔离机制，在消除分配开销与内存浪费之间取得突破，远程分配延迟降低95%，内存利用率提升2.25-2.8倍。
- **Keywords**: 细粒度分解内存, RDMA单边协议, 无锁远程分配器, Memory Window隔离, 预注册内存
- **Technical Contribution Type**: novel_design

**TrainOS相关性**

- **Relevance**: high
- **Applicable Techniques**: - RDMA Memory Window（MW）隔离机制：FineMem在每个计算节点上运行受信分配服务，通过RDMA MW对不同远程内存区域设置独立的L_Key/R_Key权限，实现租户间内存隔离。TrainOS在V29 AI原生OS阶段可参考此设计，在CXL/RDMA远程内存池化场景中实现安全的内存区域隔离
- 无锁单边RDMA分配协议：FineMem利用RDMA READ/WRITE原语直接在远程内存节点上进行分配/释放操作，无需远程CPU参与，消除上下文切换开销。TrainOS可在分布式IPC（V26）和远程内存访问路径中嵌入类似无锁协议，降低微内核IPC的远程内存访问延迟
- 预注册内存池化策略：FineMem预先注册大块RDMA内存区域，在其上运行自定义分配器管理4KB/2MB块。TrainOS可在VirtIO设备驱动层预注册共享内存区域，减少运行时内存注册开销
- 分配粒度与内存效率的权衡模型：FineMem的理论模型分析了固定大小块分配和按需分配之间的权衡。TrainOS在V25 NUMA感知内存管理、V29张量内存分配器设计中可采用类似模型
- 跨节点内存分配状态同步：FineMem的计算节点受信分配服务维护远程内存分配元数据，通过RDMA原子操作同步。TrainOS的分布式内存管理模块可借鉴此架构，在IPC消息中携带远程内存分配请求
- **Roadmap Mapping**: V27_defense_depth
- **Compatibility With Rust**: rust_compatible
- **Implementation Readiness**: needs_prototyping

**影响力与成熟度**

- **Maturity**: research
- **Open Source**: partial
- **Ecosystem Relevance**: important_trend

**行动指南**

- **Time Sensitivity**: moderately_time_sensitive
- **Engagement Level**: prototype
- **Risks And Caveats**: - FineMem依赖特定RDMA网卡硬件特性（Memory Window、原子操作），在QEMU模拟的RISC-V环境中无法直接验证，需真实的RDMA硬件或CXL模拟器
- 微内核架构下，远程内存分配服务需通过IPC转发，可能引入额外延迟，抵消FineMem的无锁协议优势
- FineMem的受信分配服务运行在计算节点上，假设计算节点本身是可信的，这与TrainOS的capability系统安全模型需要仔细集成和验证
- 4KB/2MB固定粒度分配对某些AI推理工作负载（如变长KV-cache块）可能不够灵活，需要额外的分片机制
- 当前OSDI'25论文刚发表，社区验证和后续优化工作尚不充分，直接采用存在未知风险
- RDMA Memory Window的规模和数量受硬件限制（RNIC资源有限），大规模部署时需考虑MW资源的复用和回收策略
- **Suggested Next Step**: 安排团队成员阅读FineMem论文全文（https://www.usenix.org/system/files/osdi25-wang-xiaoyang.pdf），重点关注第3-5节的设计细节和实验评估。评估在QEMU/RISC-V环境中搭建RDMA模拟测试平台的可行性（如使用SoftRoCE或RDMA over TCP模拟）。与IPADS实验室（上海交通大学）相关团队建立联系，了解RDMA MW在RISC-V平台上的适配经验。在TrainOS V26/V27路线图中，将远程内存分配器列为原型项目，优先实现基于IPC消息的远程内存分配协议草案。

**不确定字段**: authors, institution, repo_url, citations_impact, effort_estimate

---

### HONGMENG

**基本信息**

- **Title**: Microkernel Goes General: Performance and Compatibility in the HongMeng Production Microkernel
- **Conference**: OSDI 2024
- **Year**: 2024
- **Authors**: 第一作者：Haibo Chen（华为中央软件研究院 & 上海交通大学IPADS实验室）；通讯作者：Haibo Chen
- **Institution**: 华为中央软件研究院、上海交通大学IPADS实验室
- **Status**: confirmed

**技术方向**

- **Paper Theme**: microkernel_design
- **Key Idea**: 提出差异化隔离等级（IC0/IC1/IC2）和灵活组合（Flexible Composition）技术，允许微内核在安全隔离与性能之间动态权衡，通过地址令牌访问控制、策略无关内核分页等创新，在数亿设备上验证了微内核可兼顾通用场景性能与安全性，证明微内核可超越嵌入式场景走向通用。
- **Keywords**: 微内核设计, 差异化隔离等级, 灵活组合, 地址令牌访问控制, 策略无关内核分页
- **Technical Contribution Type**: production_report

**TrainOS相关性**

- **Relevance**: high
- **Applicable Techniques**: - 差异化隔离等级（IC0/IC1/IC2）：TrainOS可按安全关键度将内核服务分配到不同隔离等级，IC1采用硬件辅助轻量隔离（类似ARM Watchpoint/Intel PKS），IC2采用完整地址空间隔离，在安全性与性能之间灵活选择
- 灵活组合（Flexible Composition）：将频繁通信的OS服务（如文件系统和内存管理器）合并到同一地址空间将IPC优化为直接函数调用，大幅减少IPC开销。TrainOS可在IPC密集的服务对之间试点组合
- 地址令牌访问控制（Address-Token-Based Access Control）：将访问令牌编码到物理页的地址映射中，读操作仅需6 cycles（相比seL4 capability的526 cycles快87倍）。TrainOS可借鉴此设计优化capability系统的数据平面性能
- 策略无关内核分页（Policy-Free Kernel Paging）：内核仅执行页表操作，分页策略决策保留在用户态内存管理服务，减少IPC交互次数，页错误处理比Linux快38%
- Twin Drivers：驱动控制面在Linux驱动容器中运行，数据面使用轻量级“孪生驱动”，实现约1200 MB/s吞吐量，匹配Linux性能。TrainOS可参考此模式复用现有Linux驱动
- **Roadmap Mapping**: cross_cutting
- **Compatibility With Rust**: neutral
- **Implementation Readiness**: needs_adaptation

**影响力与成熟度**

- **Maturity**: production
- **Open Source**: no
- **Citations Impact**: highly_cited
- **Ecosystem Relevance**: important_trend

**行动指南**

- **Time Sensitivity**: long_tail
- **Engagement Level**: read_only
- **Risks And Caveats**: - HongMeng是华为商业产品，核心源码不开放，无法直接借鉴代码实现细节
- 灵活组合技术牺牲了部分隔离性以换取性能，需要在使用时仔细评估安全折衷，避免引入新的攻击面
- 三级隔离中的IC1依赖于特定硬件特性（如ARM Watchpoint / Intel PKS），RISC-V平台可能缺少对等的硬件支持，需要软件模拟或替代方案
- 论文中报告的性能提升是整体工程协同优化的结果，局部技术移植可能无法达到同样效果
- 华为拥有相关专利，部分设计可能在法律层面受限
- 无开源社区支持，无法获得持续的bug修复和功能演进
- **Suggested Next Step**: 阅读论文全文并组织团队讨论，重点分析差异化隔离等级和地址令牌访问控制的可移植性。评估在TrainOS中引入轻量级灵活组合机制的可行性，优先在IPC频繁的VFS和内存管理服务之间试点。同时调研RISC-V平台是否存在对IC1级轻量隔离的硬件支持（如PMP/ePMP），若无则评估纯软件方案的可行性和性能代价。

**不确定字段**: repo_url, effort_estimate

---

### RFL

**基本信息**

- **Title**: An Empirical Study of Rust-for-Linux: The Success, Dissatisfaction, and Compromise
- **Conference**: USENIX ATC 2024
- **Year**: 2024
- **Authors**: Hongyu Li (北京邮电大学), Liwei Guo (北京邮电大学/电子科技大学), Yexuan Yang (北京邮电大学), Shangguang Wang (北京邮电大学), Mengwei Xu (北京邮电大学)
- **Institution**: 北京邮电大学 (Beijing University of Posts and Telecommunications)
- **Status**: confirmed

**技术方向**

- **Paper Theme**: rust_lang
- **Key Idea**: 首个对Rust-for-Linux(RFL)项目的系统性实证研究，分析6个关键RFL驱动、269个issue、763个PR、1540个commit、3611封邮件和12501条Zulip讨论。发现Rust使内核更'可安全化'但非彻底安全；unsafe块在驱动开发中不可避免；ownership模型与内核细粒度内存控制存在根本矛盾导致复杂workaround；Rust驱动性能与C相当但在缺失优化场景下慢11倍；二进制体积增加33%。
- **Keywords**: Rust-for-Linux, 内核实证研究, unsafe代码, ownership冲突, 内核驱动性能, 二进制体积膨胀
- **Technical Contribution Type**: empirical_study

**TrainOS相关性**

- **Relevance**: high
- **Applicable Techniques**: safe abstraction模式：将unsafe代码封装在安全抽象层后暴露安全API；Pin+Box+类型化初始化API(InPlaceModule/PinInit)解决ownership与不可移动类型的矛盾；避免过度泛型化以减少二进制体积膨胀；理解ownership与内核资源管理的张力区域并设计适当的抽象边界；编写Rust内核代码时关注cache/TLB行为（指针密集型ownership共享会增加miss率）
- **Roadmap Mapping**: cross_cutting
- **Compatibility With Rust**: native_rust
- **Implementation Readiness**: ready_for_adoption

**影响力与成熟度**

- **Maturity**: research
- **Open Source**: yes
- **Repo Url**: https://github.com/Richardhongyu/rfl_empirical_tools
- **Citations Impact**: best_paper
- **Ecosystem Relevance**: important_trend

**行动指南**

- **Time Sensitivity**: long_tail
- **Engagement Level**: read_only
- **Risks And Caveats**: Rust在性能敏感路径上可能比C慢11倍（如e1000驱动因缺失预取优化导致ping延迟剧增）；二进制体积增加33%（泛型单态化+边界检查+drop胶水代码导致.text段增大约99%）；unsafe块无法避免需精心封装（NVMe驱动44个unsafe用法+16个安全抽象、GPU驱动107个unsafe用法+7个安全抽象）；ownership与内核设计哲学的矛盾导致复杂嵌套类型如Pin<Box<SpinLock<Box<Ring<RxDesc>>>>>；Rust的borrow checker无法捕获语义bug（如映射到错误内存位置能通过编译）
- **Suggested Next Step**: 阅读论文全文(USENIX ATC 2024 Proceedings)并深入分析实证发现；将论文中识别的ownership矛盾、性能陷阱和unsafe封装模式纳入TrainOS编码规范和代码审查checklist；特别关注Bin size膨胀问题以保证TrainOS在资源受限场景下的可行性；研究safe abstraction设计模式以确保unsafe代码的正确封装

**不确定字段**: effort_estimate

---

### SQFS

**基本信息**

- **Title**: SquirrelFS: using the Rust compiler to check file-system crash consistency
- **Conference**: OSDI 2024
- **Year**: 2024
- **Authors**: Hayley LeBlanc (德克萨斯大学奥斯汀分校), Nathan Taylor (德克萨斯大学奥斯汀分校), James Bornholt (德克萨斯大学奥斯汀分校), Vijay Chidambaram (德克萨斯大学奥斯汀分校)
- **Institution**: 德克萨斯大学奥斯汀分校 (University of Texas at Austin)
- **Status**: confirmed

**技术方向**

- **Paper Theme**: file_storage
- **Key Idea**: 利用Rust typestate模式，在编译时检查持久化内存文件系统的崩溃一致性，无需独立的形式化证明或运行时检查。核心创新是同步软更新(Synchronous Soft Updates)机制，将崩溃安全性归结为元数据更新顺序的编译时保证。
- **Keywords**: typestate模式, 崩溃一致性, 持久化内存, Rust编译器, 软更新, 编译时验证
- **Technical Contribution Type**: novel_design

**TrainOS相关性**

- **Relevance**: high
- **Applicable Techniques**: typestate模式用于在类型系统中编码状态机不变量（每个持久化对象携带持久化状态和操作状态两个类型级状态参数）；同步软更新(SSU)机制作为一种简化的崩溃一致性策略；零成本类型抽象（状态为零大小类型，无运行时开销）；编译时保证取代运行时验证的模式
- **Roadmap Mapping**: V21_verification
- **Compatibility With Rust**: native_rust
- **Implementation Readiness**: needs_adaptation

**影响力与成熟度**

- **Maturity**: research
- **Open Source**: yes
- **Repo Url**: https://github.com/utsaslab/squirrelfs
- **Citations Impact**: too_early
- **Ecosystem Relevance**: niche_specific

**行动指南**

- **Time Sensitivity**: long_tail
- **Engagement Level**: discuss_in_team
- **Risks And Caveats**: 仅检查基于排序的不变量，无法捕获语义错误（如inode内容正确性）或来自信任代码/编译器的bug；不能检查可变大小集合的属性（对编译器不可判定）；实现基于Linux内核v6.3.0的RFL框架，需适配微内核架构；当前仅针对持久化内存(PM)场景设计
- **Suggested Next Step**: 下载并阅读论文全文(arXiv:2406.09649)及演讲视频，深入理解typestate模式在Rust类型系统中的实现方式（两个typestate参数的设计模式）；评估将typestate模式应用于TrainOS文件系统崩溃一致性或IPC协议状态机编码的可行性

**不确定字段**: effort_estimate

---

### WABI

**基本信息**

- **Title**: Empowering WebAssembly with Thin Kernel Interfaces
- **Conference**: EuroSys 2025
- **Year**: 2025
- **Authors**: Arjun Ramesh（卡内基梅隆大学电子与计算机工程系）、Tianshu Huang（卡内基梅隆大学）、Ben L. Titzer（卡内基梅隆大学计算机科学系）、Anthony Rowe（卡内基梅隆大学/Bosch Research）
- **Institution**: 卡内基梅隆大学（Carnegie Mellon University）+ Bosch Research
- **Status**: confirmed

**技术方向**

- **Paper Theme**: wasm
- **Key Idea**: 将操作系统用户态syscall直接暴露为WebAssembly宿主函数（host functions），在不破坏WASM进程内沙箱隔离的前提下实现通用二进制格式。WASI等高阶能力API可作为WASM模块运行在这些内核接口之上，实现更好的分层和复用。
- **Keywords**: WebAssembly, Thin Kernel Interface, WALI, 系统调用虚拟化, 沙箱隔离
- **Technical Contribution Type**: novel_design

**TrainOS相关性**

- **Relevance**: high
- **Applicable Techniques**: 1) syscall-as-host-function模式：将TrainOS的IPC/能力调用暴露为WASM宿主函数，替代直接映射Linux syscall；2) 线性内存地址空间转换（address-space translation）：WALI对WASM线性内存做带边界检查的地址翻译，实现零拷贝I/O，可直接用于TrainOS WASM运行时；3) 名称绑定syscall（name-bound syscalls）：跨架构统一syscall接口设计，与平台无关，适合TrainOS跨RISC-V平台；4) WAMR引擎集成方式：多架构（x86-64/aarch64/riscv64）支持，AOT编译+解释双模式；5) binfmt_misc机制：让WASM/AoT二进制像原生ELF一样直接执行；6) WASI作为WASM模块叠加的设计模式：将WASI实现从引擎层解耦为沙箱化WASM层
- **Roadmap Mapping**: V28_wasm
- **Compatibility With Rust**: rust_compatible
- **Implementation Readiness**: needs_adaptation

**影响力与成熟度**

- **Maturity**: prototype
- **Open Source**: yes
- **Repo Url**: https://github.com/arjunr2/WALI
- **Citations Impact**: too_early
- **Ecosystem Relevance**: important_trend

**行动指南**

- **Effort Estimate**: 8-12人周（将WALI概念适配到TrainOS微内核架构，包括：WASM宿主函数接口设计2-3周、线性内存沙箱适配2-3周、WASI兼容层构建2-3周、集成测试与性能调优2-3周）
- **Time Sensitivity**: moderately_time_sensitive
- **Engagement Level**: prototype
- **Risks And Caveats**: 1) WALI设计深度依赖Linux syscall接口，TrainOS微内核使用IPC+能力（capability）模型而非syscall，直接移植不可行，需要重新设计宿主函数接口；2) WAMR引擎是C语言实现，集成到Rust内核需要FFI桥接或Rust重写（如使用wasmtime-rust）；3) 将WASM线性内存映射到TrainOS的Sv39页表结构需要额外适配层；4) WALI论文评估显示WASM运行时中位性能开销约2.32倍于原生执行，这对TrainOS实时性要求可能构成挑战；5) 进程内沙箱隔离虽然由WASM语义保证，但微内核环境中还需考虑能力传递和IPC级别的安全策略
- **Suggested Next Step**: 1) 详细阅读WALI论文（arXiv:2312.03858v4）及其GitHub仓库源码，重点关注syscall宿主函数接口规范和WAMR集成方式；2) 评估TrainOS V28已有WASM解释器（36 opcodes, 256-slot value stack）与WALI的差距，确定需要新增的宿主函数集合；3) 原型验证：在TrainOS上实现WALI-style的syscall宿主函数接口（约20个核心调用），移植一个非玩具应用（如lua或minimal-httpd）验证可行性；4) 与WASI preview2的已有实现（21个宿主函数）做整合评估，确定两者的分层策略；5) 关注RISC-V 64位平台上WAMR/Wasmtime的进展

**其他信息**

- **id**: WABI

---

### WASMEXT

**基本信息**

- **Title**: Safe Kernel Extensibility and Instrumentation With WebAssembly
- **Conference**: CMU Technical Report (CMU-CS-25-123)
- **Year**: 2025
- **Authors**: Faisal Abdelmonem（卡内基梅隆大学计算机科学系，硕士论文）
- **Institution**: 卡内基梅隆大学计算机科学系（Carnegie Mellon University, Computer Science Department）
- **Status**: confirmed

**技术方向**

- **Paper Theme**: kernel_extension
- **Key Idea**: 提出WebAssembly作为内核安全可扩展性的中间地带——相比于内核模块（强大但危险）和eBPF（安全但表达受限），WASM提供更强的隔离保证（SFI沙箱、形式化规范）、语言无关性、以及更丰富的表达能力。原型实现允许动态加载/卸载WASM二进制到Linux内核，通过kprobes和syscall hook实现系统调用的拦截与插桩。
- **Keywords**: WebAssembly, 内核可扩展性, eBPF, syscall hook, 沙箱隔离, 动态插桩
- **Technical Contribution Type**: novel_design

**TrainOS相关性**

- **Relevance**: high
- **Applicable Techniques**: 1) WASM作为内核模块与eBPF之间的中间地带：为TrainOS V24（内核扩展）和V28（WASM/WASI）提供融合设计思路；2) kprobe/syscall hook插桩机制：适用于TrainOS微内核的IPC端点hook和syscall dispatch hook；3) WASM内核模块动态加载/卸载：与TrainOS已有能力系统的权限控制结合；4) 线性内存沙箱隔离在微内核环境中的应用：WASM线性内存天然隔离性可作为进程内沙箱的补充；5) WASM与eBPF的优劣势对比框架：为TrainOS选择扩展机制提供决策依据；6) 三个测试用例（counter/mkdir mode capture/accept caching）的设计模式可作为TrainOS扩展点验证模板
- **Roadmap Mapping**: cross_cutting
- **Compatibility With Rust**: rust_compatible
- **Implementation Readiness**: concept_only

**影响力与成熟度**

- **Maturity**: research
- **Citations Impact**: too_early
- **Ecosystem Relevance**: niche_specific

**行动指南**

- **Effort Estimate**: 12-16人周（基于该论文概念在TrainOS上实现原型：WASM内核扩展框架设计3-4周、宿主函数接口实现3-4周、hook点集成2-3周、安全评估与测试2-3周、与已有V24 eBPF实现的融合设计2周）
- **Time Sensitivity**: moderately_time_sensitive
- **Engagement Level**: discuss_in_team
- **Risks And Caveats**: 1) 该硕士论文原型非常早期，仅实现3个基础测试用例（counter/mkdir mode capture/accept under reverse proxy），距离生产可用差距很大；2) Riptides 2025年生产实践表明，内核内WASM运行时面临严重的内存管理（vmalloc碎片、OOM panic）、调试困难（内核panic缺少WASM执行上下文）、安全攻击面（WASM运行时bug可危及内核）和维护负担问题，最终放弃内核内WASM方案转回用户态混合架构；3) 2025年行业共识趋向eBPF做热路径hook + 用户态WASM做复杂逻辑的混合架构，而非纯内核内WASM；4) TrainOS微内核架构与Linux宏内核差异显著，WASM内核模块的IPC集成需要额外设计；5) 缺少RISC-V平台上的内核内WASM运行时实现经验
- **Suggested Next Step**: 1) 获取完整论文PDF（CMU-CS-25-123），深入阅读架构设计和评估细节；2) 团队讨论：对比TrainOS已有V24 eBPF实现和V28 WASM运行时，评估WASM内核扩展与eBPF内核扩展的融合方案——可能的路径是eBPF做热路径hook + WASM（用户态）做复杂策略执行，类似2025年行业共识的混合架构；3) 调研Riptides经验教训（riptides.io/blog），避免内核内WASM已知陷阱；4) 评估Hyperlight-Wasm（微软，ICFP/SPLASH 2025）硬件虚拟化增强隔离的方案是否更适合TrainOS；5) 考虑将WASM扩展点放在用户态服务而非内核本身，利用微内核的IPC机制实现安全通信

**其他信息**

- **id**: WASMEXT

**不确定字段**: open_source, repo_url

---

## 交叉分析

### 按路线图阶段

- **V21_verification**: [CORTENMM](#cortenmm), [SQFS](#sqfs)
- **V27_defense_depth**: [FINEMEM](#finemem)
- **V28_wasm**: [WABI](#wabi)
- **V29_ai_os**: [AIOS_TREND](#aios_trend)
- **cross_cutting**: [HONGMENG](#hongmeng), [RFL](#rfl), [WASMEXT](#wasmext)
- **other**: [CONFIDENTIAL](#confidential), [EAGLE_TEEM3](#eagle_teem3), [EBPFUN](#ebpfun), [EBPFVER](#ebpfver)

### 按相关性

- **高相关性 (8项)**: [AIOS_TREND](#aios_trend) | [CORTENMM](#cortenmm) | [FINEMEM](#finemem) | [HONGMENG](#hongmeng) | [RFL](#rfl) | [SQFS](#sqfs) | [WABI](#wabi) | [WASMEXT](#wasmext)

### 按技术主题

- **AI+OS协同** (1项): [AIOS_TREND](#aios_trend)
- **分解内存** (1项): [FINEMEM](#finemem)
- **文件/存储** (1项): [SQFS](#sqfs)
- **内核扩展** (1项): [WASMEXT](#wasmext)
- **内存管理** (1项): [CORTENMM](#cortenmm)
- **微内核设计** (1项): [HONGMENG](#hongmeng)
- **other** (4项): [CONFIDENTIAL](#confidential), [EAGLE_TEEM3](#eagle_teem3), [EBPFUN](#ebpfun), [EBPFVER](#ebpfver)
- **Rust语言级** (1项): [RFL](#rfl)
- **WebAssembly** (1项): [WABI](#wabi)

## TrainOS 下一阶段建议

基于本次调研，建议下一阶段演进方向：

### V31 — 内存架构重构 (CortenMM 启发)
- 引入单层(One-Level)内存管理，消除 Sv39 页表之上的 VMA 软件抽象
- 使用 Verus 形式化验证并发页表操作的正确性
- 参考 Asterinas vostd 验证框架，建立 TrainOS 形式化验证基础设施

### V32 — WASM 运行时增强 (WABI + WASMEXT 启发)
- 将 WASM syscall 暴露为 host function，不破坏沙箱隔离
- 采用 eBPF(热路径 hook) + 用户态 WASM(复杂策略) 混合架构
- 参考 WALI 的 137 syscall 兼容列表和名称绑定机制

### V33 — 机密计算扩展 (TEEM³ + Confidential Computing Trend 启发)
- 基于 RISC-V Keystone/PMP 实现最小 TCB TEE
- 参考 TEEM³ 核心独立 TEE 设计，支持异构硬件(CPU + AI 加速器)
- 微内核 TCB 天然小而可审计，是机密计算的理想基础

### V34 — AI 原生调度增强 (AI-OS Trend 启发)
- 实现 P/D(预填充/解码)分离架构，映射到微内核独立服务
- KV-cache 分页管理，利用已有页表机制管理 GPU 缓存
- GPU-CPU 异构推理调度，利用 V25 的 NUMA 感知基础设施

## 快速对比矩阵

| 论文 | 成熟度 | Rust兼容 | 可实施性 | 时效性 | 建议行动 |
|------|--------|----------|---------|--------|---------|
| [AIOS_TREND](#aios_trend) | research | rust_compatible | needs_prototyping | highly_time_sensitive | prototype |
| [CONFIDENTIAL](#confidential) |  |  |  |  |  |
| [CORTENMM](#cortenmm) | prototype | native_rust | ready_for_adoption | highly_time_sensitive | engage_community |
| [EAGLE_TEEM3](#eagle_teem3) |  |  |  |  |  |
| [EBPFUN](#ebpfun) |  |  |  |  |  |
| [EBPFVER](#ebpfver) |  |  |  |  |  |
| [FINEMEM](#finemem) | research | rust_compatible | needs_prototyping | moderately_time_sensitive | prototype |
| [HONGMENG](#hongmeng) | production | neutral | needs_adaptation | long_tail | read_only |
| [RFL](#rfl) | research | native_rust | ready_for_adoption | long_tail | read_only |
| [SQFS](#sqfs) | research | native_rust | needs_adaptation | long_tail | discuss_in_team |
| [WABI](#wabi) | prototype | rust_compatible | needs_adaptation | moderately_time_sensitive | prototype |
| [WASMEXT](#wasmext) | research | rust_compatible | concept_only | moderately_time_sensitive | discuss_in_team |

---

*报告由 research-report skill 自动生成 | 数据来源: web search + deep research agents*