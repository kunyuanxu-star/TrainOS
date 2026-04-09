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

    print("driver: Ready for IPC block I/O\n");

    // Create endpoint on DRIVER_PORT for IPC
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

    // Buffer for IPC requests and responses
    let mut req_buf: [u8; 256] = [0; 256];
    let mut resp_buf: [u8; 516] = [0; 516]; // status (4 bytes) + data (512 bytes)

    loop {
        // Receive request (blocking)
        let size = syscall(SYS_RECV, driver_port as usize, req_buf.as_mut_ptr() as usize, 256, 0, 0, 0) as usize;

        if size > 0 && size >= 20 {
            // Parse IPC header (20 bytes)
            // from (4 bytes) | to (4 bytes) | port (4 bytes) | payload_size (4 bytes) | reply_port (4 bytes)
            let from: u32 = unsafe { *(req_buf.as_ptr() as *const u32) };
            let _to: u32 = unsafe { *(req_buf.as_ptr().add(4) as *const u32) };
            let _port: u32 = unsafe { *(req_buf.as_ptr().add(8) as *const u32) };
            let _payload_size: u32 = unsafe { *(req_buf.as_ptr().add(12) as *const u32) };
            let reply_port: u32 = unsafe { *(req_buf.as_ptr().add(16) as *const u32) };

            // Get payload pointer (after 20-byte header)
            let payload = 20usize;

            // First u32 of payload is the operation (0=read, 1=write)
            let op: u32 = unsafe { *(req_buf.as_ptr().add(payload) as *const u32) };

            if op == 0 {
                // Block read request
                let sector: u64 = unsafe { *(req_buf.as_ptr().add(payload + 4) as *const u64) };

                print("driver: Block read sector ");
                print_hex(sector as usize);
                print("\n");

                // TODO: Perform actual VirtIO block read
                // Response: status (4 bytes) + data (512 bytes)
                unsafe { *(resp_buf.as_mut_ptr() as *mut i32) = 0; } // status = OK

                // Send response
                if reply_port > 0 {
                    syscall(SYS_SEND, from as usize, reply_port as usize,
                           resp_buf.as_ptr() as usize, 516, 0, 0);
                }
            } else if op == 1 {
                // Block write request
                print("driver: Block write\n");

                // TODO: Perform actual VirtIO block write
                // Response: status (4 bytes)
                unsafe { *(resp_buf.as_mut_ptr() as *mut i32) = 0; } // status = OK

                if reply_port > 0 {
                    syscall(SYS_SEND, from as usize, reply_port as usize,
                           resp_buf.as_ptr() as usize, 4, 0, 0);
                }
            }
        }

        unsafe { core::arch::asm!("wfi"); }
    }
}