# TrainOS Service Development Tutorial

## Minimal Service

```rust
#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("Hello from my service!\r\n");
    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
```

## Building

1. Create `services/myservice/Cargo.toml` with tros dependency
2. Create `services/myservice/src/main.rs`
3. Add to workspace `Cargo.toml` members
4. Build: `cargo build --release -p myservice`
5. Copy ELF: `cp target/riscv64gc-unknown-none-elf/release/myservice kernel/src/myservice.elf`
6. Embed in `kernel/src/main.rs`: `static MY_ELF: &[u8] = include_bytes!("myservice.elf");`
7. Spawn: `proc::spawn(MY_ELF, priority);`
8. Rebuild kernel: `cargo build --release -p kernel`

## Available libtros Functions

See `lib/tros/src/lib.rs` for the full list. Key functions:
- `print/putchar/getchar` — Console I/O
- `ep_create/send/recv` — IPC
- `meminfo/perf_stats/cap_stats` — System info
- `blk_read/blk_write` — Block I/O
- `mmio_read32/mmio_write32` — MMIO access
- `open/read/write/close` — POSIX I/O
- `fork/exit/getpid/yield_cpu` — Process control
- `malloc/free/printf/strlen` — Mini C library
