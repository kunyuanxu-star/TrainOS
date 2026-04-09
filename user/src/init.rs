//! Init service - first user-space process
//!
//! Spawns driver and fs services in order:
//! 1. Spawn driver_server (handles VirtIO devices)
//! 2. Wait for driver to initialize
//! 3. Spawn fs_server (provides file system)
//! 4. Wait for fs to initialize
//! 5. Spawn shell

#![no_std]
#![no_main]

// Syscall numbers
const SYS_EXIT: usize = 93;

/// Write character
fn putchar(c: u8) {
    unsafe {
        core::arch::asm!("li a7, 1; mv a0, {0}; ecall", in(reg) c);
    }
}

/// Print string
fn print(s: &str) {
    for b in s.bytes() {
        putchar(b);
        if b == b'\n' {
            putchar(b'\r');
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() {
    print("init: TrainOS microkernel init\n");

    // In Phase 3/4:
    // 1. Create endpoint for receiving service PIDs from kernel
    // 2. Call sys_spawn(DRIVER_BIN) -> driver_pid
    // 3. Send driver endpoint capability to kernel
    // 4. Wait for driver ACK on init port
    // 5. Call sys_spawn(FS_BIN) -> fs_pid
    // 6. Send driver capability to fs (so fs can use block I/O)
    // 7. Call sys_spawn(SHELL_BIN)

    print("init: Service spawning will be added in Phase 4\n");
    print("init: For now, running shell\n");

    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}
