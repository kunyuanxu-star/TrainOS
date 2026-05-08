#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

const VR_MAGIC: usize = 0x00;
const VR_DEVICE_ID: usize = 0x08;
const VR_STATUS: usize = 0x70;
const STATUS_ACK: u32 = 1;
const STATUS_DRIVER: u32 = 2;
const STATUS_DRIVER_OK: u32 = 4;
const VIRTIO_ID_NET: u32 = 1;

fn net_probe(base: usize) -> bool {
    let magic = tros::mmio_read32(base + VR_MAGIC) as u32;
    if magic == 0x74726976 {
        let dev_id = tros::mmio_read32(base + VR_DEVICE_ID) as u32;
        if dev_id == VIRTIO_ID_NET {
            tros::print("NETDRV: found net device at 0x");
            print_hex(base as u32);
            tros::print("!\r\n");

            // Initialize device
            tros::mmio_write32(base + VR_STATUS, 0); // reset
            tros::mmio_write32(base + VR_STATUS, STATUS_ACK as usize);
            tros::mmio_write32(base + VR_STATUS, (STATUS_ACK | STATUS_DRIVER) as usize);
            tros::mmio_write32(base + VR_STATUS, (STATUS_ACK | STATUS_DRIVER | STATUS_DRIVER_OK) as usize);

            // Read MAC address (device-specific config at offset 0x14)
            let mac_lo = tros::mmio_read32(base + 0x14) as u32;
            let mac_hi = tros::mmio_read32(base + 0x18) as u32;

            tros::print("NETDRV: MAC=");
            print_hex(mac_lo);
            tros::print(" ");
            print_hex(mac_hi);
            tros::print("\r\n");

            tros::print("NETDRV: PASS\r\n");
            return true;
        }
    }
    false
}

fn probe_addr(addr: usize) -> u32 {
    let v = tros::mmio_read32(addr) as u32;
    // mmio_read32 returns usize::MAX on fault (error from kernel)
    v
}

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("NETDRV: probing VirtIO net device...\r\n");

    // Known VirtIO MMIO addresses on machina:
    // 0x10001000 = block device (already claimed)
    // 0x10002000 = potential net device
    // 0x10003000, 0x10004000 = fallbacks
    let addrs = [0x10002000usize, 0x10003000, 0x10004000];
    let mut found = false;

    for &addr in &addrs {
        let magic = probe_addr(addr);
        if magic == 0x74726976 {
            if net_probe(addr) {
                found = true;
                break;
            }
        }
    }

    if !found {
        tros::print("NETDRV: device not found\r\n");
    }

    tros::exit(0);
}

fn print_hex(val: u32) {
    for i in (0..8).rev() {
        let nibble = (val >> (i * 4)) & 0xF;
        let c = if nibble < 10 { b'0' + nibble as u8 } else { b'a' + (nibble - 10) as u8 };
        tros::putchar(c);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
