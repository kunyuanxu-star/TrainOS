.PHONY: all kernel services test clean copy-elfs run

QEMU = qemu-system-riscv64
BIOS = rustsbi-qemu-new.bin
KERNEL = target/riscv64gc-unknown-none-elf/release/kernel
QEMU_FLAGS = -machine virt -smp 2 -nographic

# Build everything in order: services -> copy ELFs -> kernel
all: services copy-elfs kernel

kernel:
	cargo build --release -p kernel

# Build all user-space services
services:
	cargo build --release -p init -p ping -p fs -p test_fs -p sh \
	  -p test_fork -p test_posix -p test_posix2 -p drv -p net -p echo -p test_net \
	  -p proc -p test_proc -p demo -p stress -p bb \
	  -p pci -p veth -p tfs -p tfs_jrnl -p edit -p cat \
	  -p reg -p netdrv -p test_sdp -p test_inv -p test_perf \
	  -p test_clib -p test_edit -p test_arp -p test_cap \
	  -p uart -p test_tfs -p rustdemo -p pkg -p test_pkg -p test_net2 \
	  -p mkfs -p test_mount -p test_sig -p test_exec \
	  -p test_smp -p test_shm -p http -p test_http \
	  -p tcp -p selftest

# Copy service ELF binaries into kernel/src/ for embedding
copy-elfs:
	@echo "Copying service ELFs to kernel/src/..."
	@for elf in target/riscv64gc-unknown-none-elf/release/*; do \
		base=$$(basename "$$elf"); \
		if [ -f "$$elf" ] && file "$$elf" | grep -q ELF && [ ! -d "kernel/src/$$base" ]; then \
			cp "$$elf" "kernel/src/$$base.elf" 2>/dev/null; \
		fi; \
	done
	@echo "Done."

# Run on QEMU (interactive)
run: all
	$(QEMU) $(QEMU_FLAGS) -bios $(BIOS) -kernel $(KERNEL)

# Run the test suite
test: all
	@echo "Running TrainOS test suite on QEMU..."
	@timeout 30 $(QEMU) $(QEMU_FLAGS) \
	  -bios $(BIOS) \
	  -kernel $(KERNEL) 2>&1 | tee /tmp/trainos_test.log || true
	@echo ""
	@echo "=== Test Results ==="
	@if grep -q "TrainOS booting\|System ready\|READY" /tmp/trainos_test.log; then \
		echo "  [PASS] System booted successfully"; \
	else \
		echo "  [FAIL] System did not boot"; \
	fi
	@if grep -q "ALL TESTS PASSED" /tmp/trainos_test.log; then \
		echo "  [PASS] All self-tests passed"; \
	fi
	@if grep -q "PANIC" /tmp/trainos_test.log; then \
		echo "  [FAIL] Kernel panic detected"; \
	else \
		echo "  [PASS] No kernel panics"; \
	fi
	@echo "Done."

clean:
	cargo clean
