//! MMIO access helpers via syscalls
//!
//! Provides safe wrappers around the DEVICE_READ and DEVICE_WRITE syscalls.

/// Device IDs
pub const DEVICE_VIRTIO_BLK: usize = 0;
pub const DEVICE_VIRTIO_NET: usize = 1;

/// Read a u32 from MMIO
pub fn read32(device_id: usize, offset: usize) -> u32 {
    let val: isize;
    unsafe {
        core::arch::asm!(
            "mv a0, {0}",
            "mv a1, {1}",
            "li a2, 4",
            "li a7, 1100",
            "ecall",
            "mv {val}, a0",
            val = out(reg) _,
            in(reg) device_id,
            in(reg) offset,
        );
    }
    val as u32
}

/// Write a u32 to MMIO
pub fn write32(device_id: usize, offset: usize, value: u32) {
    unsafe {
        core::arch::asm!(
            "mv a0, {0}",
            "mv a1, {1}",
            "mv a2, {2}",
            "li a3, 4",
            "li a7, 1101",
            "ecall",
            in(reg) device_id,
            in(reg) offset,
            in(reg) value,
        );
    }
}

/// Read a u8 from MMIO
pub fn read8(device_id: usize, offset: usize) -> u8 {
    let val: isize;
    unsafe {
        core::arch::asm!(
            "mv a0, {0}",
            "mv a1, {1}",
            "li a2, 1",
            "li a7, 1100",
            "ecall",
            "mv {val}, a0",
            val = out(reg) _,
            in(reg) device_id,
            in(reg) offset,
        );
    }
    val as u8
}

/// Write a u8 to MMIO
pub fn write8(device_id: usize, offset: usize, value: u8) {
    unsafe {
        core::arch::asm!(
            "mv a0, {0}",
            "mv a1, {1}",
            "mv a2, {2}",
            "li a3, 1",
            "li a7, 1101",
            "ecall",
            in(reg) device_id,
            in(reg) offset,
            in(reg) value as usize,
        );
    }
}