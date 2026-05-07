#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![feature(alloc_error_handler)]

extern crate alloc;

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
    // Mask interrupts during boot
    "    csrw sie, zero",
    // Set up boot stack (64KB, ample for boot)
    "    la sp, _boot_stack_top",
    // Jump to Rust
    "    tail rust_main",
    ".section .bss",
    ".align 4",
    "_boot_stack_bottom:",
    "    .space 65536, 0",
    "_boot_stack_top:",
);

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

    // Spawn the init user-space process
    static INIT_ELF: &[u8] = include_bytes!("init.elf");
    match proc::spawn(INIT_ELF, 32) {
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
