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

#[cfg(not(test))]
mod invariant;

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

    // Spawn the EDIT service (V6.0D line editor, prio 62, EP 2 after test_cap takes EP 1)
    // High priority to run before wfi-loop services that starve lower priorities.
    static EDIT_ELF: &[u8] = include_bytes!("edit.elf");
    match proc::spawn(EDIT_ELF, 62) {
        Some(pid) => {
            console::puts("  EDIT process spawned (pid=");
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
        None => console::puts("  WARNING: edit spawn failed\r\n"),
    }

    // Spawn the TEST_EDIT service (V6.0D editor test client, same prio 62 to avoid starvation)
    static TEST_EDIT_ELF: &[u8] = include_bytes!("test_edit.elf");
    match proc::spawn(TEST_EDIT_ELF, 62) {
        Some(pid) => {
            console::puts("  TEST_EDIT process spawned (pid=");
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
        None => console::puts("  WARNING: test_edit spawn failed\r\n"),
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

    // Spawn the REG service (V5.0A service registry, priority 57)
    // Runs after test_cap(63), proc(60); creates EP 3 (EP 1=test_cap, EP 2=proc)
    static REG_ELF: &[u8] = include_bytes!("reg.elf");
    match proc::spawn(REG_ELF, 57) {
        Some(pid) => {
            console::puts("  REG process spawned (pid=");
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
        None => console::puts("  WARNING: REG spawn failed\r\n"),
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

    // Spawn the CAT service (V5.0B user-space utility, priority 25)
    static CAT_ELF: &[u8] = include_bytes!("cat.elf");
    match proc::spawn(CAT_ELF, 25) {
        Some(pid) => {
            console::puts("  CAT process spawned (pid=");
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
        None => console::puts("  WARNING: cat spawn failed\r\n"),
    }

    // Spawn the test_inv service (V5.0C kernel invariant test, priority 26)
    static TEST_INV_ELF: &[u8] = include_bytes!("test_inv.elf");
    match proc::spawn(TEST_INV_ELF, 26) {
        Some(pid) => {
            console::puts("  TEST_INV process spawned (pid=");
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
        None => console::puts("  WARNING: test_inv spawn failed\r\n"),
    }

    // Spawn the test_clib service (V6.0A mini C library test, priority 62)
    static TEST_CLIB_ELF: &[u8] = include_bytes!("test_clib.elf");
    match proc::spawn(TEST_CLIB_ELF, 62) {
        Some(pid) => {
            console::puts("  TEST_CLIB process spawned (pid=");
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
        None => console::puts("  WARNING: test_clib spawn failed\r\n"),
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

    // Spawn the test_sdp service (V5.0A service discovery test, priority 56)
    // Runs after REG(57); sends to REG's EP for service lookup
    static TEST_SDP_ELF: &[u8] = include_bytes!("test_sdp.elf");
    match proc::spawn(TEST_SDP_ELF, 56) {
        Some(pid) => {
            console::puts("  TEST_SDP process spawned (pid=");
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
        None => console::puts("  WARNING: test_sdp spawn failed\r\n"),
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

    // Spawn the TFS rich filesystem service (V6.0C directory tree, priority 54)
    static TFS_ELF: &[u8] = include_bytes!("tfs.elf");
    match proc::spawn(TFS_ELF, 61) {
        Some(pid) => {
            console::puts("  TFS process spawned (pid=");
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
        None => console::puts("  WARNING: tfs spawn failed\r\n"),
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

    // Spawn the TFS journal demo (V7.0C write-ahead log, priority 62)
    // Priority 62 matches EDIT/TEST_EDIT/TEST_CLIB so it runs before
    // those services enter wfi loops and hog the scheduler at that level.
    static TFS_JRNL_ELF: &[u8] = include_bytes!("tfs_jrnl.elf");
    match proc::spawn(TFS_JRNL_ELF, 62) {
        Some(pid) => {
            console::puts("  TFS_JRNL process spawned (pid=");
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
        None => console::puts("  WARNING: tfs_jrnl spawn failed\r\n"),
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

    // Spawn the TEST_PERF service (V5.0D performance benchmark, priority 27)
    static TEST_PERF_ELF: &[u8] = include_bytes!("test_perf.elf");
    match proc::spawn(TEST_PERF_ELF, 27) {
        Some(pid) => {
            console::puts("  TEST_PERF process spawned (pid=");
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
        None => console::puts("  WARNING: TEST_PERF spawn failed\r\n"),
    }

    // Spawn the BB service (V7.0B BusyBox-like multi-command utility, priority 63)
    // High priority to run before JRNL(62) which crashes with an unhandled trap.
    // Demonstrates the BusyBox concept: single binary dispatching multiple commands.
    static BB_ELF: &[u8] = include_bytes!("bb.elf");
    match proc::spawn(BB_ELF, 63) {
        Some(pid) => {
            console::puts("  BB process spawned (pid=");
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
        None => console::puts("  WARNING: BB spawn failed\r\n"),
    }

    // Spawn the TEST_ARP service (V7.0A ARP query test for virtual Ethernet, priority 28)
    static TEST_ARP_ELF: &[u8] = include_bytes!("test_arp.elf");
    match proc::spawn(TEST_ARP_ELF, 28) {
        Some(pid) => {
            console::puts("  TEST_ARP process spawned (pid=");
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
        None => console::puts("  WARNING: TEST_ARP spawn failed\r\n"),
    }

    // Spawn the PCI service (V7.0D PCI bus enumeration, priority 59)
    // Scans PCI configuration space via ECAM to discover devices.
    static PCI_ELF: &[u8] = include_bytes!("pci.elf");
    match proc::spawn(PCI_ELF, 59) {
        Some(pid) => {
            console::puts("  PCI process spawned (pid=");
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
        None => console::puts("  WARNING: PCI spawn failed\r\n"),
    }

    // Spawn the VETH virtual ethernet service (V7.0A virtual Ethernet over IPC, priority 58)
    static VETH_ELF: &[u8] = include_bytes!("veth.elf");
    match proc::spawn(VETH_ELF, 58) {
        Some(pid) => {
            console::puts("  VETH process spawned (pid=");
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
        None => console::puts("  WARNING: VETH spawn failed\r\n"),
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
