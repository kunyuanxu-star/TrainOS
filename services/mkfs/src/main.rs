#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("MKFS: formatting disk with TFS...\r\n");

    // Superblock at sector 0
    let mut sb = [0u8; 512];
    sb[0] = b'T'; sb[1] = b'F'; sb[2] = b'S'; sb[3] = b'!'; // magic
    sb[4] = 0x00; sb[5] = 0x08; sb[6] = 0x00; sb[7] = 0x00; // 2048 sectors LE
    sb[8] = 2; // root_dir_sector = 2
    sb[12] = 1; // free_bitmap_sector = 1
    sb[16] = 64; // max_files = 64
    sb[20] = 8; // journal_sector = 8
    tros::blk_write(0, &sb);
    tros::print("MKFS: superblock written\r\n");

    // Free bitmap at sector 1: mark sectors 0-10 as used
    let mut bitmap = [0u8; 512];
    bitmap[0] = 0xFF; // sectors 0-7 used
    bitmap[1] = 0x07; // sectors 8-10 used (journal + root dir)
    tros::blk_write(1, &bitmap);
    tros::print("MKFS: bitmap written\r\n");

    // Root directory at sector 2
    let rootdir = [0u8; 512];
    tros::blk_write(2, &rootdir);
    tros::print("MKFS: root directory initialized\r\n");

    // Journal at sector 8
    let jrnl = [0u8; 512];
    tros::blk_write(8, &jrnl);
    tros::print("MKFS: journal initialized\r\n");

    // Write a "welcome" file at sector 9
    let mut welcome = [0u8; 512];
    let msg = b"Welcome to TrainOS TFS! Persistent storage active since boot.\n";
    for i in 0..msg.len() { welcome[i] = msg[i]; }
    tros::blk_write(9, &welcome);

    // Create directory entry for welcome file
    let mut dir = [0u8; 512];
    dir[0] = b'w'; dir[1] = b'e'; dir[2] = b'l'; dir[3] = b'c';
    dir[4] = b'o'; dir[5] = b'm'; dir[6] = b'e';
    // sector = 9, LE u32
    dir[28] = 9; dir[29] = 0; dir[30] = 0; dir[31] = 0;
    // flags = 1 (file)
    dir[32] = 1;
    // size = message length
    let len = msg.len() as u32;
    dir[36] = len as u8;
    dir[37] = (len >> 8) as u8;
    dir[38] = (len >> 16) as u8;
    dir[39] = (len >> 24) as u8;
    tros::blk_write(2, &dir);

    // Verify by reading back
    let mut verify = [0u8; 512];
    tros::blk_read(2, &mut verify);
    if verify[0] == b'w' {
        tros::print("MKFS: format verified\r\n");
        tros::print("MKFS: PASS\r\n");
    } else {
        tros::print("MKFS: FAIL\r\n");
    }

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
