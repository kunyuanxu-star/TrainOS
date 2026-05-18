#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("MOUNT: checking persistent filesystem...\r\n");

    // Read superblock
    let mut sb = [0u8; 512];
    tros::blk_read(0, &mut sb);

    if &sb[0..4] == b"TFS!" {
        tros::print("MOUNT: TFS superblock found\r\n");
    } else {
        tros::print("MOUNT: no filesystem!\r\n");
        tros::exit(0);
    }

    // Read root directory
    let mut dir = [0u8; 512];
    tros::blk_read(2, &mut dir);

    if dir[0] == b'w' {
        tros::print("MOUNT: found welcome file\r\n");
        let sector = dir[28] as usize | ((dir[29] as usize) << 8)
            | ((dir[30] as usize) << 16) | ((dir[31] as usize) << 24);
        tros::print("MOUNT: reading sector ");
        print_small(sector);
        tros::print("...\r\n");

        let mut data = [0u8; 512];
        tros::blk_read(sector, &mut data);
        tros::print("MOUNT: ");
        for i in 0..60 { if data[i] == 0 { break; } tros::putchar(data[i]); }
        tros::print("\r\n");

        if &data[0..7] == b"Welcome" {
            tros::print("MOUNT: PASS\r\n");
        }
    } else {
        tros::print("MOUNT: empty directory\r\n");
    }

    tros::exit(0);
}

fn print_small(n: usize) {
    let mut m = n; let mut buf = [0u8; 10]; let mut i = 10;
    if m == 0 { tros::putchar(b'0'); return; }
    loop { i -= 1; buf[i] = b'0' + (m - (m/10)*10) as u8; m /= 10; if m == 0 { break; } }
    for j in i..10 { tros::putchar(buf[j]); }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
