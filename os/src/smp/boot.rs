//! HART boot and bring-up
//!
//! Handles starting secondary HARTs (CPU cores) via SBI

use crate::smp::{hart_count, set_current_hartid};
use crate::smp::hart::{HartState, register_hart, get_hart_state};
use crate::smp::cpu::get_hart_id;

/// Boot stack size per secondary HART
const BOOT_STACK_SIZE: usize = 8192;

/// Number of secondary HARTs to start (excluding boot HART)
const SECONDARY_HARTS: usize = 1;

/// Boot stacks for secondary HARTs - static mut for unsafe access
/// These are placed in .data.boot_stacks section
static mut BOOT_STARTS: [usize; SECONDARY_HARTS] = [0; SECONDARY_HARTS];
static mut BOOT_STACKS: [[u8; BOOT_STACK_SIZE]; SECONDARY_HARTS] = [[0; BOOT_STACK_SIZE]; SECONDARY_HARTS];

/// Secondary HART entry point
///
/// This is the function that secondary HARTs jump to after being started by SBI.
/// It sets up the per-CPU data and enters a simple idle loop.
#[no_mangle]
#[link_section = ".text.boot_hart"]
pub extern "C" fn secondary_hart_entry(hart_id: usize) {
    // Set tp (thread pointer) to our HART ID
    // This is how we identify which CPU we're on
    unsafe {
        core::arch::asm!("mv tp, {0}", in(reg) hart_id);
    }

    // Initialize per-CPU data
    secondary_hart_init();

    // Idle loop - secondary HART does nothing useful yet
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// Per-CPU initialization for secondary HARTs
fn secondary_hart_init() {
    // Get HART ID from tp register
    let hart_id = get_hart_id();

    // Set current HART ID in global state
    set_current_hartid(hart_id);

    // Initialize per-CPU data via cpu module
    crate::smp::cpu::smp_percpu_init();

    crate::print!("[boot] Secondary HART ");
    crate::console::print_dec(hart_id);
    crate::println!(" initialized");
}

/// Start all secondary HARTs via SBI
pub fn start_other_harts() {
    let total_harts = hart_count();
    crate::print!("[boot] Starting ");
    crate::console::print_dec(total_harts.saturating_sub(1));
    crate::println!(" secondary HARTs");

    if total_harts <= 1 {
        crate::println!("[boot] No secondary HARTs to start");
        return;
    }

    // Start each secondary HART
    for hart_id in 1..total_harts.min(SECONDARY_HARTS + 1) {
        start_hart(hart_id);
    }
}

/// Start a single HART via SBI hart_start
///
/// Uses SBI HART_START extension to start a HART at secondary_hart_entry
fn start_hart(hart_id: usize) {
    // Get boot stack for this HART
    let stack_top = get_boot_stack_top(hart_id);

    crate::print!("[boot] Starting HART ");
    crate::console::print_dec(hart_id);
    crate::print!(" with stack @ ");
    crate::console::print_hex(stack_top);
    crate::println!("");

    // Register the HART
    register_hart(hart_id, secondary_hart_entry as usize, stack_top);

    // Use SBI to start the HART
    // SBI hart_start(hart_id, entry_addr, priv) - not available in RustSBI legacy
    // For now, just mark the HART as running if it's already up
    if let Some(state) = get_hart_state(hart_id) {
        if state == HartState::PoweredDown {
            // Try to wake up via SBI (would use sbi_hart_start in real SBI)
            // In QEMU virt, secondary HARTs are already running in M-mode
            // They will read from DT and jump to our entry point
            crate::print!("[boot] HART ");
            crate::console::print_dec(hart_id);
            crate::println!(" marked as powered down, would wake via SBI");
        }
    }
}

/// Get the boot stack top address for a HART
fn get_boot_stack_top(hart_id: usize) -> usize {
    if hart_id == 0 {
        return 0; // Boot HART uses its own stack
    }

    let idx = (hart_id - 1).min(SECONDARY_HARTS - 1);
    unsafe {
        let stack_base = BOOT_STACKS[idx].as_ptr() as usize;
        stack_base + BOOT_STACK_SIZE
    }
}

/// Initialize boot data structures
pub fn init_boot() {
    // Initialize boot stack start addresses
    unsafe {
        for i in 0..SECONDARY_HARTS {
            BOOT_STARTS[i] = BOOT_STACKS[i].as_ptr() as usize;
        }
    }
    for c in b"[boot] init_boot done\n" {
        crate::console::sbi_console_putchar_raw(*c as usize);
    }
    crate::console::console_flush();
}
