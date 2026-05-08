#!/bin/bash
# TrainOS automated test runner
set -e

MACHINA="../machina/target/release/machina"
KERNEL="target/riscv64gc-unknown-none-elf/release/kernel"
TIMEOUT=10

echo "=== TrainOS Test Suite ==="
echo ""

# Build
echo "[1/3] Building kernel..."
cargo build --release -p kernel 2>&1 | tail -1

# Run
echo "[2/3] Running on machina..."
timeout $TIMEOUT $MACHINA -M riscv64-ref \
  -bios ../machina/pc-bios/rustsbi-riscv64-machina-fw_dynamic.bin \
  -kernel $KERNEL -nographic 2>&1 | tee /tmp/trainos.log

# Verify
echo ""
echo "[3/3] Verifying results..."

PASS_COUNT=$(grep -c "PASS" /tmp/trainos.log || echo 0)
READY=$(grep -c "READY" /tmp/trainos.log || echo 0)
DEMO=$(grep -c "All systems operational" /tmp/trainos.log || echo 0)

echo "  PASS count: $PASS_COUNT"
echo "  READY: $READY"
echo "  Demo: $DEMO"

if [ "$DEMO" -ge 1 ]; then
    echo ""
    echo "=== All tests passed! ==="
    exit 0
else
    echo ""
    echo "=== WARNING: Demo banner not found ==="
    exit 1
fi
