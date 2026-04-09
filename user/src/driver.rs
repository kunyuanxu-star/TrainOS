//! Driver service entry point
//!
//! This service runs as a user-space process and handles VirtIO devices.
//! It provides block and network I/O to other services via IPC.

#![no_std]
#![no_main]

mod driver_blk;
mod driver_net;
mod driver_mmio;

use driver_blk::VirtioBlkDevice;
use driver_mmio::DEVICE_VIRTIO_BLK;
use driver_net::VirtioNetDevice;
use driver_mmio::DEVICE_VIRTIO_NET;

// Syscall numbers
const SYS_WRITE: usize = 64;
const SYS_EXIT: usize = 93;
const SYS_ENDPOINT_CREATE: usize = 1000;
const SYS_SEND: usize = 1002;
const SYS_RECV: usize = 1003;
const SYS_SCHED_YIELD: usize = 124;

// Driver port number
const DRIVER_PORT: u32 = 2;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

/// Write character to console
fn putchar(c: u8) {
    unsafe {
        core::arch::asm!(
            "li a7, 1; mv a0, {0}; ecall",
            in(reg) c
        );
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

/// Driver service main
#[no_mangle]
pub extern "C" fn _start() {
    print("driver: VirtIO driver service starting\n");

    // Initialize VirtIO block device
    let mut blk = VirtioBlkDevice::new(DEVICE_VIRTIO_BLK);

    match blk.init() {
        Ok(_) => {
            print("driver: VirtIO block device initialized\n");
            let cap = blk.capacity();
            print("driver: Block device capacity: 0x");
            print_hex(cap as usize);
            print("\n");
        }
        Err(e) => {
            print("driver: VirtIO block init failed: ");
            print(e);
            print("\n");
        }
    }

    // Initialize VirtIO net device
    let mut net = VirtioNetDevice::new(DEVICE_VIRTIO_NET);
    match net.init() {
        Ok(_) => print("driver: VirtIO net device initialized\n"),
        Err(e) => {
            print("driver: VirtIO net init failed: ");
            print(e);
            print("\n");
        }
    }

    print("driver: Ready (IPC placeholder - Phase 3)\n");

    // Phase 4: Create endpoint on DRIVER_PORT for IPC
    let driver_port = syscall(SYS_ENDPOINT_CREATE, 0, 0, 0, 0, 0, 0) as u32;
    if driver_port < 2 {
        print("driver: Failed to create endpoint\n");
        loop {
            unsafe { core::arch::asm!("wfi"); }
        }
    }

    print("driver: Listening on port ");
    print_hex(driver_port as usize);
    print("\n");

    // Buffer for IPC requests
    let mut req_buf: [u8; 256] = [0; 256];

    loop {
        // Phase 4 IPC stub: just yield for now
        // In Phase 5, we will:
        // 1. Receive BlockReadRequest / BlockWriteRequest on driver_port
        // 2. Process using VirtIO driver
        // 3. Send response back
        syscall(SYS_SCHED_YIELD, 0, 0, 0, 0, 0, 0);
        unsafe { core::arch::asm!("wfi"); }
    }
}