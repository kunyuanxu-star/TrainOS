#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

// PCI ECAM base for machina virt platform
const ECAM_BASE: usize = 0x30000000;

fn pci_read(bus: u8, dev: u8, func: u8, offset: u16) -> u32 {
    let addr = ECAM_BASE
        + ((bus as usize) << 20)
        + ((dev as usize) << 15)
        + ((func as usize) << 12)
        + (offset as usize);
    tros::mmio_read32(addr) as u32
}

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("PCI: scanning bus 0...\r\n");

    let mut found = 0;

    for dev in 0..32u8 {
        let id = pci_read(0, dev, 0, 0);
        // On ECAM, a non-existent function returns all 1s (0xFFFFFFFF).
        // Valid vendor IDs are never 0xFFFF or 0x0000.
        if id == 0xFFFFFFFF || id == 0 {
            continue;
        }
        let vendor_id = (id & 0xFFFF) as u16;

        let device_id = id >> 16;
        let class_rev = pci_read(0, dev, 0, 8);
        let class_code = (class_rev >> 24) as u8;
        let subclass = ((class_rev >> 16) & 0xFF) as u8;

        tros::print("PCI: dev=");
        print_hex2(dev);
        tros::print(" vendor=0x");
        print_hex4(vendor_id as u16);
        tros::print(" device=0x");
        print_hex4(device_id as u16);
        tros::print(" class=");
        print_hex2(class_code);
        tros::print(":");
        print_hex2(subclass);
        tros::print("\r\n");

        found += 1;
    }

    if found > 0 {
        tros::print("PCI: found ");
        tros::print_uint(found as usize);
        tros::print(" devices\r\n");
        tros::print("PCI: PASS\r\n");
    } else {
        tros::print("PCI: no devices found (ECAM may be at different address)\r\n");
        tros::print("PCI: trying alternate addresses...\r\n");
        // Try some alternate ECAM bases
        let bases = [0x30000000usize, 0x40000000, 0x3F000000];
        for base in bases.iter() {
            tros::print("PCI:  probing 0x");
            tros::print_hex(*base);
            let v = tros::mmio_read32(*base);
            if v != 0 && v != 0xFFFFFFFF {
                tros::print(" -> val=0x");
                tros::print_hex(v);
                tros::print(" (possible ECAM)\r\n");
            } else {
                tros::print(" -> val=0x");
                tros::print_hex(v);
                tros::print("\r\n");
            }
        }
    }

    tros::exit(0);
}

fn print_hex2(b: u8) {
    for shift in [4u8, 0u8].iter() {
        let n = (b >> shift) & 0xF;
        let c = if n < 10 { b'0' + n } else { b'a' + (n - 10) };
        tros::putchar(c);
    }
}

fn print_hex4(w: u16) {
    print_hex2((w >> 8) as u8);
    print_hex2((w & 0xFF) as u8);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
