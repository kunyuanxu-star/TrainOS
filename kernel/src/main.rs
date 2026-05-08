#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(test))]
static BOOT_READY: AtomicBool = AtomicBool::new(false);

use alloc::boxed::Box;

#[cfg(not(test))]
mod console;

#[cfg(not(test))]
mod mem;

#[cfg(not(test))]
mod trap;

#[cfg(not(test))]
mod sched;

#[cfg(not(test))]
mod per_cpu;

#[cfg(not(test))]
mod sync;

#[cfg(not(test))]
mod proc;

#[cfg(not(test))]
mod cap;

#[cfg(not(test))]
mod ipc;

#[cfg(not(test))]
mod syscall;

#[cfg(test)]
mod mem;

#[alloc_error_handler]
fn alloc_error(_: core::alloc::Layout) -> ! {
    crate::console::puts("KERNEL: allocation error\r\n");
    crate::idle_loop();
}

#[cfg(not(test))]
use core::panic::PanicInfo;

#[cfg(not(test))]
core::arch::global_asm!(
    ".section .text.entry, \"ax\", @progbits",
    ".globl _start",
    "_start:",
    "    csrw sie, zero",
    // Read HART ID from tp register (set by RustSBI)
    "    mv t0, tp",
    // Load per-HART boot stack: _boot_stacks + hart_id * 65536
    "    slli t1, t0, 16",            // t1 = hart_id * 65536
    "    la t2, _boot_stacks",
    "    add t2, t2, t1",
    "    mv sp, t2",
    // If HART 0, jump to rust_main. Otherwise, rust_secondary.
    "    bnez t0, 1f",
    "    tail rust_main",
    "1:  tail rust_secondary",
    ".section .bss",
    ".align 12",                         // 4096-byte aligned
    "_boot_stacks:",
    "    .space 65536 * 4, 0",           // 4 HARTs x 64KB each
);

#[cfg(not(test))]
#[no_mangle]
extern "C" fn rust_secondary() -> ! {
    // Park until primary HART signals ready
    while !BOOT_READY.load(Ordering::Acquire) {
        unsafe { core::arch::asm!("wfi"); }
    }

    // Same setup as primary (minus BSS clear and memory init)
    crate::trap::enable_timer_interrupt();
    crate::trap::init();
    crate::mem::sv39::enable_mmu();

    // Init per-CPU and enter scheduler
    crate::per_cpu::init_secondary();
    crate::sched::schedule();

    // Should never reach here
    crate::idle_loop();
}

#[cfg(not(test))]
#[no_mangle]
extern "C" fn rust_main(_hart_id: usize) -> ! {
    // Clear BSS
    unsafe {
        let bss_start = &_bss_start as *const u8 as usize;
        let bss_end = &_bss_end as *const u8 as usize;
        let size = bss_end - bss_start;
        core::ptr::write_bytes(bss_start as *mut u8, 0, size);
    }

    console::puts("TrainOS booting...\r\n");

    mem::init();
    console::puts("  Memory subsystem initialized\r\n");

    // MMIO and trap init BEFORE enabling MMU.
    // After sv39 enable, only the identity-mapped DRAM range
    // [0x80000000, 0x88000000) via L2[2] and the kernel virtual
    // range via L2[256] are accessible.  MMIO at low addresses
    // (e.g. CLINT at 0x2000000) would fault without a mapping,
    // so we set up CLINT and stvec while the CPU is still in
    // BARE translation mode.
    trap::clint_init();
    console::puts("  CLINT timer initialized\r\n");

    trap::enable_timer_interrupt();
    trap::init();
    console::puts("  Trap handling initialized\r\n");

    cap::init();
    console::puts("  Capability system initialized\r\n");

    ipc::init();
    console::puts("  IPC subsystem initialized\r\n");

    mem::sv39::enable_mmu();
    console::puts("  MMU enabled (Sv39)\r\n");

    // Spawn the init user-space process (highest priority so it creates EP 1 first)
    static INIT_ELF: &[u8] = include_bytes!("init.elf");
    match proc::spawn(INIT_ELF, 48) {
        Some(pid) => {
            console::puts("  Init process spawned (pid=");
            // Simple digit-by-digit print for pid (avoid format)
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: init spawn failed\r\n"),
    }

    // Spawn the ping user-space process
    static PING_ELF: &[u8] = include_bytes!("ping.elf");
    match proc::spawn(PING_ELF, 16) {
        Some(pid) => {
            console::puts("  Ping process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: ping spawn failed\r\n"),
    }

    // Spawn the FS service
    static FS_ELF: &[u8] = include_bytes!("fs.elf");
    match proc::spawn(FS_ELF, 32) {
        Some(pid) => {
            console::puts("  FS process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: FS spawn failed\r\n"),
    }

    // Spawn the NET service (V2.5 network stack, prio 43, above DRV(40))
    static NET_ELF: &[u8] = include_bytes!("net.elf");
    match proc::spawn(NET_ELF, 43) {
        Some(pid) => {
            console::puts("  NET process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: NET spawn failed\r\n"),
    }

    // Spawn the ECHO service (V2.5 network echo, prio 42)
    static ECHO_ELF: &[u8] = include_bytes!("echo.elf");
    match proc::spawn(ECHO_ELF, 42) {
        Some(pid) => {
            console::puts("  ECHO process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: ECHO spawn failed\r\n"),
    }

    // Spawn the TEST_NET service (V2.5 network test, prio 41)
    static TEST_NET_ELF: &[u8] = include_bytes!("test_net.elf");
    match proc::spawn(TEST_NET_ELF, 41) {
        Some(pid) => {
            console::puts("  TEST_NET process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: TEST_NET spawn failed\r\n"),
    }

    // Spawn the test_fs service
    static TEST_FS_ELF: &[u8] = include_bytes!("test_fs.elf");
    match proc::spawn(TEST_FS_ELF, 24) {
        Some(pid) => {
            console::puts("  TEST_FS process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: test_fs spawn failed\r\n"),
    }

    // Spawn the test_posix service (V2.3 POSIX compatibility demo)
    // Priority 31 (< FS=32 so FS starts first; > test_fork=30 so we run before fork hogs CPU)
    static TEST_POSIX_ELF: &[u8] = include_bytes!("test_posix.elf");
    match proc::spawn(TEST_POSIX_ELF, 31) {
        Some(pid) => {
            console::puts("  test_posix process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: test_posix spawn failed\r\n"),
    }

    // Spawn the shell service (same priority as test_fs for round-robin)
    static SH_ELF: &[u8] = include_bytes!("sh.elf");
    match proc::spawn(SH_ELF, 24) {
        Some(pid) => {
            console::puts("  Shell process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: sh spawn failed\r\n"),
    }

    // Spawn the test_fork service (V2.0 demo)
    static TEST_FORK_ELF: &[u8] = include_bytes!("test_fork.elf");
    match proc::spawn(TEST_FORK_ELF, 30) {
        Some(pid) => {
            console::puts("  test_fork process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: test_fork spawn failed\r\n"),
    }

    // Spawn the UART user-space driver (lowest priority, runs last)
    static UART_ELF: &[u8] = include_bytes!("uart.elf");
    match proc::spawn(UART_ELF, 24) {
        Some(pid) => {
            console::puts("  UART process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: uart spawn failed\r\n"),
    }

    // Spawn the drv service (VirtIO block driver, priority 5 so it runs last)
    static DRV_ELF: &[u8] = include_bytes!("drv.elf");
    match proc::spawn(DRV_ELF, 5) {
        Some(pid) => {
            console::puts("  DRV process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: drv spawn failed\r\n"),
    }

    // Spawn the NETDRV service (V4.0B VirtIO network driver, priority 63)
    // Prio 63 = highest, matches TEST_CAP. NETDRV enqueued first so runs first, then exits.
    static NETDRV_ELF: &[u8] = include_bytes!("netdrv.elf");
    match proc::spawn(NETDRV_ELF, 63) {
        Some(pid) => {
            console::puts("  NETDRV process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: netdrv spawn failed\r\n"),
    }

    // Spawn the TFS test service (V4.0A persistent disk FS, priority 55)
    static TEST_TFS_ELF: &[u8] = include_bytes!("test_tfs.elf");
    match proc::spawn(TEST_TFS_ELF, 55) {
        Some(pid) => {
            console::puts("  TEST_TFS process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: test_tfs spawn failed\r\n"),
    }

    // Spawn the C/ASM test program (V3.0 Route B — Standard C program support demo)
    static TEST_C_ELF: &[u8] = include_bytes!("test_c.elf");
    match proc::spawn(TEST_C_ELF, 50) {
        Some(pid) => {
            console::puts("  C program spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: C program spawn failed\r\n"),
    }

    // Spawn the PROC service (V3.2 namespace isolation — process listing/management)
    static PROC_ELF: &[u8] = include_bytes!("proc.elf");
    match proc::spawn(PROC_ELF, 60) {
        Some(pid) => {
            console::puts("  PROC process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: PROC spawn failed\r\n"),
    }

    // Spawn the TEST_PROC service (V3.2 test client)
    static TEST_PROC_ELF: &[u8] = include_bytes!("test_proc.elf");
    match proc::spawn(TEST_PROC_ELF, 59) {
        Some(pid) => {
            console::puts("  TEST_PROC process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: TEST_PROC spawn failed\r\n"),
    }

    // Spawn the TEST_CAP service (V4.0C capability security test, priority 63)
    static TEST_CAP_ELF: &[u8] = include_bytes!("test_cap.elf");
    match proc::spawn(TEST_CAP_ELF, 63) {
        Some(pid) => {
            console::puts("  TEST_CAP process spawned (pid=");
            unsafe {
                let mut n = pid;
                let mut buf = [0u8; 10];
                let mut i = 10;
                loop {
                    i -= 1;
                    buf[i] = b'0' + (n % 10) as u8;
                    n /= 10;
                    if n == 0 { break; }
                }
                for j in i..10 {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize);
                }
            }
            console::puts(")\r\n");
        }
        None => console::puts("  WARNING: TEST_CAP spawn failed\r\n"),
    }

    // Signal secondary HARTs that they can proceed
    BOOT_READY.store(true, Ordering::Release);
    console::puts("  Secondary HARTs released\r\n");

    // Create idle thread and start scheduler
    let idle = Box::new(crate::proc::thread::Thread::new_idle());
    let idle_ptr: *mut crate::proc::thread::Thread = Box::into_raw(idle);
    console::puts("  Starting scheduler...\r\n");
    crate::sched::start_scheduler(idle_ptr);
}

#[cfg(not(test))]
pub fn idle_loop() -> ! {
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    console::puts("KERNEL PANIC: ");
    if let Some(loc) = info.location() {
        console::puts(loc.file());
        console::puts(":");
    }
    console::puts("\r\n");
    idle_loop();
}

#[cfg(not(test))]
extern "C" {
    static _bss_start: u8;
    static _bss_end: u8;
}
