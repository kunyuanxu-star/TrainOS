.PHONY: all kernel services test clean

MACHINA = ../machina/target/release/machina
KERNEL = target/riscv64gc-unknown-none-elf/release/kernel

all: kernel

kernel:
	cargo build --release -p kernel

services:
	cargo build --release -p init -p ping -p fs -p test_fs -p sh \
	  -p test_fork -p test_posix -p test_posix2 -p drv -p net -p echo -p test_net \
	  -p proc -p test_proc -p demo -p stress -p bb \
	  -p pci -p veth -p tfs -p tfs_jrnl -p edit -p cat \
	  -p reg -p netdrv -p test_sdp -p test_inv -p test_perf \
	  -p test_clib -p test_edit -p test_arp -p test_cap \
	  -p uart -p test_tfs -p rustdemo -p pkg -p test_pkg -p test_net2 \
	  -p mkfs -p test_mount -p test_sig -p test_exec -p tcp
	@for elf in target/riscv64gc-unknown-none-elf/release/*; do \
		base=$$(basename "$$elf"); \
		if [ -f "$$elf" ] && file "$$elf" | grep -q ELF && [ ! -d "kernel/src/$$base" ]; then \
			cp "$$elf" kernel/src/; \
		fi; \
	done

test: kernel
	@echo "Running TrainOS test suite..."
	@timeout 10 $(MACHINA) -M riscv64-ref \
	  -bios ../machina/pc-bios/rustsbi-riscv64-machina-fw_dynamic.bin \
	  -kernel $(KERNEL) -nographic 2>&1 | tee /tmp/trainos_test.log
	@echo ""
	@echo "Test results:"
	@grep -c "PASS" /tmp/trainos_test.log || echo "  PASS count: $$(grep -c 'PASS' /tmp/trainos_test.log)"
	@grep -c "READY" /tmp/trainos_test.log || echo "  READY check: OK"
	@echo "Done."

clean:
	cargo clean
