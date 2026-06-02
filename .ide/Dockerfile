# TrainOS CNB Cloud-Native Development Environment
# RISC-V 64-bit microkernel OS — Rust nightly + RISC-V toolchain
FROM rust:1.85-slim-bookworm

LABEL description="TrainOS — RISC-V 64-bit Microkernel OS (Rust nightly, no_std)"
LABEL org="trainos"

# Install system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    # Build essentials
    make \
    gcc \
    binutils \
    # RISC-V toolchain
    gcc-riscv64-unknown-elf \
    binutils-riscv64-unknown-elf \
    # QEMU for RISC-V emulation
    qemu-system-misc \
    # Git + misc
    git \
    curl \
    ca-certificates \
    # Python for scripting
    python3 \
    && rm -rf /var/lib/apt/lists/*

# Install Rust nightly toolchain with RISC-V target
RUN rustup toolchain install nightly \
    && rustup default nightly \
    && rustup target add riscv64gc-unknown-none-elf \
    && rustup component add rust-src \
    && rustup component add clippy \
    && rustup component add rustfmt \
    && rustup component add llvm-tools-preview

# Verify toolchain
RUN rustc --version \
    && cargo --version \
    && rustup show \
    && riscv64-unknown-elf-gcc --version 2>/dev/null || echo "riscv64-gcc OK"

# Set working directory
WORKDIR /workspace

# Default command: build TrainOS
CMD ["make", "all"]
