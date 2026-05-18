# Contributing to TrainOS

TrainOS is a microkernel operating system written in Rust for RISC-V 64-bit. We welcome contributions!

## Getting Started

### Prerequisites
- Rust nightly toolchain
- RISC-V target: `rustup target add riscv64gc-unknown-none-elf`
- Rust components: `rustup component add rust-src clippy rustfmt`
- [machina](https://github.com/gevico/machina) emulator (for testing)

### Build
```bash
# Build all services
make services

# Build kernel (includes embedded service binaries)
make kernel

# Or build everything
make all
```

### Run
```bash
./machina/target/release/machina \
  -M riscv64-ref -smp 2 \
  -bios rustsbi-riscv64-machina-fw_dynamic.bin \
  -kernel target/riscv64gc-unknown-none-elf/release/kernel \
  -nographic
```

## Project Structure

```
TrainOS/
├── kernel/src/          # Microkernel (~3500 lines)
│   ├── main.rs          # Boot sequence, service spawning
│   ├── mem/             # Buddy allocator, Sv39 page tables
│   ├── proc/            # Process/thread management, ELF loader
│   ├── sched/           # 64-priority SMP scheduler
│   ├── cap/             # Capability-based access control
│   ├── ipc/             # IPC endpoints, message passing
│   ├── trap/            # Trap handling, interrupts, page faults
│   ├── syscall/         # Syscall dispatch (83 syscalls)
│   ├── ns/              # Namespace subsystem
│   └── device/          # Device driver framework
├── services/            # User-space services (35+)
│   ├── init/            # System init (pid 1)
│   ├── fs/              # VFS with procfs (EP 2)
│   ├── net/             # Network stack (EP 3)
│   ├── tcp/             # TCP reliable stream protocol
│   ├── sh/              # Interactive shell
│   └── selftest/        # System self-test suite
├── lib/tros/            # User-space syscall library
└── docs/                # Documentation
```

## Development Workflow

1. Fork the repository
2. Create a feature branch: `git checkout -b feat/your-feature`
3. Make changes following our conventions
4. Run `cargo fmt --all --check` and `cargo clippy`
5. Build and test with `make all`
6. Commit using conventional commits format
7. Push and open a Pull Request

## Commit Convention

We use conventional commits:
- `feat:` — New feature
- `fix:` — Bug fix
- `docs:` — Documentation
- `refactor:` — Code restructuring
- `test:` — Test additions/changes

Example: `feat: add TCP retransmission timer`

## Code Style

- **No heap in services**: User-space services use static allocation only
- **No_std everywhere**: Both kernel and services use `#![no_std]`
- **Minimal comments**: Code should be self-documenting through good naming
- **Prefer simple over clever**: Three similar lines > one premature abstraction
- **64-byte IPC limit**: Messages fit in a single cache line

## Architecture Rules (Iron Rules)

1. **Runtime**: RustSBI (M-mode) + machina (RISC-V JIT emulator). Non-negotiable.
2. **Language**: Rust nightly (`no_std` kernel + user-space)
3. **Architecture**: RISC-V 64-bit (rv64gc), Sv39 virtual memory
4. **License**: MIT

## Testing

- **Self-test**: The `selftest` service runs 15 subsystem tests on boot
- **CI**: GitHub Actions builds all services and runs system test on machina
- **Manual**: Run `make test` to execute the full test suite locally

## Adding a New Syscall

1. Add syscall number constant in `kernel/src/syscall/mod.rs`
2. Implement handler in the appropriate `kernel/src/syscall/*.rs` module
3. Add dispatch entry in `syscall_dispatch()`
4. Add user-space wrapper in `lib/tros/src/lib.rs`
5. Document in `docs/syscalls.md`

## Adding a New Service

1. Create `services/yourname/Cargo.toml` and `services/yourname/src/main.rs`
2. Add to workspace `members` in root `Cargo.toml`
3. Build: `cargo build --release -p yourname`
4. Copy ELF: `cp target/riscv64gc-unknown-none-elf/release/yourname kernel/src/yourname.elf`
5. Add `spawn_service!(yourname, priority)` in `kernel/src/main.rs`

## Questions?

Open an issue or discussion on GitHub.
