//! Init service - first user-space process
//!
//! Spawns services in order: driver -> fs -> shell

#![no_std]
#![no_main]

// Syscall numbers
const SYS_SPAWN: usize = 1105;
const SYS_SEND: usize = 1002;
const SYS_RECV: usize = 1003;
const SYS_ENDPOINT_CREATE: usize = 1000;
const SYS_EXIT: usize = 93;
const SYS_SCHED_YIELD: usize = 124;

/// Write character to console
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

/// Print hex number
fn print_hex(val: usize) {
    let hex = b"0123456789abcdef";
    for i in (0..16).rev() {
        putchar(hex[(val >> (i * 4)) & 0xf as usize]);
    }
}

/// Make a syscall
fn syscall(n: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize) -> isize {
    let ret;
    unsafe {
        core::arch::asm!(
            "mv a7, {syscall_num}",
            "mv a0, {arg0}; mv a1, {arg1}; mv a2, {arg2}; mv a3, {arg3}; mv a4, {arg4}; mv a5, {arg5}",
            "ecall",
            lateout("a0") ret,
            arg0 = in(reg) a0,
            arg1 = in(reg) a1,
            arg2 = in(reg) a2,
            arg3 = in(reg) a3,
            arg4 = in(reg) a4,
            arg5 = in(reg) a5,
            syscall_num = in(reg) n,
        );
    }
    ret
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

#[no_mangle]
pub extern "C" fn _start() {
    print("init: Starting TrainOS services\n");

    // Create endpoint for init port (for service handshakes)
    let init_port = syscall(SYS_ENDPOINT_CREATE, 0, 0, 0, 0, 0, 0) as u32;
    print("init: Init port created: ");
    print_hex(init_port as usize);
    print("\n");

    // Step 1: Spawn driver service
    print("init: Spawning driver...\n");
    let driver_pid = syscall(SYS_SPAWN, 0, 0, 0, 0, 0, 0);
    if driver_pid < 0 {
        print("init: Failed to spawn driver\n");
    } else {
        print("init: Driver spawned with pid ");
        print_hex(driver_pid as usize);
        print("\n");
    }

    // Wait for driver to initialize (yield to let it run)
    for _ in 0..100 {
        syscall(SYS_SCHED_YIELD, 0, 0, 0, 0, 0, 0);
    }

    // Step 2: Spawn fs service
    print("init: Spawning fs...\n");
    let fs_pid = syscall(SYS_SPAWN, 1, 0, 0, 0, 0, 0);
    if fs_pid < 0 {
        print("init: Failed to spawn fs\n");
    } else {
        print("init: FS spawned with pid ");
        print_hex(fs_pid as usize);
        print("\n");
    }

    // Wait for fs to initialize
    for _ in 0..100 {
        syscall(SYS_SCHED_YIELD, 0, 0, 0, 0, 0, 0);
    }

    // Step 3: Spawn network service
    print("init: Spawning network...\n");
    let net_pid = syscall(SYS_SPAWN, 3, 0, 0, 0, 0, 0);
    if net_pid < 0 {
        print("init: Failed to spawn network\n");
    } else {
        print("init: Network spawned with pid ");
        print_hex(net_pid as usize);
        print("\n");
    }

    // Wait for network to initialize
    for _ in 0..100 {
        syscall(SYS_SCHED_YIELD, 0, 0, 0, 0, 0, 0);
    }

    // Step 4: Spawn VFS service
    print("init: Spawning vfs...\n");
    let vfs_pid = syscall(SYS_SPAWN, 4, 0, 0, 0, 0, 0);
    if vfs_pid < 0 {
        print("init: Failed to spawn vfs\n");
    } else {
        print("init: VFS spawned with pid ");
        print_hex(vfs_pid as usize);
        print("\n");
    }

    // Wait for vfs to initialize
    for _ in 0..100 {
        syscall(SYS_SCHED_YIELD, 0, 0, 0, 0, 0, 0);
    }

    // Step 5: Spawn shell
    print("init: Spawning shell...\n");
    let shell_pid = syscall(SYS_SPAWN, 2, 0, 0, 0, 0, 0);
    if shell_pid < 0 {
        print("init: Failed to spawn shell\n");
    } else {
        print("init: Shell spawned with pid ");
        print_hex(shell_pid as usize);
        print("\n");
    }

    print("init: All services launched\n");

    // Init can exit now - services are running
    syscall(SYS_EXIT, 0, 0, 0, 0, 0, 0);

    // Should never reach here, but just in case
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}