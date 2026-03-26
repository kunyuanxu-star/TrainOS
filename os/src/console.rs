//! Console output module
//! Implements console output via SBI calls

/// SBI console putchar - outputs a single character
fn sbi_console_putchar(c: usize) {
    unsafe {
        core::arch::asm!(
            "li a7, 1",
            "mv a0, {0}",
            "ecall",
            in(reg) c
        );
    }
}

/// Write a string to console
pub fn console_write(s: &str) {
    for &c in s.as_bytes() {
        if c == b'\n' {
            sbi_console_putchar(b'\r' as usize);
        }
        sbi_console_putchar(c as usize);
    }
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::console::console_write("\r\n")
    };
    ($s:expr) => {
        $crate::console::console_write($s);
        $crate::console::console_write("\r\n");
    };
}

#[macro_export]
macro_rules! print {
    ($s:expr) => {
        $crate::console::console_write($s);
    };
}
