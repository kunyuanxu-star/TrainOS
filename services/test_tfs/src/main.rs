#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("TFS: formatting disk...\r\n");

    // Write superblock to sector 0
    let mut sb = [0u8; 512];
    sb[0] = b'T'; sb[1] = b'F'; sb[2] = b'S'; sb[3] = b'!';
    // total_sectors = 2048 (1MB / 512)
    sb[4] = 0x00; sb[5] = 0x08; sb[6] = 0x00; sb[7] = 0x00; // 2048 LE
    // root_dir_sector = 2
    sb[8] = 2; sb[9] = 0; sb[10] = 0; sb[11] = 0;
    // free_bitmap_sector = 1
    sb[12] = 1; sb[13] = 0; sb[14] = 0; sb[15] = 0;
    // max_files = 32
    sb[16] = 32; sb[17] = 0; sb[18] = 0; sb[19] = 0;

    tros::blk_write(0, &sb);
    tros::print("TFS: superblock written\r\n");

    // Write free bitmap (sector 1): mark sectors 0-3 as used
    let mut bitmap = [0u8; 512];
    bitmap[0] = 0x0F; // sectors 0-3 used
    tros::blk_write(1, &bitmap);
    tros::print("TFS: bitmap written\r\n");

    // Write empty root directory (sector 2)
    let rootdir = [0u8; 512];
    tros::blk_write(2, &rootdir);
    tros::print("TFS: root dir written\r\n");

    // Write a test file "hello.txt" at sector 3
    let mut dir = [0u8; 512];
    let name = b"hello.txt";
    let mut i = 0;
    while i < name.len() {
        dir[i] = name[i];
        i += 1;
    }
    dir[28] = 3; // file at sector 3, LE u32
    tros::blk_write(2, &dir);

    // Write file content to sector 3
    let mut content = [0u8; 512];
    let msg = b"Hello from TFS! Persistent storage works!";
    let mut j = 0;
    while j < msg.len() {
        content[j] = msg[j];
        j += 1;
    }
    tros::blk_write(3, &content);
    tros::print("TFS: hello.txt written to sector 3\r\n");

    // Read back and verify
    let mut readback = [0u8; 512];
    tros::blk_read(3, &mut readback);

    if &readback[0..5] == b"Hello" {
        tros::print("TFS: readback OK - ");
        let mut k = 0;
        while k < 37 {
            tros::putchar(readback[k]);
            k += 1;
        }
        tros::print("\r\nTFS: PASS\r\n");
    } else {
        tros::print("TFS: FAIL\r\n");
    }

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfi"); } }
}
