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

#[cfg(not(test))]
mod ns;

#[cfg(not(test))]
mod device;

#[cfg(not(test))]
mod security;

#[cfg(not(test))]
mod iouring;

#[cfg(not(test))]
mod hypervisor;

#[cfg(not(test))]
mod extension;

#[cfg(not(test))]
mod numa;

#[cfg(not(test))]
mod distributed;

#[cfg(not(test))]
mod aslr;

#[cfg(not(test))]
mod wasm;

#[cfg(not(test))]
mod ai;

#[cfg(not(test))]
mod compat;

#[cfg(test)]
mod mem;

#[alloc_error_handler]
fn alloc_error(_: core::alloc::Layout) -> ! {
    crate::println!("KERNEL: allocation error");
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
    "    mv t0, tp",
    "    slli t1, t0, 16",
    "    la t2, _boot_stacks",
    "    add t2, t2, t1",
    "    mv sp, t2",
    "    bnez t0, 1f",
    "    tail rust_main",
    "1:  tail rust_secondary",
    ".section .bss",
    ".align 12",
    "_boot_stacks:",
    "    .space 65536 * 4, 0",
);

#[cfg(not(test))]
#[no_mangle]
extern "C" fn rust_secondary() -> ! {
    while !BOOT_READY.load(Ordering::Acquire) {
        unsafe {
            core::arch::asm!("wfi");
        }
    }

    crate::trap::enable_timer_interrupt();
    crate::trap::init();
    crate::mem::sv39::enable_mmu();

    crate::per_cpu::init_secondary();
    crate::sched::schedule();
    crate::idle_loop();
}

/// Spawn a service and print the result — eliminates ~500 lines of repetitive code.
macro_rules! spawn_service {
    ($name:ident, $prio:expr) => {{
        static ELF: &[u8] = include_bytes!(concat!(stringify!($name), ".elf"));
        match crate::proc::spawn(ELF, $prio) {
            Some(pid) => crate::println!("  {} spawned (pid={})", stringify!($name).to_uppercase(), pid),
            None => crate::println!("  WARNING: {} spawn failed", stringify!($name)),
        }
    }};
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

    println!("TrainOS booting...");

    mem::init();
    println!("  Memory subsystem initialized");

    trap::clint_init();
    println!("  CLINT timer initialized");

    trap::enable_timer_interrupt();
    trap::init();
    println!("  Trap handling initialized");

    cap::init();
    println!("  Capability system initialized");

    ipc::init();
    println!("  IPC subsystem initialized");

    // V26: Initialize distributed IPC subsystem (cluster node 0)
    crate::distributed::init(0);
    println!("  Distributed IPC initialized");

    mem::sv39::enable_mmu();
    println!("  MMU enabled (Sv39)");

    // V27: Initialize ASLR and KASLR
    crate::aslr::aslr_init();
    crate::aslr::kaslr_init();
    println!("  ASLR/KASLR initialized");

    // V33: Initialize Confidential Computing TEE subsystem
    crate::security::tee::tee_init();
    println!("  TEE subsystem initialized");

    // V28: Initialize WASM/WASI subsystem
    wasm::wasi::wasi_init();
    println!("  WASI subsystem initialized");

    // V32: Initialize WASM host-call interface + eBPF+WASM hybrid
    wasm::hostcall::init_wasm_syscall_table();
    wasm::hybrid::init_hybrid();
    println!("  V32: WASM host-call + hybrid engine initialized");

    // V30: Initialize Linux ABI compatibility subsystem
    compat::compat_init();
    println!("  Linux ABI compat initialized");

    // V30: Register default services with service manager
    let _svc_idx = compat::deploy::service_register(
        "init", "/sbin/init",
        compat::deploy::RestartPolicy::Always,
        &[],
    );
    let _svc_idx = compat::deploy::service_register(
        "fs", "/sbin/fs",
        compat::deploy::RestartPolicy::OnFailure,
        &[],
    );
    let _svc_idx = compat::deploy::service_register(
        "net", "/sbin/net",
        compat::deploy::RestartPolicy::OnFailure,
        &["fs"],
    );
    let _svc_idx = compat::deploy::service_register(
        "sh", "/bin/sh",
        compat::deploy::RestartPolicy::UnlessStopped,
        &["fs"],
    );
    println!("  Service manager initialized");

    // Spawn all services in priority order.
    //
    // Priority allocation rationale:
    //  63 — Must-run-first services (test_cap, netdrv, bb, test_shm, test_exec, test_sig, test_clib)
    //  62 — High-priority services (edit, test_edit, tfs_jrnl, test_user)
    //  61 — Core services (tfs, test_smp)
    //  60 — System management (proc, rustdemo)
    //  59 — Device enumeration (pci, bench)
    //  58 — Network infra (veth, pkg, http, test_http)
    //  57 — Service registry (reg, mkfs)
    //  56 — Discovery (test_sdp)
    //  55 — Demo (demo)
    //  54 — Storage tests (test_net2)
    //  53 — Package tests (test_pkg)
    //  50 — Persistence tests (test_mount)
    //  48 — Init (highest non-test)
    //  43 — Network (net)
    //  42 — Network echo (echo)
    //  41 — Network tests (test_net)
    //  32 — File system (fs)
    //  31 — FS tests (test_posix, stress)
    //  30 — Process tests (test_fork)
    //  24-27 — Low-priority misc tests
    //  10 — C program
    //  5  — Block driver (drv, runs last)

    // Priority 63 group: must-run-first services
    spawn_service!(test_cap, 63);
    spawn_service!(netdrv, 63);
    spawn_service!(bb, 63);
    spawn_service!(test_shm, 63);
    spawn_service!(test_exec, 63);

    // Priority 62 group
    spawn_service!(edit, 62);
    spawn_service!(test_edit, 62);
    spawn_service!(test_clib, 62);
    spawn_service!(test_user, 62);
    spawn_service!(test_sig, 62);
    spawn_service!(tfs_jrnl, 62);

    // Priority 61
    spawn_service!(tfs, 61);
    spawn_service!(test_smp, 61);

    // Priority 60
    spawn_service!(proc, 60);
    spawn_service!(rustdemo, 60);

    // Priority 59
    spawn_service!(pci, 59);
    spawn_service!(bench, 59);

    // Priority 58 — Network infrastructure
    spawn_service!(veth, 58);
    spawn_service!(pkg, 58);
    spawn_service!(http, 58);
    spawn_service!(test_http, 58);

    // Priority 57
    spawn_service!(reg, 57);
    spawn_service!(mkfs, 57);

    // Priority 56
    spawn_service!(test_sdp, 56);

    // Priority 55
    spawn_service!(test_tfs, 55);

    // Priority 54
    spawn_service!(test_net2, 54);

    // Priority 53
    spawn_service!(test_pkg, 53);

    // Priority 50
    spawn_service!(test_mount, 50);

    // Priority 48 — Init (creates EP 1)
    spawn_service!(init, 48);

    // Priority 44 — TCP protocol service (runs after net at 43)
    spawn_service!(tcp, 44);

    // Priority 43 — Network stack
    spawn_service!(net, 43);

    // Priority 42 — Echo service
    spawn_service!(echo, 42);

    // Priority 41 — Network test
    spawn_service!(test_net, 41);

    // Priority 32 — File system service
    spawn_service!(fs, 32);

    // Priority 31 — POSIX/Filesystem tests
    spawn_service!(test_posix, 31);
    spawn_service!(stress, 31);

    // Priority 30 — Fork test
    spawn_service!(test_fork, 30);

    // Priority 28
    spawn_service!(test_arp, 28);

    // Priority 27 — Low-priority test services
    spawn_service!(test_posix2, 27);
    spawn_service!(test_perf, 27);

    // Priority 26
    spawn_service!(test_inv, 26);

    // Priority 25
    spawn_service!(cat, 25);

    // Priority 24 — Shell and filesystem test
    spawn_service!(test_fs, 24);
    spawn_service!(sh, 24);
    spawn_service!(uart, 24);

    // Priority 10 — C program
    spawn_service!(test_c, 10);

    // Priority 5 — Block driver (runs last)
    spawn_service!(drv, 5);

    // Priority 55 — Demo (runs after all services are spawned)
    spawn_service!(selftest, 56);
    spawn_service!(demo, 55);

    // Signal secondary HARTs
    BOOT_READY.store(true, Ordering::Release);
    println!("  Secondary HARTs released");

    // Create idle thread and start scheduler
    let idle = Box::new(crate::proc::thread::Thread::new_idle());
    let idle_ptr: *mut crate::proc::thread::Thread = Box::into_raw(idle);
    println!("  Starting scheduler...");
    crate::sched::start_scheduler(idle_ptr);
}

#[cfg(not(test))]
pub fn idle_loop() -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("KERNEL PANIC: {}", info);
    idle_loop();
}

#[cfg(not(test))]
extern "C" {
    static _bss_start: u8;
    static _bss_end: u8;
}
