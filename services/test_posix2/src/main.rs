#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("POSIX2: testing extended syscalls\r\n");

    // stat: kernel returns file size via return value (not buffer fill)
    let mut buf = [0u8; 16];
    let size_ret = tros::stat(0, &mut buf);
    tros::printf("  stat(): ret=%u\r\n", size_ret);

    // lseek: returns the offset
    let pos = tros::lseek(0, 100, 0);
    tros::printf("  lseek(): pos=%u\r\n", pos);

    // dup: returns new fd
    let fd2 = tros::dup(0);
    tros::printf("  dup(): new fd=%u\r\n", fd2);

    // getcwd: returns success
    let mut cwd = [0u8; 32];
    tros::getcwd(&mut cwd);
    tros::printf("  getcwd(): char[0]=%u\r\n", cwd[0] as usize);

    if size_ret >= 512 {
        tros::print("POSIX2: PASS\r\n");
    } else {
        tros::print("POSIX2: FAIL\r\n");
    }

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
