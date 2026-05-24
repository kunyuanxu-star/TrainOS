// V23: VirtIO backend — forward guest VirtIO MMIO to host driver services
//
// This module decodes VirtIO MMIO register accesses from guest VMs and
// returns the appropriate response values.  For queue notifications, the
// access is forwarded to the host driver service via IPC (currently logged).
//
// The implementation covers the legacy MMIO transport as defined in the
// VirtIO 1.0 specification (Section 4.2.4), mapped within a single 4 KB
// MMIO page.

/// Decode a VirtIO MMIO access from a guest VM and handle it.
///
/// `vm_id`: guest VM identifier.
/// `addr`:  offset within the VirtIO MMIO region (0–0xFFF).
/// `is_write`: `true` if the guest is writing to the register.
/// `value`: the value being written (only meaningful when `is_write` is
///          `true`).
///
/// Returns the value to return to the guest for read accesses.  For write
/// accesses the return value is undefined (callers should ignore it).
pub fn handle_virtio_mmio(vm_id: u32, addr: usize, is_write: bool, value: u32) -> u32 {
    match addr & 0xFFF {
        // VirtIO MMIO Register Map (legacy transport)
        // Section 4.2.4 of the VirtIO 1.0 specification.

        0x000 => {
            // MagicValue — always returns "virt" (0x74726976)
            0x7472_6976
        }
        0x004 => {
            // Version — legacy device version
            0x2
        }
        0x008 => {
            // DeviceID — 2 = block device
            2
        }
        0x00C => {
            // VendorID — a fixed identifier
            0x554D_4551
        }
        0x010 => {
            // DeviceFeatures — no special features advertised
            0
        }
        0x034 => {
            // QueueNumMax — max queue depth reported to guest (256 entries)
            256
        }
        0x044 => {
            // QueueReady — not ready until guest writes 1
            0
        }
        0x060 => {
            // QueueNotify — guest kicks a virtqueue
            if is_write {
                forward_to_host(vm_id, value);
            }
            0
        }
        0x070 => {
            // DeviceStatus — start with reset state
            0
        }
        // All other register offsets are unimplemented: return 0.
        _ => 0,
    }
}

/// Forward a virtqueue notification to the host driver service.
///
/// In a production hypervisor this would send an IPC message to the
/// `drv` service.  For now, we log the event for debugging and
/// development purposes.
fn forward_to_host(vm_id: u32, queue_notify_value: u32) {
    crate::println!(
        "VirtIO: VM {} kicked queue notify={}",
        vm_id,
        queue_notify_value,
    );
}
