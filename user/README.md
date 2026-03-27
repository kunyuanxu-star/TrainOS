# TrainOS GCC 支持

本目录包含 TrainOS 的 C 程序支持，包括:

- C 标准库头文件 (`include/`)
- 链接脚本 (`link.ld`)
- 示例 C 程序 (`hello.c`, `test.c`)
- Makefile 用于构建

## 安装 RISC-V GCC 工具链

### Ubuntu / Debian

```bash
sudo apt-get update
sudo apt-get install -y \
    riscv64-unknown-elf-gcc \
    riscv64-unknown-elf-binutils \
    riscv64-unknown-elf-newlib
```

### Fedora / RHEL / CentOS

```bash
sudo dnf install \
    riscv64-linux-gnu-gcc \
    riscv64-linux-gnu-binutils
```

### Arch Linux

```bash
sudo pacman -S \
    riscv64-linux-gnu-gcc \
    riscv64-linux-gnu-binutils
```

### macOS (Homebrew)

```bash
brew install riscv-gnu-toolchain
```

### 从源码编译

如果包管理器没有 RISC-V 工具链，可以从源码编译:

```bash
# 安装依赖
sudo apt-get install -y \
    build-essential \
    autoconf \
    automake \
    autotools-dev \
    libmpc-dev \
    libmpfr-dev \
    libgmp-dev \
    libisl-dev \
    zlib1g-dev

# 克隆并编译
git clone https://github.com/riscv/riscv-gnu-toolchain.git
cd riscv-gnu-toolchain
mkdir build && cd build
../configure --prefix=/opt/riscv --with-arch=rv64gc --with-abi=lp64d
make -j$(nproc)
```

## 构建 C 程序

```bash
# 检查工具链
make check

# 构建 hello 程序
make hello

# 构建 test 程序
make test

# 构建所有 Rust 程序
make rust

# 清理
make clean
```

## C 程序示例

### hello.c

```c
#include <stdio.h>

int main(int argc, char* argv[]) {
    printf("Hello from TrainOS C program!\n");
    return 0;
}
```

### 使用系统调用

```c
#include <unistd.h>

int main() {
    write(STDOUT_FILENO, "Hello!\n", 7);
    return 0;
}
```

## 文件结构

```
user/
├── include/          # C 标准库头文件
│   ├── stdio.h
│   ├── stdlib.h
│   ├── string.h
│   ├── unistd.h
│   ├── signal.h
│   ├── fcntl.h
│   ├── time.h
│   ├── errno.h
│   ├── stdarg.h
│   ├── stddef.h
│   ├── limits.h
│   ├── ctype.h
│   └── sys/
│       ├── types.h
│       ├── stat.h
│       ├── wait.h
│       └── socket.h
├── libc/             # C 标准库实现
├── hello.c           # Hello World 示例
├── test.c            # 测试程序
├── syscall.S         # 系统调用封装
├── link.ld           # 链接脚本
├── Makefile         # 构建文件
└── README.md         # 本文档
```

## 注意事项

1. TrainOS 用户空间目前不支持完整的文件系统
2. 一些系统调用可能返回错误或被简化实现
3. 内存管理使用简单的 sbrk() 接口
4. 建议使用 Rust 编写更复杂用户程序 (参考 shell.rs, vi.rs)
