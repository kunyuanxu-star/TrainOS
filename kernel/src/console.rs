use core::fmt::{self, Write};

/// Output a string via SBI console putchar (eid=1).
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

/// Kernel console writer for use with `core::fmt::Write`.
pub struct KernelWriter;

impl Write for KernelWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        puts(s);
        Ok(())
    }
}

/// Print to the SBI console. Formatting arguments follow `core::fmt` syntax.
///
/// Use `print!` and `println!` instead of the manual digit-by-digit printing
/// that was previously scattered throughout the kernel.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::console::KernelWriter, $($arg)*);
    }};
}

/// Print to the SBI console with a trailing CRLF newline.
#[macro_export]
macro_rules! println {
    () => { $crate::print!("\r\n") };
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::console::KernelWriter, "{}\r\n", format_args!($($arg)*));
    }};
}
