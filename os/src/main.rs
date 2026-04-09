//! trainOS - A Rust Operating System for RISC-V
//!
//! This is the main entry point for the kernel.

#![no_std]
#![no_main]

mod boot;
pub mod console;
pub mod error;
mod elf;
mod memory;
mod process;
pub mod ipc;
mod trap;
mod fs;
mod syscall;
mod smp;
mod drivers;
mod net;
mod thread;
