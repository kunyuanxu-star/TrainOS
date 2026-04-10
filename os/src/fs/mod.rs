//! File system module
//!
//! Provides VFS layer and file system implementations

pub mod easyfs;
pub mod vfs;
pub mod devfs;
pub mod ramfs;

/// Initialize the file system
pub fn init() {
    for c in b"[fs] init start\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    for c in b"[fs] Initializing file system...\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    for c in b"[fs] Calling vfs::init\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }

    // Initialize VFS
    vfs::init();

    for c in b"[fs] vfs::init done, calling ramfs::init\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    // Initialize RAM filesystem
    ramfs::init();

    for c in b"[fs] ramfs::init done\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    // Initialize device file system
    for c in b"[fs] Mounting devfs...\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
    // devfs would be mounted here in a full implementation

    for c in b"[fs] File system initialized\r\n" { crate::console::sbi_console_putchar_raw(*c as usize); }
}
