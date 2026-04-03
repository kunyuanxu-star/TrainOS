//! Console output module
//! Implements console output via SBI calls with buffering for performance

/// Buffer size for console output
const CONSOLE_BUF_SIZE: usize = 256;

/// Console output buffer
static CONSOLE_BUF: spin::Mutex<ConsoleBuffer> = spin::Mutex::new(ConsoleBuffer::new());

/// Console buffer for batching writes
struct ConsoleBuffer {
    buf: [u8; CONSOLE_BUF_SIZE],
    pos: usize,
    flush_enabled: bool,
}

impl ConsoleBuffer {
    const fn new() -> Self {
        Self {
            buf: [0u8; CONSOLE_BUF_SIZE],
            pos: 0,
            flush_enabled: true,
        }
    }

    /// Add a character to buffer
    fn put_char(&mut self, c: u8) {
        if self.pos < CONSOLE_BUF_SIZE {
            self.buf[self.pos] = c;
            self.pos += 1;
        }
        // Auto-flush on newline or buffer full
        if c == b'\n' || self.pos >= CONSOLE_BUF_SIZE - 1 {
            self.flush();
        }
    }

    /// Flush buffer to console
    fn flush(&mut self) {
        if self.pos > 0 && self.flush_enabled {
            // Already holding lock, use unsafe to avoid deadlock
            unsafe {
                let ptr = self.buf.as_ptr();
                let mut i = 0usize;
                while i < self.pos {
                    let c = *ptr.add(i);
                    if c == b'\n' {
                        sbi_console_putchar_raw(b'\r' as usize);
                    }
                    sbi_console_putchar_raw(c as usize);
                    i += 1;
                }
            }
            self.pos = 0;
        }
    }

    /// Flush and disable further flushing
    fn panic_flush(&mut self) {
        self.flush_enabled = false;
        self.flush();
    }
}

/// SBI console putchar - outputs a single character (raw, no buffering)
#[inline(always)]
pub fn sbi_console_putchar_raw(c: usize) {
    unsafe {
        core::arch::asm!(
            "li a7, 1",
            "mv a0, {0}",
            "ecall",
            in(reg) c
        );
    }
}

/// SBI console putchar - outputs a single character (buffered)
pub fn sbi_console_putchar(c: usize) {
    let mut buf = CONSOLE_BUF.lock();
    buf.put_char(c as u8);
}

/// Write a string to console (buffered)
pub fn console_write(s: &str) {
    let mut buf = CONSOLE_BUF.lock();
    for &c in s.as_bytes() {
        buf.put_char(c);
    }
}

/// Write a single character to console (buffered)
pub fn console_write_char(c: char) {
    let mut buf = CONSOLE_BUF.lock();
    buf.put_char(c as u8);
}

/// Write a string to console without buffering (for panic messages)
pub fn console_write_raw(s: &str) {
    for &c in s.as_bytes() {
        if c == b'\n' {
            sbi_console_putchar_raw(b'\r' as usize);
        }
        sbi_console_putchar_raw(c as usize);
    }
}

/// Flush the console buffer
pub fn console_flush() {
    let mut buf = CONSOLE_BUF.lock();
    buf.flush();
}

/// Flush buffer for panic (called during panic handling)
pub fn panic_flush() {
    let mut buf = CONSOLE_BUF.lock();
    buf.panic_flush();
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

/// Print a number in hexadecimal
pub fn print_hex(val: usize) {
    if val == 0 {
        console_write("0");
        return;
    }
    let mut buf = [0u8; 16];
    let mut i = 0;
    let mut v = val;
    while v > 0 {
        let d = (v & 0xf) as u8;
        buf[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
        i += 1;
        v >>= 4;
    }
    for j in (0..i).rev() {
        console_write_char(buf[j] as char);
    }
}

/// Print a decimal number
pub fn print_dec(val: usize) {
    if val == 0 {
        console_write("0");
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = 0;
    let mut v = val;
    while v > 0 {
        buf[i] = b'0' + (v % 10) as u8;
        i += 1;
        v /= 10;
    }
    for j in (0..i).rev() {
        console_write_char(buf[j] as char);
    }
}
