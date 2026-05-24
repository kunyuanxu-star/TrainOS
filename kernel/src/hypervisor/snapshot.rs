// V23: VM snapshot and restore
//
// Provides `snapshot()` and `restore()` functions that serialise and
// deserialise the full state of a `VirtualMachine` (general-purpose
// registers, CSRs, and metadata) into/from a byte buffer.
//
// The serialised format is little-endian and consists of:
//
//   [0..4)   Magic number 0x564D_534E ("VMSN")
//   [4..8)   VM ID (u32 LE)
//   [8..40)  VM name (32 bytes, null-terminated byte string)
//   [40..296)  GPRs x0–x31 (32 × 8 = 256 bytes)
//   [296..336) CSRs: sepc, sstatus, stvec, sscratch, scause (5 × 8 = 40 bytes)
//   [336..360) hgatp, l2_phys, mem_mb (3 × 8 = 24 bytes)
//   [360)     running flag (1 byte)
//   [361)     active flag (1 byte)
//   Total: 362 bytes

/// Magic value identifying a valid snapshot buffer: "VMSN" in ASCII.
const SNAPSHOT_MAGIC: u32 = 0x564D_534E;

/// Total size of a snapshot in bytes.
pub const SNAPSHOT_SIZE: usize = 362;

// ── Little-endian I/O helpers ─────────────────────────────────────────────

/// Write a `u32` in little-endian format into `buf` at position `pos`,
/// advancing `pos` by 4.
#[inline]
fn write_u32_le(buf: &mut [u8], pos: &mut usize, val: u32) {
    buf[*pos] = val as u8;
    buf[*pos + 1] = (val >> 8) as u8;
    buf[*pos + 2] = (val >> 16) as u8;
    buf[*pos + 3] = (val >> 24) as u8;
    *pos += 4;
}

/// Write a `usize` in little-endian format into `buf` at position `pos`,
/// advancing `pos` by 8 (always 8 bytes, matching rv64).
#[inline]
fn write_usize_le(buf: &mut [u8], pos: &mut usize, val: usize) {
    for i in 0..8 {
        buf[*pos + i] = (val >> (i * 8)) as u8;
    }
    *pos += 8;
}

/// Read a `u32` in little-endian format from `buf` at position `pos`,
/// advancing `pos` by 4.
#[inline]
fn read_u32_le(buf: &[u8], pos: &mut usize) -> u32 {
    let v = (buf[*pos] as u32)
        | ((buf[*pos + 1] as u32) << 8)
        | ((buf[*pos + 2] as u32) << 16)
        | ((buf[*pos + 3] as u32) << 24);
    *pos += 4;
    v
}

/// Read a `usize` in little-endian format from `buf` at position `pos`,
/// advancing `pos` by 8.
#[inline]
fn read_usize_le(buf: &[u8], pos: &mut usize) -> usize {
    let mut val = 0usize;
    for i in 0..8 {
        val |= (buf[*pos + i] as usize) << (i * 8);
    }
    *pos += 8;
    val
}

// ── Public API ────────────────────────────────────────────────────────────

/// Serialise a `VirtualMachine` into a byte buffer.
///
/// Returns the number of bytes written (`SNAPSHOT_SIZE`).  If the buffer is
/// too small, returns 0 without writing anything.
pub(super) fn snapshot(vm: &super::VirtualMachine, buf: &mut [u8]) -> usize {
    if buf.len() < SNAPSHOT_SIZE {
        return 0;
    }

    let mut pos = 0usize;

    // 1. Magic number (4 bytes)
    write_u32_le(buf, &mut pos, SNAPSHOT_MAGIC);

    // 2. VM ID (4 bytes)
    write_u32_le(buf, &mut pos, vm.vm_id);

    // 3. VM name (32 bytes, null-terminated)
    for &byte in vm.name.iter() {
        buf[pos] = byte;
        pos += 1;
    }

    // 4. GPRs x0–x31 (32 × 8 = 256 bytes)
    for &reg in vm.ctx.gprs.iter() {
        write_usize_le(buf, &mut pos, reg);
    }

    // 5. CSRs (5 × 8 = 40 bytes)
    write_usize_le(buf, &mut pos, vm.ctx.sepc);
    write_usize_le(buf, &mut pos, vm.ctx.sstatus);
    write_usize_le(buf, &mut pos, vm.ctx.stvec);
    write_usize_le(buf, &mut pos, vm.ctx.sscratch);
    write_usize_le(buf, &mut pos, vm.ctx.scause);

    // 6. G-stage metadata (3 × 8 = 24 bytes)
    write_usize_le(buf, &mut pos, vm.ctx.hgatp);
    write_usize_le(buf, &mut pos, vm.ctx.l2_phys);
    write_usize_le(buf, &mut pos, vm.ctx.mem_mb);

    // 7. Running flag (1 byte)
    buf[pos] = if vm.running { 1 } else { 0 };
    pos += 1;

    // 8. Active flag (1 byte)
    buf[pos] = if vm.active { 1 } else { 0 };
    pos += 1;

    debug_assert_eq!(pos, SNAPSHOT_SIZE);
    pos
}

/// Deserialise a `VirtualMachine` from a byte buffer previously written by
/// `snapshot()`.
///
/// Returns `Ok(())` on success, or an `Err` with a description of what went
/// wrong (bad magic, truncated buffer, etc.).
pub(super) fn restore(vm: &mut super::VirtualMachine, buf: &[u8]) -> Result<(), &'static str> {
    if buf.len() < SNAPSHOT_SIZE {
        return Err("snapshot buffer too small");
    }

    let mut pos = 0usize;

    // 1. Magic number
    let magic = read_u32_le(buf, &mut pos);
    if magic != SNAPSHOT_MAGIC {
        return Err("invalid snapshot magic");
    }

    // 2. VM ID
    vm.vm_id = read_u32_le(buf, &mut pos);

    // 3. VM name
    for byte in vm.name.iter_mut() {
        *byte = buf[pos];
        pos += 1;
    }

    // 4. GPRs
    for reg in vm.ctx.gprs.iter_mut() {
        *reg = read_usize_le(buf, &mut pos);
    }

    // 5. CSRs
    vm.ctx.sepc = read_usize_le(buf, &mut pos);
    vm.ctx.sstatus = read_usize_le(buf, &mut pos);
    vm.ctx.stvec = read_usize_le(buf, &mut pos);
    vm.ctx.sscratch = read_usize_le(buf, &mut pos);
    vm.ctx.scause = read_usize_le(buf, &mut pos);

    // 6. G-stage metadata
    vm.ctx.hgatp = read_usize_le(buf, &mut pos);
    vm.ctx.l2_phys = read_usize_le(buf, &mut pos);
    vm.ctx.mem_mb = read_usize_le(buf, &mut pos);

    // 7. Running flag
    vm.running = buf[pos] != 0;
    pos += 1;

    // 8. Active flag
    vm.active = buf[pos] != 0;
    pos += 1;

    debug_assert_eq!(pos, SNAPSHOT_SIZE);
    Ok(())
}
