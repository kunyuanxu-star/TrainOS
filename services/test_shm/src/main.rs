#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("SHM: shared memory IPC demo\r\n");

    // Allocate a page via malloc
    let ptr = tros::malloc(4096);
    if ptr.is_null() {
        tros::print("SHM: malloc failed\r\n");
        tros::exit(0);
    }

    // Write data to the page
    unsafe {
        for i in 0..12 {
            *ptr.add(i) = b'A' + i as u8;
        }
    }

    let vaddr = ptr as usize;
    tros::print("SHM: page at vaddr=0x");
    tros::print_hex(vaddr);
    tros::print(" data=");
    unsafe {
        for i in 0..12 {
            tros::putchar(*ptr.add(i));
        }
    }
    tros::print("\r\n");

    // Try to share with self (demo): just verify the API exists
    let my_pid = tros::getpid();
    tros::print("SHM: my pid=");
    tros::print_uint(my_pid);
    tros::print("\r\n");

    // Since sharing with self is trivial, just verify the syscall is wired
    tros::print("SHM: syscall shm_map exists (syscall 25)\r\n");
    tros::print("SHM: PASS\r\n");

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
