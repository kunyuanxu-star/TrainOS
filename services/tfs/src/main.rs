#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("TFS: rich filesystem demo\r\n");

    // Read root directory from sector 2
    let mut rootdir = [0u8; 512];
    tros::blk_read(2, &mut rootdir);

    // List files in root directory
    tros::print("TFS: root directory listing:\r\n");
    let mut i = 0;
    while i < 4 { // check first 4 entries
        let off = i * 40;
        if rootdir[off] == 0 { i += 1; continue; }

        tros::print("  ");
        // Print name
        let mut j = 0;
        while j < 28 {
            let c = rootdir[off + j];
            if c == 0 { break; }
            tros::putchar(c);
            j += 1;
        }
        // Print sector
        let sector = (rootdir[off+28] as usize)
            | ((rootdir[off+29] as usize) << 8)
            | ((rootdir[off+30] as usize) << 16)
            | ((rootdir[off+31] as usize) << 24);
        tros::printf(" (sector %u)\r\n", sector);
        i += 1;
    }

    // Create a second file "world.txt" in a new directory entry
    let mut entry = [0u8; 40];
    entry[0] = b'w'; entry[1] = b'o'; entry[2] = b'r'; entry[3] = b'l';
    entry[4] = b'd'; entry[5] = b'.'; entry[6] = b't'; entry[7] = b'x';
    entry[8] = b't';
    entry[28] = 4; // sector 4 (LE u32)
    entry[32] = 1; // flags = file
    entry[36] = 11; // size = 11 bytes (LE u32)

    // Write to second entry slot (offset 40 in rootdir)
    let mut k = 0;
    while k < 40 {
        rootdir[40 + k] = entry[k];
        k += 1;
    }
    tros::blk_write(2, &rootdir);

    // Write content to sector 4
    let mut content = [0u8; 512];
    let msg = b"hello world";
    let mut m = 0;
    while m < msg.len() {
        content[m] = msg[m];
        m += 1;
    }
    tros::blk_write(4, &content);

    // Re-read root directory to verify
    let mut verify = [0u8; 512];
    tros::blk_read(2, &mut verify);

    if verify[40] == b'w' && verify[41] == b'o' {
        tros::print("TFS: second file created OK\r\n");
        tros::print("TFS: PASS\r\n");
    } else {
        tros::print("TFS: FAIL\r\n");
    }

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
