#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    // Test printf
    tros::printf("CLIB: printf test: answer = %u\r\n", 42);

    // Test strlen
    let s = b"hello\0extra";
    let len = tros::strlen(s);
    tros::printf("CLIB: strlen = %u\r\n", len);

    // Test malloc
    let ptr = tros::malloc(16);
    if !ptr.is_null() {
        unsafe {
            for i in 0..4 { *ptr.add(i) = b'A' + i as u8; }
        }
        tros::print("CLIB: malloc works: ");
        unsafe {
            tros::putchar(*ptr);
            tros::putchar(*ptr.add(1));
        }
        tros::print("\r\n");
    }

    // Test memcpy + memset
    let mut buf = [0u8; 32];
    tros::memset(&mut buf, b'X', 5);
    tros::memcpy(&mut buf[5..], b"hello", 5);

    tros::print("CLIB: mem ops: ");
    for i in 0..10 { tros::putchar(buf[i]); }
    tros::print("\r\n");

    if len == 5 {
        tros::print("CLIB: PASS\r\n");
    }

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
