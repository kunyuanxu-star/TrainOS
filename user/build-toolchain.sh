#!/bin/bash
# build-toolchain.sh - Download and build RISC-V GCC toolchain for TrainOS
# This script sets up the complete development environment

set -e

TRAINOS_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RISCV_VERSION="2024.02.10"
RISCV_INSTALL_DIR="${TRAINOS_DIR}/toolchain"
RISCV_ARCHIVE="riscv64-unknown-elf-x86_64-${RISCV_VERSION}.tar.gz"
RISCV_URL="https://github.com/stnolting/riscv-gcc/releases/download/prebuilt-${RISCV_VERSION}/${RISCV_ARCHIVE}"

echo "=========================================="
echo "TrainOS RISC-V Toolchain Setup"
echo "=========================================="
echo ""

# Check for existing toolchain
check_toolchain() {
    if command -v riscv64-unknown-elf-gcc &> /dev/null; then
        echo "Found system RISC-V GCC:"
        riscv64-unknown-elf-gcc --version | head -1
        return 0
    fi

    if [ -d "${RISCV_INSTALL_DIR}/bin" ] && [ -f "${RISCV_INSTALL_DIR}/bin/riscv64-unknown-elf-gcc" ]; then
        echo "Found local RISC-V GCC at ${RISCV_INSTALL_DIR}"
        export PATH="${RISCV_INSTALL_DIR}/bin:$PATH"
        riscv64-unknown-elf-gcc --version | head -1
        return 0
    fi

    return 1
}

# Try to find existing toolchain
if check_toolchain; then
    echo "Toolchain already available!"
    exit 0
fi

echo "No RISC-V GCC toolchain found."
echo ""
echo "Options:"
echo "  1) Download prebuilt toolchain (recommended)"
echo "  2) Build from source (takes 1-2 hours)"
echo "  3) Install via package manager"
echo ""

read -p "Choose option [1-3]: " choice
case $choice in
    1)
        echo ""
        echo "Downloading prebuilt toolchain..."
        echo "URL: ${RISCV_URL}"
        echo ""

        mkdir -p "${TRAINOS_DIR}/downloads"
        cd "${TRAINOS_DIR}/downloads"

        if [ -f "${RISCV_ARCHIVE}" ]; then
            echo "Archive already downloaded."
        else
            echo "Downloading (this may take a while)..."
            curl -L -o "${RISCV_ARCHIVE}" "${RISCV_URL}" || {
                echo "Download failed. Trying alternative source..."
                # Try alternative source
                ALT_URL="https://github.com/stnolting/riscv-gcc/releases/download/prebuilt-rv64gc/lp64d/riscv64-unknown-elf-20240318.tar.gz"
                curl -L -o "${RISCV_ARCHIVE}" "${ALT_URL}" || {
                    echo "All downloads failed."
                    echo "Please install riscv64-unknown-elf-gcc via your package manager:"
                    echo "  Ubuntu/Debian: sudo apt-get install riscv64-unknown-elf-gcc"
                    echo "  Arch:          sudo pacman -S riscv64-linux-gnu-gcc"
                    exit 1
                }
            }
        fi

        echo ""
        echo "Extracting..."
        cd "${TRAINOS_DIR}"
        mkdir -p toolchain
        tar -xzf "downloads/${RISCV_ARCHIVE}" -C toolchain --strip-components=1

        echo ""
        echo "Verifying installation..."
        export PATH="${TRAINOS_DIR}/toolchain/bin:$PATH"
        if riscv64-unknown-elf-gcc --version | head -1; then
            echo ""
            echo "=========================================="
            echo "Toolchain installed successfully!"
            echo "=========================================="
            echo ""
            echo "Add to your PATH:"
            echo "  export PATH=\"${TRAINOS_DIR}/toolchain/bin:\$$PATH\""
            echo ""
            echo "Or add this to your ~/.bashrc or ~/.zshrc"
            echo ""
        else
            echo "Installation verification failed!"
            exit 1
        fi
        ;;

    2)
        echo ""
        echo "Building RISC-V GCC from source..."
        echo "This will take 1-2 hours depending on your machine."
        echo ""
        read -p "Continue? [y/N]: " confirm
        if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
            echo "Cancelled."
            exit 0
        fi

        # Install dependencies
        echo "Installing build dependencies..."
        if command -v apt-get &> /dev/null; then
            sudo apt-get install -y \
                build-essential \
                autoconf \
                automake \
                autotools-dev \
                libmpc-dev \
                libmpfr-dev \
                libgmp-dev \
                libisl-dev \
                zlib1g-dev \
                flex \
                bison \
                python3 \
                texinfo
        elif command -v pacman &> /dev/null; then
            sudo pacman -S --noconfirm \
                base-devel \
                autoconf \
                automake \
                libmpc \
                mpfr \
                gmp \
                isl \
                zlib \
                flex \
                bison \
                python3 \
                texinfo
        fi

        # Clone and build
        echo "Cloning riscv-gnu-toolchain..."
        cd "${TRAINOS_DIR}"
        if [ -d "riscv-gnu-toolchain" ]; then
            echo "Repository already exists."
            cd riscv-gnu-toolchain
            git pull
        else
            git clone --recursive https://github.com/riscv/riscv-gnu-toolchain.git
            cd riscv-gnu-toolchain
        fi

        echo "Configuring..."
        mkdir -p build
        cd build
        ../configure --prefix="${RISCV_INSTALL_DIR}" \
            --with-arch=rv64gc \
            --with-abi=lp64d \
            --with-cmodel=medany \
            --disable-multilib

        echo "Building (this may take a long time)..."
        make -j$(nproc)

        echo ""
        echo "Toolchain built successfully!"
        ;;

    3)
        echo ""
        echo "Installing via package manager..."
        echo ""

        if command -v apt-get &> /dev/null; then
            echo "Ubuntu/Debian:"
            echo "  sudo apt-get update"
            echo "  sudo apt-get install riscv64-unknown-elf-gcc riscv64-unknown-elf-binutils riscv64-unknown-elf-newlib"
            echo ""
        fi

        if command -v pacman &> /dev/null; then
            echo "Arch Linux:"
            echo "  sudo pacman -S riscv64-linux-gnu-gcc riscv64-linux-gnu-binutils"
            echo ""
        fi

        if command -v brew &> /dev/null; then
            echo "macOS (Homebrew):"
            echo "  brew install riscv-gnu-toolchain"
            echo ""
        fi

        echo "After installation, run 'make check' to verify."
        ;;

    *)
        echo "Invalid option."
        exit 1
        ;;
esac

echo ""
echo "Now you can build C programs for TrainOS with:"
echo "  cd ${TRAINOS_DIR}/user"
echo "  make hello"
echo ""
