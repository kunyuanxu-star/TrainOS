#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

const PKG_EP: usize = 6;

#[derive(Copy, Clone)]
struct Package {
    name: [u8; 16],
    version: [u8; 8],
    ep: usize,
    installed: bool,
}

static mut PACKAGES: [Package; 16] = [
    Package { name: [0;16], version: [0;8], ep: 0, installed: false }; 16
];
static mut PKG_COUNT: usize = 0;

#[no_mangle]
extern "C" fn _start() -> ! {
    // Register pre-installed packages
    register(b"init\0\0\0\0\0\0\0\0\0\0\0\0", b"1.0.0\0\0\0", 1);
    register(b"fs\0\0\0\0\0\0\0\0\0\0\0\0\0\0", b"1.0.0\0\0\0", 2);
    register(b"net\0\0\0\0\0\0\0\0\0\0\0\0\0", b"1.0.0\0\0\0", 3);
    register(b"proc\0\0\0\0\0\0\0\0\0\0\0\0", b"1.0.0\0\0\0", 4);
    register(b"reg\0\0\0\0\0\0\0\0\0\0\0\0\0", b"1.0.0\0\0\0", 5);
    register(b"pkg\0\0\0\0\0\0\0\0\0\0\0\0\0", b"1.0.0\0\0\0", 6);

    tros::print("PKG: package manager on EP 6\r\n");

    let mut buf = [0u8; 64];
    loop {
        let (_sender, opcode) = tros::recv(PKG_EP, &mut buf);
        match opcode {
            0 => { // LIST
                unsafe {
                    tros::printf("PKG: %u packages installed\r\n", PKG_COUNT);
                    for i in 0..PKG_COUNT {
                        tros::print("  ");
                        for j in 0..16 { if PACKAGES[i].name[j] == 0 { break; } tros::putchar(PACKAGES[i].name[j]); }
                        tros::print(" v");
                        for j in 0..8 { if PACKAGES[i].version[j] == 0 { break; } tros::putchar(PACKAGES[i].version[j]); }
                        tros::printf(" (EP %u)\r\n", PACKAGES[i].ep);
                    }
                }
            }
            1 => { // INFO <name>
                let mut name = [0u8; 16];
                for i in 0..16 { name[i] = buf[i]; }
                unsafe {
                    for i in 0..PKG_COUNT {
                        let mut matches = true;
                        for j in 0..16 {
                            if PACKAGES[i].name[j] != name[j] && name[j] != 0 { matches = false; break; }
                            if PACKAGES[i].name[j] == 0 && name[j] == 0 { break; }
                        }
                        if matches {
                            tros::print("PKG: ");
                            for j in 0..16 { if PACKAGES[i].name[j] == 0 { break; } tros::putchar(PACKAGES[i].name[j]); }
                            tros::print(" v");
                            // Print version bytes
                            for j in 0..8 {
                                if PACKAGES[i].version[j] == 0 { break; }
                                tros::putchar(PACKAGES[i].version[j]);
                            }
                            tros::printf(" EP=%u\r\n", PACKAGES[i].ep);
                            break;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn register(name: &[u8], version: &[u8], ep: usize) {
    unsafe {
        if PKG_COUNT < 16 {
            for i in 0..name.len().min(16) { PACKAGES[PKG_COUNT].name[i] = name[i]; }
            for i in 0..version.len().min(8) { PACKAGES[PKG_COUNT].version[i] = version[i]; }
            PACKAGES[PKG_COUNT].ep = ep;
            PACKAGES[PKG_COUNT].installed = true;
            PKG_COUNT += 1;
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
