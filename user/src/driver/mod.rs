//! Driver service - user-space VirtIO driver
//!
//! This module contains the driver implementations that run as
//! user-space processes and access devices via syscalls.

#![no_std]

pub mod virtio_blk;
pub mod virtio_net;
pub mod mmio;