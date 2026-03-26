//! Boot module for trainOS
//! Entry point and early initialization

// Assembly boot code - placed at .text.entry (linked at 0x80200000)
// Stack grows downward from boot_stack_bottom
core::arch::global_asm!(
    ".section .text.entry,\"ax\",@progbits",
    ".globl _start",
    "_start:",
    "    la sp, boot_stack_top",
    "    tail rust_main",
    ".align 4",
    "boot_stack_bottom:",
    "    .space 8192, 0",
    "boot_stack_top:",
);

/// Early panic handler
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    crate::println!("PANIC: kernel panic occurred");
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// Main entry point called from assembly
#[no_mangle]
extern "C" fn rust_main() -> ! {
    // Test with println!
    crate::println!("========================================");
    crate::println!("  trainOS is booting!");
    crate::println!("========================================");
    crate::println!("  RISC-V64 Architecture");
    crate::println!("  Sv39 Virtual Memory");
    crate::println!("========================================");
    crate::println!();

    // Initialize memory management
    crate::memory::init();

    // Initialize process management
    crate::process::init();

    // Initialize trap handling
    crate::trap::init();

    // Initialize file system
    crate::fs::init();

    crate::println!("[OK] All subsystems initialized");
    crate::println!();

    // Run the first process
    crate::process::run_first_process();
}
