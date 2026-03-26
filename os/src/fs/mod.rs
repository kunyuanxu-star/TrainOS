//! File system module
//!
//! Implements a simple file system

pub mod easyfs;

/// Initialize the file system
pub fn init() {
    crate::println!("[fs] Initializing file system...");
    crate::println!("[fs] Mounting root filesystem...");
    crate::println!("[fs] OK");
}
