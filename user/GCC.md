# GCC 支持指南 - TrainOS

本文档说明如何在 TrainOS 上支持 GCC 编译器。

## 概述

GCC (GNU Compiler Collection) 是一个复杂的编译器套件。要在 TrainOS 上"支持 GCC"，有几种可能的理解：

1. **交叉编译**：在开发机器上使用 GCC 为 TrainOS 编译程序
2. **本地编译**：在 TrainOS 上运行 GCC 来编译程序

## 方案 1：交叉编译（推荐）

这是最实用的方案。开发者在主机上使用 RISC-V GCC 交叉编译器为 TrainOS 编译程序。

### 安装 RISC-V GCC

```bash
# 下载并运行工具链设置脚本
cd trainOS/user
chmod +x build-toolchain.sh
./build-toolchain.sh

# 或者手动安装 (Ubuntu/Debian):
sudo apt-get install riscv64-unknown-elf-gcc riscv64-unknown-elf-binutils riscv64-unknown-elf-newlib

# 或者从源码编译 (需要 1-2 小时):
git clone https://github.com/riscv/riscv-gnu-toolchain.git
cd riscv-gnu-toolchain
mkdir build && cd build
../configure --prefix=/opt/riscv --with-arch=rv64gc --with-abi=lp64d
make -j$(nproc)
```

### 编译 C 程序

```bash
cd trainOS/user

# 检查工具链
make check

# 编译 hello 程序
make hello

# 编译 test 程序
make test

# 查看构建信息
make info
```

### 构建产物

- ELF 文件: `build/hello`
- 原始二进制: `build/hello.bin`
- 反汇编: `make disasm`

### 链接脚本说明

`link.ld` 链接脚本配置：

```
MEMORY {
    RAM (wxa!ri) : ORIGIN = 0x00400000, LENGTH = 128M
}
```

用户程序从虚拟地址 `0x00400000` 开始，这是 Linux 兼容的用户空间基地址。

## 方案 2：本地编译（高级）

要在 TrainOS 上本地运行 GCC，需要：

### 1. 完整的文件系统支持

需要实现：
- VFS (虚拟文件系统) 层
- devfs (设备文件系统)
- 一个实际的文件系统 (ramfs, ext2, 等)

### 2. 多进程支持

GCC 编译过程调用多个程序：
```
gcc -> cpp (C预处理器) -> cc1 (C编译器) -> as (汇编器) -> ld (链接器)
```

需要实现：
- `fork()` - 创建新进程
- `execve()` - 执行新程序
- `wait4()` - 等待进程退出
- `pipe()` - 进程间通信

### 3. 完整的内存管理

- `mmap()` - 内存映射文件
- `mprotect()` - 内存保护
- `munmap()` - 解除内存映射
- 更多的堆内存 (`_sbrk` 已实现)

### 4. 获取 RISC-V GCC 二进制

需要为 RISC-V 架构构建或获取 GCC 的预编译二进制：
- riscv64-unknown-elf-gcc
- 包含 cpp, cc1, as, ld 等工具

### 简化方案：TinyCC

一个更现实的方案是移植 [TinyCC](https://bellard.org/tcc/) (Tiny C Compiler)：
- 更小的二进制 (约 150KB vs GCC 的 数十MB)
- 可以编译大多数 C 程序
- 可以作为单个程序运行

## 当前 TrainOS 状态

### 已实现

| 功能 | 状态 | 说明 |
|------|------|------|
| `_exit` | ✅ | 进程退出 |
| `_open` | ✅ | 打开文件 (基本) |
| `_read` | ✅ | 读文件 (基本) |
| `_write` | ✅ | 写文件 (stdout/stderr) |
| `_sbrk` | ✅ | 堆内存分配 |
| `_getpid` | ✅ | 获取进程 ID |
| `_gettimeofday` | ✅ | 获取时间 |
| 文件描述符表 | ✅ | stdin/stdout/stderr |

### 需要实现

| 功能 | 优先级 | 说明 |
|------|--------|------|
| 完整文件系统 | 高 | VFS, devfs, ramfs |
| `fork/exec` | 高 | 多进程支持 |
| `mmap` | 中 | 内存映射 |
| `pipe` | 中 | 进程通信 |
| 信号处理 | 低 | 进程间信号 |

## 下一步

1. **交叉编译验证**：
   ```bash
   cd trainOS/user
   make hello
   # 将 build/hello.bin 放到磁盘镜像
   ```

2. **增强 syscall 接口**：
   - 实现 `fork()` 和 `execve()`
   - 实现 `mmap()` 内存映射

3. **文件系统**：
   - 实现 RAM 文件系统
   - 将 hello.bin 加载到内存并执行

4. **最终目标**：
   - 在 TrainOS 上运行 GCC 交叉编译的 hello 程序
   - 然后可以尝试本地 GCC 编译

## 参考资料

- [RISC-V GCC](https://github.com/riscv/riscv-gnu-toolchain)
- [Newlib 文档](https://sourceware.org/newlib/)
- [GCC 内部设计](https://gcc.gnu.org/onlinedocs/gccint/)
- [QEMU RISC-V](https://www.qemu.org/docs/master/system/riscv/)
