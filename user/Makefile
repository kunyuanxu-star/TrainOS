# TrainOS GCC Cross-Compilation Makefile
# This Makefile builds C programs for TrainOS using RISC-V GCC

# ===========================================
# Configuration
# ===========================================

# Toolchain - set RISCV environment variable or override here
RISCV ?= riscv64-unknown-elf
GCC = $(RISCV)-gcc
AS = $(RISCV)-as
LD = $(RISCV)-ld
OBJCOPY = $(RISCV)-objcopy
OBJDUMP = $(RISCV)-objdump
STRIP = $(RISCV)-strip

# Target architecture
ARCH = rv64gc
ABI = lp64d
MARCH = rv64imafdc_zba_zbb
MTUNE = rocket

# Paths
USER_DIR := $(shell pwd)
OS_DIR := $(USER_DIR)/../os
BUILD_DIR := $(USER_DIR)/build
TARGET_DIR := $(OS_DIR)/target/riscv64gc-unknown-none/release
INCLUDE_DIR := $(USER_DIR)/include

# C library sources
LIBC_DIR := $(USER_DIR)/libc
LIBC_SRCS := $(wildcard $(LIBC_DIR)/*.c)
LIBC_OBJS := $(patsubst $(LIBC_DIR)/%.c, $(BUILD_DIR)/libc/%.o, $(LIBC_SRCS))

# Syscall assembly
SYSCALL_SRC := $(USER_DIR)/syscall.S
SYSCALL_OBJ := $(BUILD_DIR)/syscall.o

# Startup code
CRT0_SRC := $(USER_DIR)/crt0.s
CRT0_OBJ := $(BUILD_DIR)/crt0.o

# ===========================================
# Compiler and Linker Flags
# ===========================================

# C flags
CFLAGS := -march=$(ARCH) -mabi=$(ABI) -Wall -O2
CFLAGS += -fno-builtin -fno-stack-protector -fnoPIC
CFLAGS += -I$(INCLUDE_DIR)
CFLAGS += -D__TRAINOS__ -D__riscv__ -D__riscv64

# Linker flags
LDFLAGS := -static -no-pie -T $(USER_DIR)/link.ld
LDFLAGS += -Map=$(BUILD_DIR)/$(TARGET).map

# Assembly flags
ASFLAGS := -march=$(ARCH) -mabi=$(ABI)

# ===========================================
# Build Targets
# ===========================================

.PHONY: all clean info check help \
	hello test benchmark shell \
	kernel rust \
	disasm hex binary \
	install run qemu

# Default target: build hello
all: hello

# Check if toolchain is available
check:
	@if ! command -v $(GCC) > /dev/null 2>&1; then \
		echo "Error: RISC-V GCC toolchain not found!"; \
		echo "Please install riscv64-unknown-elf-gcc and add it to PATH"; \
		echo ""; \
		echo "Installation options:"; \
		echo "  Ubuntu/Debian: sudo apt-get install riscv64-unknown-elf-gcc"; \
		echo "  Or download from: https://github.com/riscv/riscv-gnu-toolchain"; \
		echo ""; \
		echo "You can also set RISCV variable to point to your toolchain:"; \
		echo "  make RISCV=/path/to/toolchainPrefix hello"; \
		exit 1; \
	fi
	@echo "Toolchain found: $$($(GCC) --version | head -1)"

# Create build directory
$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)
	mkdir -p $(BUILD_DIR)/libc

# ===========================================
# C Library Build
# ===========================================

$(BUILD_DIR)/libc/%.o: $(LIBC_DIR)/%.c | $(BUILD_DIR)
	@echo "  CC      $@"
	$(GCC) $(CFLAGS) -c $< -o $@

# ===========================================
# Syscall Assembly
# ===========================================

$(SYSCALL_OBJ): $(SYSCALL_SRC) | $(BUILD_DIR)
	@echo "  AS      $@"
	$(GCC) $(ASFLAGS) -c $< -o $@

# ===========================================
# Startup Code
# ===========================================

$(CRT0_OBJ): $(CRT0_SRC) | $(BUILD_DIR)
	@echo "  AS      $@"
	$(GCC) $(ASFLAGS) -c $< -o $@

# ===========================================
# User Programs
# ===========================================

# Hello world
hello: check $(LIBC_OBJS) $(SYSCALL_OBJ) $(CRT0_OBJ)
	@echo "  CC      hello.c"
	$(GCC) $(CFLAGS) -c $(USER_DIR)/hello.c -o $(BUILD_DIR)/hello.o
	@echo "  LD      $@"
	$(LD) $(LDFLAGS) -o $(BUILD_DIR)/hello \
		$(CRT0_OBJ) \
		$(BUILD_DIR)/hello.o \
		$(LIBC_OBJS) \
		$(SYSCALL_OBJ)
	$(OBJCOPY) -O binary $(BUILD_DIR)/hello $(BUILD_DIR)/hello.bin
	@echo "  Built:  $(BUILD_DIR)/hello"

# Test program
test: check $(LIBC_OBJS) $(SYSCALL_OBJ) $(CRT0_OBJ)
	@echo "  CC      test.c"
	$(GCC) $(CFLAGS) -c $(USER_DIR)/test.c -o $(BUILD_DIR)/test.o
	@echo "  LD      $@"
	$(LD) $(LDFLAGS) -o $(BUILD_DIR)/test \
		$(CRT0_OBJ) \
		$(BUILD_DIR)/test.o \
		$(LIBC_OBJS) \
		$(SYSCALL_OBJ)
	$(OBJCOPY) -O binary $(BUILD_DIR)/test $(BUILD_DIR)/test.bin
	@echo "  Built:  $(BUILD_DIR)/test"

# Benchmark program
benchmark: check $(LIBC_OBJS) $(SYSCALL_OBJ) $(CRT0_OBJ)
	@echo "  CC      benchmark.c"
	$(GCC) $(CFLAGS) -c $(USER_DIR)/benchmark.c -o $(BUILD_DIR)/benchmark.o 2>/dev/null || \
		echo "benchmark.c not found, skipping"
	$(LD) $(LDFLAGS) -o $(BUILD_DIR)/benchmark \
		$(CRT0_OBJ) \
		$(BUILD_DIR)/benchmark.o \
		$(LIBC_OBJS) \
		$(SYSCALL_OBJ) 2>/dev/null || true
	@echo "  Built:  $(BUILD_DIR)/benchmark"

# ===========================================
# Rust Programs (existing)
# ===========================================

kernel:
	cd $(OS_DIR) && cargo build --release
	@echo "Kernel built: $(TARGET_DIR)/trainos"

rust:
	cd $(USER_DIR) && cargo build
	@echo "Rust user programs built"

# ===========================================
# Output Formats
# ===========================================

# Disassemble binary
disasm: $(BUILD_DIR)/hello
	@echo "Disassembly:"
	$(OBJDUMP) -d $(BUILD_DIR)/hello | head -100

# Show hex dump
hex: $(BUILD_DIR)/hello.bin
	@echo "Hex dump:"
	hexdump -C $(BUILD_DIR)/hello.bin | head -50

# Show symbols
symbols: $(BUILD_DIR)/hello
	@echo "Symbols:"
	$(RISCV)-nm $(BUILD_DIR)/hello | head -30

# ===========================================
# Installation
# ===========================================

# Copy built binaries to os directory
install: hello test
	cp $(BUILD_DIR)/hello.bin $(OS_DIR)/bin/hello 2>/dev/null || \
		mkdir -p $(OS_DIR)/bin && cp $(BUILD_DIR)/hello.bin $(OS_DIR)/bin/
	cp $(BUILD_DIR)/test.bin $(OS_DIR)/bin/test 2>/dev/null || true
	@echo "Installed to $(OS_DIR)/bin/"

# ===========================================
# QEMU Testing
# ===========================================

# Run in QEMU (requires QEMU with RISC-V support)
run: all
	@echo "Starting QEMU with RISC-V..."
	@echo "Note: You need to have built the kernel and disk image first"
	qemu-system-riscv64 \
		-machine virt \
		-bios $(OS_DIR)/../rustsbi-qemu.bin \
		-kernel $(TARGET_DIR)/trainos \
		-drive file=$(OS_DIR)/disk.img,format=raw,if=sd \
		-serial mon:stdio \
		-smp 2 \
		-m 2G

# ===========================================
# Cleanup
# ===========================================

clean:
	rm -rf $(BUILD_DIR)
	cd $(USER_DIR) && cargo clean

clean-all: clean
	cd $(OS_DIR) && cargo clean

# ===========================================
# Information
# ===========================================

info:
	@echo "TrainOS C Compilation Info"
	@echo "==========================="
	@echo "Toolchain prefix:  $(RISCV)"
	@echo "GCC:               $$($(GCC) --version | head -1)"
	@echo "Architecture:      $(ARCH)"
	@echo "ABI:               $(ABI)"
	@echo "MARCH:             $(MARCH)"
	@echo "Build dir:         $(BUILD_DIR)"
	@echo "Include dir:       $(INCLUDE_DIR)"

# ===========================================
# Help
# ===========================================

help:
	@echo "TrainOS C Program Build System"
	@echo "==============================="
	@echo ""
	@echo "Targets:"
	@echo "  all        - Build hello (default)"
	@echo "  hello      - Build hello world program"
	@echo "  test       - Build test program"
	@echo "  kernel     - Build TrainOS kernel"
	@echo "  rust       - Build Rust user programs"
	@echo ""
	@echo "Output formats:"
	@echo "  disasm     - Disassemble hello binary"
	@echo "  hex        - Show hex dump of hello.bin"
	@echo "  symbols    - Show symbol table"
	@echo ""
	@echo "Utilities:"
	@echo "  check      - Verify toolchain is installed"
	@echo "  info       - Show build configuration"
	@echo "  install    - Install binaries to OS directory"
	@echo "  clean      - Clean build artifacts"
	@echo ""
	@echo "Examples:"
	@echo "  make hello              # Build hello program"
	@echo "  make RISCV=riscv64-unknown-elf test"
	@echo "  make info               # Show toolchain info"
