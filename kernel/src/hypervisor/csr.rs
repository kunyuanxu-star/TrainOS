// V23: RISC-V H-extension CSR wrappers
//
// Provides safe abstractions for reading/writing H-extension CSRs.
// These instructions are only valid when running in HS-mode (hypervisor
// supervisor mode) with H-extension enabled. When running in S-mode
// without H-extension, they will raise illegal-instruction exceptions.
//
// CSR address map (H-extension):
//   HS-mode CSRs: hstatus=0x600, hedeleg=0x602, hideleg=0x603,
//                 hgatp=0x680
//   VS-mode CSRs: vsstatus=0x200, vstvec=0x205

use core::arch::asm;

// ── HS-mode CSRs ──────────────────────────────────────────────────────────────

/// Read hgatp register (G-stage page-table base address).
#[inline]
pub unsafe fn hgatp_read() -> usize {
    let val: usize;
    asm!("csrr {}, 0x680", out(reg) val, options(nostack));
    val
}

/// Write hgatp register (G-stage page-table base address).
#[inline]
pub unsafe fn hgatp_write(val: usize) {
    asm!("csrw 0x680, {}", in(reg) val, options(nostack));
}

/// Read hstatus register (hypervisor status).
#[inline]
pub unsafe fn hstatus_read() -> usize {
    let val: usize;
    asm!("csrr {}, 0x600", out(reg) val, options(nostack));
    val
}

/// Write hstatus register (hypervisor status).
#[inline]
pub unsafe fn hstatus_write(val: usize) {
    asm!("csrw 0x600, {}", in(reg) val, options(nostack));
}

/// Read hedeleg register (hypervisor exception delegation).
#[inline]
pub unsafe fn hedeleg_read() -> usize {
    let val: usize;
    asm!("csrr {}, 0x602", out(reg) val, options(nostack));
    val
}

/// Write hedeleg register (hypervisor exception delegation).
#[inline]
pub unsafe fn hedeleg_write(val: usize) {
    asm!("csrw 0x602, {}", in(reg) val, options(nostack));
}

/// Read hideleg register (hypervisor interrupt delegation).
#[inline]
pub unsafe fn hideleg_read() -> usize {
    let val: usize;
    asm!("csrr {}, 0x603", out(reg) val, options(nostack));
    val
}

/// Write hideleg register (hypervisor interrupt delegation).
#[inline]
pub unsafe fn hideleg_write(val: usize) {
    asm!("csrw 0x603, {}", in(reg) val, options(nostack));
}

// ── VS-mode CSRs ──────────────────────────────────────────────────────────────

/// Read vsstatus register (virtual supervisor status).
#[inline]
pub unsafe fn vsstatus_read() -> usize {
    let val: usize;
    asm!("csrr {}, 0x200", out(reg) val, options(nostack));
    val
}

/// Write vsstatus register (virtual supervisor status).
#[inline]
pub unsafe fn vsstatus_write(val: usize) {
    asm!("csrw 0x200, {}", in(reg) val, options(nostack));
}

/// Read vstvec register (virtual supervisor trap vector).
#[inline]
pub unsafe fn vstvec_read() -> usize {
    let val: usize;
    asm!("csrr {}, 0x205", out(reg) val, options(nostack));
    val
}

/// Write vstvec register (virtual supervisor trap vector).
#[inline]
pub unsafe fn vstvec_write(val: usize) {
    asm!("csrw 0x205, {}", in(reg) val, options(nostack));
}
