//! File system module
//!
//! Provides VFS layer and file system implementations

pub mod easyfs;
pub mod vfs;
pub mod devfs;
pub mod ramfs;

/// Initialize the file system
pub fn init() {
    crate::println!("[fs] Initializing file system...");

    // Initialize VFS
    vfs::init();

    // Initialize RAM filesystem
    ramfs::init();

    // Initialize device file system
    crate::println!("[fs] Mounting devfs...");
    // devfs would be mounted here in a full implementation

    crate::println!("[fs] File system initialized");
}
