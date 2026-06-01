// V23: RISC-V H-extension Hypervisor subsystem
//
// Features: VM creation/destroy, G-stage two-stage address translation,
// virtualized CSR access (HS-mode and VS-mode), VM pause/resume lifecycle.
//
// Architecture:
//   Each VM owns a pre-allocated G-stage page table (see mmu.rs) that maps
//   the guest physical address space to host physical pages.  The VM context
//   stores the trap frame that would be used when returning to/handling traps
//   from VS-mode.

pub mod csr;
pub mod mmu;
pub mod plic;
pub mod snapshot;
pub mod timer;
pub mod virtio;
pub mod vs_aia;

/// Maximum number of concurrent virtual machines.
const MAX_VMS: usize = 8;

/// Maximum length of a VM name (excluding null terminator).
const VM_NAME_LEN: usize = 32;

/// Saved execution context for a virtual CPU.
///
/// When the VM is paused or has not yet been started, these fields hold the
/// register state that must be restored before the VM can resume VS-mode
/// execution.  Layout mirrors the trap frame that the hypervisor would save
/// on a world switch.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct VmContext {
    // General-purpose registers
    gprs: [usize; 32],
    // Supervisor-mode trap state
    sepc: usize,
    sstatus: usize,
    stvec: usize,
    sscratch: usize,
    scause: usize,
    // G-stage translation
    hgatp: usize,
    // Host physical address of the L2 (root) G-stage page table page
    l2_phys: usize,
    // Guest memory size in MB
    mem_mb: usize,
}

impl VmContext {
    const fn empty() -> Self {
        VmContext {
            gprs: [0; 32],
            sepc: 0,
            sstatus: 0,
            stvec: 0,
            sscratch: 0,
            scause: 0,
            hgatp: 0,
            l2_phys: 0,
            mem_mb: 0,
        }
    }
}

/// A virtual machine instance.
#[derive(Clone, Copy, Debug)]
struct VirtualMachine {
    vm_id: u32,
    /// Human-readable name (null-terminated byte string).
    name: [u8; VM_NAME_LEN],
    /// Saved execution context.
    ctx: VmContext,
    /// Whether the VM is currently running (executing in VS-mode).
    running: bool,
    /// Whether this slot is occupied by a live VM.
    active: bool,
}

/// Global VM table.  Protected by the fact that all VM operations go through
/// the syscall dispatcher (single-threaded w.r.t. VM mutations in the kernel).
static mut VMS: [VirtualMachine; MAX_VMS] = unsafe_zeroed_vms();
static mut VM_ID_COUNTER: u32 = 1; // VM IDs start at 1

/// Helper to produce a zeroed VM array at compile time.
const fn unsafe_zeroed_vms() -> [VirtualMachine; MAX_VMS] {
    // Safety: every field in VirtualMachine is an integer type or a fixed-size
    // array thereof, so all-zeroes is a valid initial state (active=false).
    //
    // This const fn avoids a dependency on Default while keeping the array
    // static.
    unsafe { core::mem::transmute([0u8; core::mem::size_of::<VirtualMachine>() * MAX_VMS]) }
}

/// Find an active VM by its ID.  Returns a mutable pointer or None.
unsafe fn vm_by_id(vm_id: u32) -> Option<*mut VirtualMachine> {
    for vm in VMS.iter_mut() {
        if vm.active && vm.vm_id == vm_id {
            return Some(vm as *mut VirtualMachine);
        }
    }
    None
}

/// Find the first inactive slot.  Returns a mutable pointer or None.
unsafe fn free_slot() -> Option<*mut VirtualMachine> {
    for vm in VMS.iter_mut() {
        if !vm.active {
            return Some(vm as *mut VirtualMachine);
        }
    }
    None
}

// ── Public API ─────────────────────────────────────────────────────────────────

/// Create a new virtual machine.
///
/// `name` is a byte slice (may be shorter than `VM_NAME_LEN`; only the first
/// `VM_NAME_LEN-1` bytes are stored).  `memory_mb` specifies the size of the
/// guest physical memory in megabytes (must be ? 128).
///
/// Returns the new VM ID on success, or `None` if the table is full or memory
/// allocation fails.
pub fn vm_create(name: &[u8], memory_mb: usize) -> Option<u32> {
    // 1. Set up G-stage page tables
    let (hgatp, l2_phys) = mmu::create_gstage(memory_mb).ok()?;

    // 2. Find a free slot
    let vm_ptr = unsafe { free_slot()? };

    // 3. Build the VM name (null-terminated)
    let mut name_buf = [0u8; VM_NAME_LEN];
    let copy_len = core::cmp::min(name.len(), VM_NAME_LEN - 1);
    name_buf[..copy_len].copy_from_slice(&name[..copy_len]);

    // 4. Allocate a VM ID
    let vm_id = unsafe {
        let id = VM_ID_COUNTER;
        VM_ID_COUNTER = VM_ID_COUNTER.wrapping_add(1);
        id
    };

    // 5. Initialise the VM context
    let ctx = VmContext {
        gprs: [0; 32],
        sepc: 0,
        sstatus: 0,
        stvec: 0,
        sscratch: 0,
        scause: 0,
        hgatp,
        l2_phys,
        mem_mb: memory_mb,
    };

    unsafe {
        (*vm_ptr).vm_id = vm_id;
        (*vm_ptr).name = name_buf;
        (*vm_ptr).ctx = ctx;
        (*vm_ptr).running = false;
        (*vm_ptr).active = true;
    }

    Some(vm_id)
}

/// Destroy a virtual machine.
///
/// Frees all G-stage page-table pages and backing pages, then marks the slot
/// as inactive.  Returns `true` on success, `false` if the VM ID was not found.
pub fn vm_destroy(vm_id: u32) -> bool {
    unsafe {
        let vm_ptr = match vm_by_id(vm_id) {
            Some(p) => p,
            None => return false,
        };

        // Free G-stage resources
        let l2_phys = (*vm_ptr).ctx.l2_phys;
        if l2_phys != 0 {
            mmu::destroy_gstage(l2_phys);
        }

        // Mark slot inactive
        (*vm_ptr).active = false;
        (*vm_ptr).running = false;
    }
    true
}

/// Start a virtual machine by setting its initial entry point.
///
/// In a real hypervisor, this would perform an `sret` into VS-mode at
/// `entry_pc`.  For now, we configure the context so that when the vCPU
/// is later dispatched, execution begins at `entry_pc`.  Returns `true`
/// on success, `false` if the VM was not found or already running.
pub fn vm_start(vm_id: u32, entry_pc: usize) -> bool {
    unsafe {
        let vm_ptr = match vm_by_id(vm_id) {
            Some(p) => p,
            None => return false,
        };

        if (*vm_ptr).running {
            return false; // already running
        }

        // Set the entry point
        (*vm_ptr).ctx.sepc = entry_pc;

        // Initial SSTATUS value: SPP=0 (return to VS-mode, not HS-mode),
        // SPIE=0, SUM=1 (allow VS-mode to access VU-mode pages).
        (*vm_ptr).ctx.sstatus = 1 << 18; // SUM bit

        // Mark as running
        (*vm_ptr).running = true;
    }
    true
}

/// Pause a running virtual machine.
///
/// In a real hypervisor this would trigger a VS-mode interrupt or trap to
/// save the guest context.  For now we simply flip the state flag.
/// Returns `true` on success, `false` if the VM was not found or not running.
pub fn vm_pause(vm_id: u32) -> bool {
    unsafe {
        let vm_ptr = match vm_by_id(vm_id) {
            Some(p) => p,
            None => return false,
        };

        if !(*vm_ptr).running {
            return false;
        }

        (*vm_ptr).running = false;
    }
    true
}

/// Resume a paused virtual machine.
///
/// Returns `true` on success, `false` if the VM was not found or already
/// running.
pub fn vm_resume(vm_id: u32) -> bool {
    unsafe {
        let vm_ptr = match vm_by_id(vm_id) {
            Some(p) => p,
            None => return false,
        };

        if (*vm_ptr).running {
            return false; // already running
        }

        (*vm_ptr).running = true;
    }
    true
}

/// List all active VMs into the provided byte buffer.
///
/// Format per VM (8 bytes):
///   [0..3] VM ID (little-endian u32)
///   [4]    running flag (1 = running, 0 = paused)
///   [5]    pad / reserved
///   [6..7] pad / reserved
///
/// Returns the number of bytes written.
pub fn vm_list(buf: &mut [u8]) -> usize {
    unsafe {
        let mut pos = 0;
        for vm in VMS.iter() {
            if !vm.active {
                continue;
            }
            if pos + 8 > buf.len() {
                break;
            }
            let id = vm.vm_id;
            buf[pos] = id as u8;
            buf[pos + 1] = (id >> 8) as u8;
            buf[pos + 2] = (id >> 16) as u8;
            buf[pos + 3] = (id >> 24) as u8;
            buf[pos + 4] = if vm.running { 1 } else { 0 };
            buf[pos + 5] = 0;
            buf[pos + 6] = 0;
            buf[pos + 7] = 0;
            pos += 8;
        }
        pos
    }
}

// ── Module-level tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_create_destroy() {
        let id = vm_create(b"test-vm", 2).expect("vm_create should succeed");
        assert!(id > 0);
        assert!(vm_destroy(id));
    }

    #[test]
    fn test_vm_create_invalid_size() {
        assert!(vm_create(b"bad", 0).is_none());
        assert!(vm_create(b"bad", 256).is_none());
    }

    #[test]
    fn test_vm_start_pause_resume() {
        let id = vm_create(b"lifecycle", 2).unwrap();

        // Start
        assert!(vm_start(id, 0x8020_0000));
        // Second start should fail (already running)
        assert!(!vm_start(id, 0x8020_0000));

        // Pause
        assert!(vm_pause(id));
        // Second pause should fail (already paused)
        assert!(!vm_pause(id));

        // Resume
        assert!(vm_resume(id));
        // Second resume should fail (already running)
        assert!(!vm_resume(id));

        vm_destroy(id);
    }

    #[test]
    fn test_vm_list() {
        let a = vm_create(b"alpha", 2).unwrap();
        let b = vm_create(b"beta", 2).unwrap();
        let _ = vm_start(a, 0x1000);

        let mut buf = [0u8; 32];
        let written = vm_list(&mut buf);
        // 2 VMs x 8 bytes = 16
        assert_eq!(written, 16);

        // Alpha should be running (flag=1)
        assert_eq!(buf[4], 1);
        // Beta should be paused (flag=0)
        assert_eq!(buf[12], 0);

        vm_destroy(a);
        vm_destroy(b);
    }
}
