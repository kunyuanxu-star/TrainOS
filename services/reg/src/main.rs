#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

// Service registry: name -> EP
static mut ENTRIES: [([u8; 16], usize); 8] = [([0; 16], 0); 8];
static mut COUNT: usize = 0;

#[no_mangle]
extern "C" fn _start() -> ! {
    // Create our endpoint and print the assigned EP number
    let my_ep = tros::ep_create();
    tros::print("REG: registry on ep=");
    print_small(my_ep);
    tros::print("\r\n");

    // Pre-register known services
    register(b"fs", 2);
    register(b"net", 3);
    register(b"proc", 4);
    register(b"reg", my_ep);

    let mut buf = [0u8; 64];
    loop {
        let (_sender, opcode) = tros::recv(my_ep, &mut buf);
        if _sender == usize::MAX {
            continue;
        }
        match opcode {
            0 => {
                // LOOKUP: buf[0..] = service name, returns EP number
                let mut found_ep: usize = 0;
                unsafe {
                    for i in 0..COUNT {
                        let mut matches = true;
                        for j in 0..ENTRIES[i].0.len() {
                            if ENTRIES[i].0[j] == 0 {
                                break;
                            }
                            if j >= 16 || ENTRIES[i].0[j] != buf[j] {
                                matches = false;
                                break;
                            }
                        }
                        if matches {
                            found_ep = ENTRIES[i].1;
                            break;
                        }
                    }
                }
                tros::print("REG: lookup -> ep=");
                print_small(found_ep);
                tros::print("\r\n");
            }
            1 => {
                // REGISTER: buf[0..16]=name, buf[16]=ep
                let ep = buf[16] as usize;
                let mut name = [0u8; 16];
                for i in 0..16 {
                    name[i] = buf[i];
                }
                register(&name, ep);
            }
            _ => {}
        }
    }
}

fn register(name: &[u8], ep: usize) {
    unsafe {
        if COUNT < 8 {
            for i in 0..name.len().min(16) {
                ENTRIES[COUNT].0[i] = name[i];
            }
            ENTRIES[COUNT].1 = ep;
            COUNT += 1;
        }
    }
}

fn print_small(n: usize) {
    let mut m = n;
    let mut buf = [0u8; 10];
    let mut i = 10;
    if m == 0 {
        tros::putchar(b'0');
        return;
    }
    loop {
        i -= 1;
        buf[i] = b'0' + (m - (m / 10) * 10) as u8;
        m = m / 10;
        if m == 0 {
            break;
        }
    }
    for j in i..10 {
        tros::putchar(buf[j]);
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
