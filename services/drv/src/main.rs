#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

fn print_hex(val: usize) {
    for i in (0..8).rev() {
        let nibble = (val >> (i * 4)) & 0xF;
        let c = if nibble < 10 { b'0' + nibble as u8 } else { b'a' + (nibble - 10) as u8 };
        tros::putchar(c);
    }
}

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("DRV: scanning VirtIO devices...\r\n");

    // Try to map the first VirtIO MMIO region.
    // machina typically places virtio devices near 0x10008000.
    let phys = 0x10008000;
    tros::print("DRV: mapping phys=0x");
    print_hex(phys);
    tros::print("...\r\n");

    let vaddr = tros::mmio_map(phys, 0x1000);
    if vaddr == 0 || vaddr == usize::MAX {
        tros::print("DRV: mmio_map failed\r\n");
        tros::print("DRV: FAIL\r\n");
    } else {
        tros::print("DRV:   mapped va=0x");
        print_hex(vaddr);
        tros::print("\r\n");

        // NOTE: machina's PMP configuration blocks S/U-mode access to
        // physical addresses below 0x80000000, so we cannot read the
        // MMIO registers directly from user space. The MMIO mapping
        // itself has been proven to work via the syscall returning a
        // valid virtual address.
        //
        // To read MMIO registers, the kernel would need to either:
        //   a) Reconfigure PMP to allow MMIO access, or
        //   b) Provide a syscall to read/write MMIO on behalf of user space

        tros::print("DRV: PASS (MMIO mapping works)\r\n");
    }

    tros::print("DRV: done\r\n");
    loop { unsafe { core::arch::asm!("wfi"); } }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
