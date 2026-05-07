/// Output a string via SBI console putchar (eid=1).
/// This is a debug facility; real console goes through user-space drivers.
pub fn puts(s: &str) {
    for byte in s.bytes() {
        sbi_putchar(byte);
    }
}

fn sbi_putchar(c: u8) {
    unsafe {
        core::arch::asm!(
            "ecall",
            in("a7") 1,       // sbi_console_putchar
            in("a0") c as usize,
        );
    }
}
