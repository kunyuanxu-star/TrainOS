//! trainOS - A Rust Operating System for RISC-V
//!
//! This is the main entry point for the kernel.

#![no_std]
#![no_main]

mod boot;
pub mod console;
mod memory;
mod process;
mod trap;
mod fs;
mod syscall;
