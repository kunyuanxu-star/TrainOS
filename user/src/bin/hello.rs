//! Hello world user program

#![no_std]
#![no_main]

#[no_mangle]
extern "C" fn main() {
    // Write "Hello from trainOS!\n" to stdout (fd=1)
    let msg = b"Hello from trainOS!\n";

    // In a real system, this would be a syscall
    // For now, just return
    let _ = msg;
}
