#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

/// VirtIO MMIO register offsets
const MAGIC_VALUE: usize = 0x000; // 0x74726976 = "virt"
const VERSION:     usize = 0x004; // 1 (legacy) or 2 (modern)
const DEVICE_ID:   usize = 0x008; // 2 = block device
const VENDOR_ID:   usize = 0x00C;

/// Physical base address of the first VirtIO MMIO device on machina.
const VIRTIO_BASE: usize = 0x10001000;

fn print_hex(val: usize, digits: usize) {
    for i in (0..digits).rev() {
        let nibble = (val >> (i * 4)) & 0xF;
        let c = if nibble < 10 { b'0' + nibble as u8 } else { b'a' + (nibble - 10) as u8 };
        tros::putchar(c);
    }
}

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("DRV: scanning VirtIO devices via kernel MMIO proxy...\r\n");

    // Read VirtIO MMIO registers using the kernel proxy syscall.
    // The kernel (S-mode) accesses the physical address through an
    // identity mapping set up during boot.
    let magic   = tros::mmio_read32(VIRTIO_BASE + MAGIC_VALUE);
    let version = tros::mmio_read32(VIRTIO_BASE + VERSION);
    let dev_id  = tros::mmio_read32(VIRTIO_BASE + DEVICE_ID);
    let vendor  = tros::mmio_read32(VIRTIO_BASE + VENDOR_ID);

    tros::print("DRV:   magic  = 0x");
    print_hex(magic, 8);
    tros::print("\r\n");

    tros::print("DRV:   version = 0x");
    print_hex(version, 8);
    tros::print("\r\n");

    tros::print("DRV:   dev_id = 0x");
    print_hex(dev_id, 8);
    tros::print("\r\n");

    tros::print("DRV:   vendor = 0x");
    print_hex(vendor, 8);
    tros::print("\r\n");

    // Check magic value: 0x74726976 ("virt")
    if magic == 0x74726976 {
        tros::print("DRV: PASS (VirtIO device found)\r\n");
    } else {
        tros::print("DRV: FAIL (no VirtIO device)\r\n");
    }

    tros::print("DRV: done\r\n");
    loop { unsafe { core::arch::asm!("wfi"); } }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
