#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("POSIX: testing open/read/write...\r\n");

    // Open (returns fd 0 for FS)
    let fd = tros::open("test.txt");
    tros::print("POSIX: fd=");
    let d = b'0' + (fd - (fd / 10) * 10) as u8;
    tros::putchar(d);
    tros::print("\r\n");

    // Write
    let msg = b"hello from posix!";
    let n = tros::write(fd, msg);
    tros::print("POSIX: wrote ");
    let d = b'0' + (n - (n / 10) * 10) as u8;
    tros::putchar(d);
    tros::print(" bytes\r\n");

    // Read
    let mut buf = [0u8; 64];
    let n = tros::read(fd, &mut buf);
    tros::print("POSIX: read '");
    for i in 0..n {
        tros::putchar(buf[i]);
    }
    tros::print("'\r\n");

    if &buf[..n] == b"hello from posix!" {
        tros::print("POSIX: PASS\r\n");
    } else {
        tros::print("POSIX: FAIL\r\n");
    }

    tros::close(fd);

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
