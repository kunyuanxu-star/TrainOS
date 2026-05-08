#![no_std]
#![no_main]

use core::panic::PanicInfo;
use tros;

// Static storage - lives in the data section, NOT on stack, so the compiler
// can never alias it with the stack-local recv buffer.
static mut STORAGE: [u8; 64] = [0; 64];
static mut STORAGE_LEN: usize = 0;

#[no_mangle]
extern "C" fn _start() -> ! {
    // Use well-known EP 2 (pre-created by kernel ipc::init()).
    let my_ep = 2;
    tros::print("FS: ep=2 listening\r\n");

    let mut buf = [0u8; 64];

    loop {
        let (sender_pid, opcode) = tros::recv(my_ep, &mut buf);
        if sender_pid == usize::MAX {
            continue;
        }

        // Payload format for both operations:
        //   bytes [0..1]: reply_ep (little-endian u16)
        // For WRITE (opcode 3):
        //   byte [2]:     data length
        //   bytes [3..]:  data
        let reply_ep = buf[0] as usize | ((buf[1] as usize) << 8);

        match opcode {
            3 => {
                // WRITE: store data using raw pointer to static storage
                let data_len = buf[2] as usize;
                if data_len > 0 && data_len <= 63 {
                    unsafe {
                        let dst = &raw mut STORAGE as *mut u8;
                        let src = buf.as_ptr().add(3);
                        for i in 0..data_len {
                            dst.add(i).write(src.add(i).read());
                        }
                        (&raw mut STORAGE_LEN as *mut usize).write(data_len);
                    }
                }
                tros::print("FS: stored data\r\n");
                tros::print("FS: sending reply...\r\n");
                let resp = [b'O', b'K'];
                tros::send(reply_ep, 0, &resp);
            }
            4 => {
                // APPEND: append data to existing storage
                let data_len = buf[2] as usize;
                if data_len > 0 && data_len <= 63 {
                    unsafe {
                        let current_len = (&raw const STORAGE_LEN as *const usize).read();
                        let new_len = current_len + data_len;
                        let new_len = if new_len > 64 { 64 } else { new_len };
                        let dst = (&raw mut STORAGE as *mut u8).add(current_len);
                        let src = buf.as_ptr().add(3);
                        for i in 0..(new_len - current_len) {
                            dst.add(i).write(src.add(i).read());
                        }
                        (&raw mut STORAGE_LEN as *mut usize).write(new_len);
                    }
                }
                tros::print("FS: appended data\r\n");
                let resp = [b'O', b'K'];
                tros::send(reply_ep, 0, &resp);
            }
            2 => {
                // READ: create a slice directly from static storage and send it.
                // Use raw pointer to avoid any stack aliasing with `buf`.
                unsafe {
                    let len = (&raw const STORAGE_LEN as *const usize).read();
                    let ptr = &raw const STORAGE as *const u8;
                    let data = core::slice::from_raw_parts(ptr, len);
                    tros::send(reply_ep, 0, data);
                }
                tros::print("FS: sent data\r\n");
            }
            _ => {
                tros::print("FS: unknown op=");
                let d = if opcode < 10 {
                    b'0' + opcode as u8
                } else {
                    b'X'
                };
                tros::putchar(d);
                tros::print("\r\n");
            }
        }
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
