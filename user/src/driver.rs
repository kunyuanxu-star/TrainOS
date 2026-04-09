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

    print("driver: Driver service ready (placeholder - IPC not yet connected)\n");

    // For now, just loop forever - in Phase 3, we will:
    // 1. Create endpoint for block I/O requests (port 2)
    // 2. Wait for fs_server to connect
    // 3. Handle IPC requests

    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}