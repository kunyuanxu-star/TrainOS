use crate::mem::{buddy, sv39};
use crate::proc::process::ProcessState;

pub fn sys_spawn(_elf_ptr: usize, _elf_len: usize) -> Result<usize, &'static str> {
    // In a real implementation, copy ELF from user space
    // For now, this is a placeholder
    Err("spawn not implemented via syscall")
}

pub fn sys_exit(_code: i32) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no proc")?;
    unsafe {
        let current = crate::sched::current_thread().ok_or("no thread")?;
        (*current).state = crate::proc::thread::ThreadState::Dead;
    }
    // Also mark the Process as Dead so waitpid can find it
    let mut procs = crate::proc::PROCESSES.lock();
    if let Some(proc) = procs.iter_mut().find(|p| p.pid == pid) {
        proc.state = ProcessState::Dead;
    }
    drop(procs);

    crate::sched::schedule();
    // Never returns
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// Map a physical MMIO region into the current process's page table.
/// phys: physical base address (must be page-aligned)
/// size: region size in bytes (will be rounded up to page boundary)
/// Returns: virtual address of the mapping
pub fn sys_mmio_map(phys: usize, size: usize) -> Result<usize, &'static str> {
    if phys == 0 || size == 0 {
        return Err("invalid args");
    }
    if phys & 0xFFF != 0 {
        return Err("phys not page-aligned");
    }

    // Get current process
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no current process")?;

    let procs = crate::proc::PROCESSES.lock();
    let proc = procs
        .iter()
        .find(|p| p.pid == pid)
        .ok_or("process not found")?;
    let pt_root = proc.page_table_root;
    drop(procs);

    let pages = (size + 0xFFF) >> 12;

    // Use a fixed MMIO virtual address region for user space
    // Map at 0x20000000 + offset (in user space)
    let vbase = 0x2000_0000;

    for i in 0..pages {
        let pa = phys + i * 0x1000;
        let va = vbase + i * 0x1000;
        unsafe {
            crate::proc::elf::map_into_pt(pt_root, va, pa, true, true, false, true);
        }
    }

    Ok(vbase)
}

/// Fork the current process. Returns 0 in child, child_pid in parent.
/// `parent_sepc` is the saved sepc from the current trap frame (address of ecall instruction).
pub fn sys_fork(parent_sepc: usize) -> Result<usize, &'static str> {
    // Get current process
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no current process")?;

    let (pt_root, user_sp, _satp_val, priority) = {
        let procs = crate::proc::PROCESSES.lock();
        let proc = procs
            .iter()
            .find(|p| p.pid == pid)
            .ok_or("process not found")?;
        let thread = proc.thread.as_ref().ok_or("no thread")?;
        (
            proc.page_table_root,
            thread.trap_frame.as_ref().unwrap().user_sp,
            thread.trap_frame.as_ref().unwrap().satp,
            thread.effective_priority,
        )
    };

    // Child entry point = instruction after the ecall (sepc + 4)
    let child_entry = parent_sepc + 4;

    // Create child page table: share parent's mappings (no page copying)
    let child_pt = buddy::alloc_page().ok_or("OOM")?;

    unsafe {
        let child_pt_kva = sv39::pa_to_kva(child_pt);
        core::ptr::write_bytes(child_pt_kva as *mut u8, 0, 4096);

        // Copy kernel mappings (VPN2 >= 256, plus identity mappings at VPN2=0,2)
        sv39::copy_kernel_mappings(child_pt);

        // Deep-copy user-space page table entries (VPN2 0..256).
        // This allocates new L1/L0 pages and copies writable page content.
        copy_user_mappings_full(pt_root, child_pt)?;
    }

    let child_satp = sv39::make_satp(child_pt);
    let child_pid = crate::proc::fork_child(
        child_pt,
        pt_root,
        child_entry,
        user_sp,
        child_satp,
        priority,
        pid,
    )
    .ok_or("fork_child failed")?;

    Ok(child_pid as usize)
}

/// Copy user-space page table entries from parent to child,
/// doing a full page copy for all writable pages (no COW).
unsafe fn copy_user_mappings_full(parent_pt: usize, child_pt: usize) -> Result<(), &'static str> {
    use crate::mem::sv39::{self, PTE};

    // Walk the parent's L2 entries for user space (VPN2 = 0..256)
    let parent_l2 = &*(sv39::pa_to_kva(parent_pt) as *const [PTE; 512]);
    let child_l2 = &mut *(sv39::pa_to_kva(child_pt) as *mut [PTE; 512]);

    // Only copy user-space entries (VPN2 < 256, lower half of Sv39)
    for vpn2_idx in 0..256 {
        let l2_entry = parent_l2[vpn2_idx];
        if !l2_entry.is_valid() {
            continue;
        }
        if l2_entry.is_leaf() {
            continue;
        } // skip superpages at L2

        // Allocate and copy L1 page
        let parent_l1_phys = l2_entry.phys_addr();
        let child_l1_phys = buddy::alloc_page().ok_or("OOM")?;
        // Zero child L1 page
        core::ptr::write_bytes(sv39::pa_to_kva(child_l1_phys) as *mut u8, 0, 4096);

        let parent_l1 = &*(sv39::pa_to_kva(parent_l1_phys) as *const [PTE; 512]);
        let child_l1 = &mut *(sv39::pa_to_kva(child_l1_phys) as *mut [PTE; 512]);

        // Set the L2 entry in child
        let mut new_l2_entry = PTE::empty();
        new_l2_entry.set_ppn(child_l1_phys >> 12);
        new_l2_entry.set_flags(false, false, false, false); // branch: R=W=X=0
        child_l2[vpn2_idx] = new_l2_entry;

        // Copy L1 entries
        for vpn1_idx in 0..512 {
            let l1_entry = parent_l1[vpn1_idx];
            if !l1_entry.is_valid() {
                continue;
            }

            if l1_entry.is_leaf() {
                // 2MB superpage — just share (read-only in practice, or copy on write later)
                child_l1[vpn1_idx] = l1_entry;
            } else {
                // Branch to L0 page
                let parent_l0_phys = l1_entry.phys_addr();
                let child_l0_phys = buddy::alloc_page().ok_or("OOM")?;
                // Zero child L0 page
                core::ptr::write_bytes(sv39::pa_to_kva(child_l0_phys) as *mut u8, 0, 4096);

                let parent_l0 = &*(sv39::pa_to_kva(parent_l0_phys) as *const [PTE; 512]);
                let child_l0 = &mut *(sv39::pa_to_kva(child_l0_phys) as *mut [PTE; 512]);

                let mut new_l1_entry = PTE::empty();
                new_l1_entry.set_ppn(child_l0_phys >> 12);
                new_l1_entry.set_flags(false, false, false, false); // branch: R=W=X=0
                child_l1[vpn1_idx] = new_l1_entry;

                // Copy L0 entries — for writable pages, allocate new page in child
                for vpn0_idx in 0..512 {
                    let l0_entry = parent_l0[vpn0_idx];
                    if !l0_entry.is_valid() || !l0_entry.is_leaf() {
                        continue;
                    }

                    if l0_entry.is_writable() || l0_entry.is_dirty() {
                        // Allocate a new physical page for the child
                        let new_page = buddy::alloc_page().ok_or("OOM")?;
                        let old_kva = sv39::pa_to_kva(l0_entry.phys_addr());
                        let new_kva = sv39::pa_to_kva(new_page);
                        // Copy page content from parent to child
                        core::ptr::copy_nonoverlapping(
                            old_kva as *const u8,
                            new_kva as *mut u8,
                            4096,
                        );

                        // Create writable PTE for child
                        let mut child_pte = PTE::empty();
                        child_pte.set_ppn(new_page >> 12);
                        child_pte.set_flags(true, true, l0_entry.is_executable(), true);
                        child_pte.set_accessed(true);
                        child_pte.set_dirty(true);
                        child_l0[vpn0_idx] = child_pte;
                    } else {
                        // Read-only — share the physical page
                        child_l0[vpn0_idx] = l0_entry;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Fill a buffer with process information.
/// Format per process: [pid:4][prio:1][state:1] = 6 bytes each
/// Returns number of processes written.
/// Only includes processes with valid (non-Dead) state.
pub fn sys_proclist(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    let procs = crate::proc::PROCESSES.lock();
    let alive_count = procs
        .iter()
        .filter(|p| p.state != ProcessState::Dead)
        .count();
    let count = alive_count.min(buf_len / 6);

    let mut written = 0;
    for proc in procs.iter() {
        if proc.state == ProcessState::Dead {
            continue;
        }
        if written >= count {
            break;
        }

        let off = written * 6;
        unsafe {
            let buf = buf_ptr as *mut u8;
            // pid (4 bytes, little-endian)
            buf.add(off).write((proc.pid & 0xFF) as u8);
            buf.add(off + 1).write(((proc.pid >> 8) & 0xFF) as u8);
            buf.add(off + 2).write(((proc.pid >> 16) & 0xFF) as u8);
            buf.add(off + 3).write(((proc.pid >> 24) & 0xFF) as u8);
            // priority
            buf.add(off + 4).write(proc.base_priority);
            // state: 0=Ready, 1=Running, 2=Waiting, 3=Dead
            buf.add(off + 5).write(proc.state as u8);
        }
        written += 1;
    }

    Ok(written)
}

/// Kill a process by PID.
pub fn sys_kill(pid: u32) -> Result<usize, &'static str> {
    let mut procs = crate::proc::PROCESSES.lock();
    if let Some(proc) = procs.iter_mut().find(|p| p.pid == pid) {
        proc.state = ProcessState::Dead;
        // Mark any blocked thread as Dead too
        if let Some(ref mut thread) = proc.thread {
            if thread.state == crate::proc::thread::ThreadState::Waiting {
                thread.state = crate::proc::thread::ThreadState::Dead;
            }
        }
        Ok(0)
    } else {
        Err("process not found")
    }
}

/// Map a shared memory page. Shares caller's page at vaddr with target_pid.
/// Returns the shared virtual address in the target process.
pub fn sys_shm_map(target_pid: u32, vaddr: usize) -> Result<usize, &'static str> {
    let caller_pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no proc")?;

    if caller_pid == target_pid {
        return Err("cannot share with self");
    }

    // Get both page table roots from the process table
    let (caller_pt, target_pt) = {
        let procs = crate::proc::PROCESSES.lock();
        let caller = procs.iter().find(|p| p.pid == caller_pid).ok_or("caller not found")?;
        let caller_pt = caller.page_table_root;
        let target = procs.iter().find(|p| p.pid == target_pid).ok_or("target not found")?;
        let target_pt = target.page_table_root;
        (caller_pt, target_pt)
    };

    // Translate caller's virtual address to physical address
    let phys = unsafe {
        crate::proc::elf::virt_to_phys_in_pt(caller_pt, vaddr).ok_or("bad vaddr")?
    };

    // Map the same physical page into target's page table at the same VA
    let target_va = vaddr;
    unsafe {
        crate::proc::elf::map_into_pt(target_pt, target_va, phys, true, true, false, true);
    }

    Ok(target_va)
}

// ── VirtIO block device driver (V3.1) ──────────────────────────────────────
//
// VirtIO MMIO register offsets (modern MMIO transport)
const VR_REG_QUEUE_SEL: usize = 0x30;
const VR_REG_QUEUE_NUM_MAX: usize = 0x34;
const VR_REG_QUEUE_NUM: usize = 0x38;
const VR_REG_QUEUE_DESC_LOW: usize = 0x80;
const VR_REG_QUEUE_DESC_HIGH: usize = 0x84;
const VR_REG_QUEUE_AVAIL_LOW: usize = 0x90;
const VR_REG_QUEUE_AVAIL_HIGH: usize = 0x94;
const VR_REG_QUEUE_USED_LOW: usize = 0xA0;
const VR_REG_QUEUE_USED_HIGH: usize = 0xA4;
const VR_REG_STATUS: usize = 0x70;
const VR_REG_QUEUE_READY: usize = 0x44;

// Device status bits
const STATUS_ACKNOWLEDGE: u32 = 1;
const STATUS_DRIVER: u32 = 2;
const STATUS_DRIVER_OK: u32 = 4;
const STATUS_FAILED: u32 = 128;

// Block request types
const VIRTIO_BLK_T_IN: u32 = 0;
const VIRTIO_BLK_T_OUT: u32 = 1;

// Physical address of the VirtIO block device on machina
const VIRTIO_BASE: usize = 0x10001000;

fn vr_read(offset: usize) -> u32 {
    unsafe { ((VIRTIO_BASE + offset) as *const u32).read_volatile() }
}

fn vr_write(offset: usize, val: u32) {
    unsafe { ((VIRTIO_BASE + offset) as *mut u32).write_volatile(val) }
}

/// Read a disk sector from the VirtIO block device using the virtqueue mechanism.
///
/// All virtqueue and DMA memory is allocated from the kernel heap (bump allocator),
/// which is identity-mapped (virt addr == phys addr for the DRAM region).
/// After the DMA completes, data is copied to the user-space buffer (SUM=1 enabled).
///
/// Arguments:
///   sector  — Logical block address (LBA, 512-byte units)
///   buf_ptr — User-space buffer virtual address
///   buf_len — Buffer size in bytes (must be >= 512)
///
/// Returns: number of bytes read, or an error string.
pub fn sys_blk_read(sector: usize, buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_len < 512 {
        return Err("buffer too small");
    }

    // 1. Reset device
    vr_write(VR_REG_STATUS, 0);

    // 2. Acknowledge
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE);

    // 3. Driver
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);

    // 4. Feature negotiation (VirtIO 1.0)
    // Read device features (first 32 bits)
    vr_write(0x14, 0); // DeviceFeaturesSel = 0
    let _dev_features = vr_read(0x10); // DeviceFeatures (ignored, we want no features)
                                       // Write 0 as DriverFeatures (no features requested)
    vr_write(0x20, 0); // DriverFeatures = 0
    vr_write(0x24, 0); // DriverFeaturesSel = 0
                       // Set FEATURES_OK (bit 8) and verify
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER | (1 << 8));
    let feat_check = vr_read(VR_REG_STATUS);
    if feat_check & (1 << 8) == 0 {
        return Err("FEATURES_OK not accepted");
    }

    // 5. Select and configure queue 0
    vr_write(VR_REG_QUEUE_SEL, 0);
    let max_size = vr_read(VR_REG_QUEUE_NUM_MAX);
    if max_size == 0 {
        return Err("no virtqueue");
    }
    let queue_size = (max_size as usize).min(16);
    vr_write(VR_REG_QUEUE_NUM, queue_size as u32);

    // 5. Allocate virtqueue memory (contiguous, identity-mapped)
    let desc_size = queue_size * 16;
    let avail_size = 6 + 2 * queue_size;
    let used_size = 6 + 8 * queue_size;
    let total_size = desc_size + ((avail_size + 1) & !1) + ((used_size + 3) & !3);

    let vq_mem = unsafe {
        alloc::alloc::alloc_zeroed(core::alloc::Layout::from_size_align(total_size, 4096).unwrap())
    };
    if vq_mem.is_null() {
        return Err("OOM (vq)");
    }

    let desc_table = vq_mem as usize;
    let avail_ring = (desc_table + desc_size + 1) & !1; // 2-byte align
    let used_ring = (avail_ring + avail_size + 3) & !3; // 4-byte align

    // 6. Allocate request header (16 bytes: type + reserved + sector)
    let req_buf =
        unsafe { alloc::alloc::alloc(core::alloc::Layout::from_size_align(16, 8).unwrap()) };
    if req_buf.is_null() {
        return Err("OOM (req)");
    }

    unsafe {
        (req_buf as *mut u32).write_volatile(VIRTIO_BLK_T_IN); // type
        (req_buf as *mut u32).add(1).write_volatile(0); // reserved
        (req_buf as *mut u64).add(1).write_volatile(sector as u64); // sector
    }

    // 7. Allocate data buffer (512 bytes, identity-mapped for DMA)
    let data_buf =
        unsafe { alloc::alloc::alloc(core::alloc::Layout::from_size_align(512, 64).unwrap()) };
    if data_buf.is_null() {
        return Err("OOM (data)");
    }

    // 8. Allocate status byte
    let status_buf =
        unsafe { alloc::alloc::alloc(core::alloc::Layout::from_size_align(1, 1).unwrap()) };
    if status_buf.is_null() {
        return Err("OOM (status)");
    }

    // 9. Set up descriptor table
    // Descriptor 0: request header (OUT, driver->device, flags=NEXT)
    unsafe {
        let d0 = desc_table as *mut u32;
        d0.add(0).write_volatile(req_buf as u32); // addr low (identity-mapped)
        d0.add(1).write_volatile(0); // addr high
        d0.add(2).write_volatile(16); // len
        d0.add(3).write_volatile(1 | (1 << 16)); // flags=NEXT, next=1
    }

    // Descriptor 1: data buffer (IN, device->driver, flags=NEXT|WRITE)
    unsafe {
        let d1 = (desc_table + 16) as *mut u32;
        d1.add(0).write_volatile(data_buf as u32); // addr low (identity-mapped)
        d1.add(1).write_volatile(0); // addr high
        d1.add(2).write_volatile(512); // len
        d1.add(3).write_volatile(3 | (2 << 16)); // flags=NEXT|WRITE, next=2
    }

    // Descriptor 2: status byte (IN, device->driver, flags=WRITE)
    unsafe {
        let d2 = (desc_table + 32) as *mut u32;
        d2.add(0).write_volatile(status_buf as u32); // addr low
        d2.add(1).write_volatile(0); // addr high
        d2.add(2).write_volatile(1); // len
        d2.add(3).write_volatile(2); // flags: WRITE
    }

    // 10. Set up available ring
    // Layout: [flags(u16), idx(u16), ring[0](u16), ring[1](u16), ...]
    unsafe {
        // flags = 0 (no interrupt)
        (avail_ring as *mut u16).write_volatile(0);
        // idx = 0
        (avail_ring as *mut u16).add(1).write_volatile(0);
        // ring[0] = descriptor chain start (descriptor index 0)
        (avail_ring as *mut u16).add(2).write_volatile(0);
        // Memory barrier: make descriptor writes visible to device
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        // Update idx to 1 (one descriptor chain available)
        (avail_ring as *mut u16).add(1).write_volatile(1);
    }

    // 11. Write virtqueue physical addresses to device
    vr_write(VR_REG_QUEUE_DESC_LOW, desc_table as u32);
    vr_write(VR_REG_QUEUE_DESC_HIGH, 0);
    vr_write(VR_REG_QUEUE_AVAIL_LOW, avail_ring as u32);
    vr_write(VR_REG_QUEUE_AVAIL_HIGH, 0);
    vr_write(VR_REG_QUEUE_USED_LOW, used_ring as u32);
    vr_write(VR_REG_QUEUE_USED_HIGH, 0);

    // 12. Set queue ready
    vr_write(VR_REG_QUEUE_READY, 1);

    // 13. Set DRIVER_OK (preserve ACKNOWLEDGE | DRIVER | FEATURES_OK)
    vr_write(
        VR_REG_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER | (1 << 8) | STATUS_DRIVER_OK,
    );

    // 14. Kick the device (write queue index 0 to QueueNotify at offset 0x50)
    // NOTE: Must be u32 write; machina's VirtIO MMIO handler ignores writes
    // with size != 4 for transport registers.
    unsafe {
        ((VIRTIO_BASE + 0x50) as *mut u32).write_volatile(0);
    }

    // 15. Poll for completion (used ring idx > 0)
    let used_idx_ptr = (used_ring + 2) as *mut u16;
    let mut poll_count: u32 = 0;
    loop {
        if unsafe { used_idx_ptr.read_volatile() > 0 } {
            break;
        }
        poll_count += 1;
        if poll_count > 10_000_000 {
            // Debug: print device state on timeout
            crate::console::puts("  BLK: timeout, status=");
            let st = vr_read(VR_REG_STATUS);
            let isr_raw = unsafe { ((VIRTIO_BASE + 0x60) as *const u32).read_volatile() };
            crate::console::puts(" st=");
            hex_dbg(st as usize);
            crate::console::puts(" isr=");
            hex_dbg(isr_raw as usize);
            crate::console::puts("\r\n");
            return Err("device timeout");
        }
        core::hint::spin_loop();
    }

    // 16. Read used ring element: ring[0].len (total bytes written by device)
    let used_elem_len = unsafe { ((used_ring + 8) as *const u32).read_volatile() };
    let used_elem_id = unsafe { ((used_ring + 4) as *const u32).read_volatile() };

    // 17. Check VirtIO block status byte (0 = OK)
    let blk_status = unsafe { status_buf.read() };
    crate::console::puts("  BLK: status_byte=");
    hex_dbg(blk_status as usize);
    crate::console::puts("\r\n");
    if blk_status != 0 {
        return Err("virtio block error");
    }

    // 18. Copy data to user buffer (SUM=1 enables kernel access to user pages)
    let copy_len = core::cmp::min(used_elem_len as usize, 512);
    let copy_len = core::cmp::min(copy_len, buf_len);
    unsafe {
        core::ptr::copy_nonoverlapping(data_buf, buf_ptr as *mut u8, copy_len);
    }

    Ok(copy_len)
}

/// Write a disk sector to the VirtIO block device using the virtqueue mechanism.
///
/// Similar to sys_blk_read but with VIRTIO_BLK_T_OUT (write to device).
/// Copies user data to a kernel DMA buffer first, then submits to virtqueue.
///
/// Arguments:
///   sector  — Logical block address (LBA, 512-byte units)
///   buf_ptr — User-space buffer virtual address (data to write)
///   buf_len — Buffer size in bytes (must be >= 512)
///
/// Returns: 512 on success, or an error string.
pub fn sys_blk_write(sector: usize, buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_len < 512 {
        return Err("buffer too small");
    }

    // 1. Reset device
    vr_write(VR_REG_STATUS, 0);

    // 2. Acknowledge
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE);

    // 3. Driver
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER);

    // 4. Feature negotiation (VirtIO 1.0)
    vr_write(0x14, 0); // DeviceFeaturesSel = 0
    let _dev_features = vr_read(0x10); // DeviceFeatures (ignored)
    vr_write(0x20, 0); // DriverFeatures = 0
    vr_write(0x24, 0); // DriverFeaturesSel = 0
    vr_write(VR_REG_STATUS, STATUS_ACKNOWLEDGE | STATUS_DRIVER | (1 << 8));
    let feat_check = vr_read(VR_REG_STATUS);
    if feat_check & (1 << 8) == 0 {
        return Err("FEATURES_OK not accepted");
    }

    // 5. Select and configure queue 0
    vr_write(VR_REG_QUEUE_SEL, 0);
    let max_size = vr_read(VR_REG_QUEUE_NUM_MAX);
    if max_size == 0 {
        return Err("no virtqueue");
    }
    let queue_size = (max_size as usize).min(16);
    vr_write(VR_REG_QUEUE_NUM, queue_size as u32);

    // 5b. Allocate virtqueue memory (contiguous, identity-mapped)
    let desc_size = queue_size * 16;
    let avail_size = 6 + 2 * queue_size;
    let used_size = 6 + 8 * queue_size;
    let total_size = desc_size + ((avail_size + 1) & !1) + ((used_size + 3) & !3);

    let vq_mem = unsafe {
        alloc::alloc::alloc_zeroed(core::alloc::Layout::from_size_align(total_size, 4096).unwrap())
    };
    if vq_mem.is_null() {
        return Err("OOM (vq)");
    }

    let desc_table = vq_mem as usize;
    let avail_ring = (desc_table + desc_size + 1) & !1; // 2-byte align
    let used_ring = (avail_ring + avail_size + 3) & !3; // 4-byte align

    // 6. Allocate request header (16 bytes: type + reserved + sector)
    let req_buf =
        unsafe { alloc::alloc::alloc(core::alloc::Layout::from_size_align(16, 8).unwrap()) };
    if req_buf.is_null() {
        return Err("OOM (req)");
    }

    unsafe {
        (req_buf as *mut u32).write_volatile(VIRTIO_BLK_T_OUT); // type = 1 for OUT
        (req_buf as *mut u32).add(1).write_volatile(0); // reserved
        (req_buf as *mut u64).add(1).write_volatile(sector as u64); // sector
    }

    // 7. Allocate data buffer (512 bytes, identity-mapped for DMA)
    let data_buf =
        unsafe { alloc::alloc::alloc(core::alloc::Layout::from_size_align(512, 64).unwrap()) };
    if data_buf.is_null() {
        return Err("OOM (data)");
    }

    // Copy user data to DMA buffer (SUM=1 enables kernel access to user pages)
    unsafe {
        core::ptr::copy_nonoverlapping(buf_ptr as *const u8, data_buf, 512);
    }

    // 8. Allocate status byte
    let status_buf =
        unsafe { alloc::alloc::alloc(core::alloc::Layout::from_size_align(1, 1).unwrap()) };
    if status_buf.is_null() {
        return Err("OOM (status)");
    }

    // 9. Set up descriptor table
    // Descriptor 0: request header (OUT, driver->device, flags=NEXT)
    unsafe {
        let d0 = desc_table as *mut u32;
        d0.add(0).write_volatile(req_buf as u32); // addr low (identity-mapped)
        d0.add(1).write_volatile(0); // addr high
        d0.add(2).write_volatile(16); // len
        d0.add(3).write_volatile(1 | (1 << 16)); // flags=NEXT, next=1
    }

    // Descriptor 1: data buffer (OUT, driver->device, flags=NEXT, no WRITE)
    unsafe {
        let d1 = (desc_table + 16) as *mut u32;
        d1.add(0).write_volatile(data_buf as u32); // addr low (identity-mapped)
        d1.add(1).write_volatile(0); // addr high
        d1.add(2).write_volatile(512); // len
        d1.add(3).write_volatile(1 | (2 << 16)); // flags=NEXT (no WRITE), next=2
    }

    // Descriptor 2: status byte (IN, device->driver, flags=WRITE)
    unsafe {
        let d2 = (desc_table + 32) as *mut u32;
        d2.add(0).write_volatile(status_buf as u32); // addr low
        d2.add(1).write_volatile(0); // addr high
        d2.add(2).write_volatile(1); // len
        d2.add(3).write_volatile(2); // flags: WRITE
    }

    // 10. Set up available ring
    unsafe {
        (avail_ring as *mut u16).write_volatile(0);
        (avail_ring as *mut u16).add(1).write_volatile(0);
        (avail_ring as *mut u16).add(2).write_volatile(0);
        core::sync::atomic::fence(core::sync::atomic::Ordering::SeqCst);
        (avail_ring as *mut u16).add(1).write_volatile(1);
    }

    // 11. Write virtqueue physical addresses to device
    vr_write(VR_REG_QUEUE_DESC_LOW, desc_table as u32);
    vr_write(VR_REG_QUEUE_DESC_HIGH, 0);
    vr_write(VR_REG_QUEUE_AVAIL_LOW, avail_ring as u32);
    vr_write(VR_REG_QUEUE_AVAIL_HIGH, 0);
    vr_write(VR_REG_QUEUE_USED_LOW, used_ring as u32);
    vr_write(VR_REG_QUEUE_USED_HIGH, 0);

    // 12. Set queue ready
    vr_write(VR_REG_QUEUE_READY, 1);

    // 13. Set DRIVER_OK
    vr_write(
        VR_REG_STATUS,
        STATUS_ACKNOWLEDGE | STATUS_DRIVER | (1 << 8) | STATUS_DRIVER_OK,
    );

    // 14. Kick the device (u32 write required for machina VirtIO MMIO)
    unsafe {
        ((VIRTIO_BASE + 0x50) as *mut u32).write_volatile(0);
    }

    // 15. Poll for completion
    let used_idx_ptr = (used_ring + 2) as *mut u16;
    let mut poll_count: u32 = 0;
    loop {
        if unsafe { used_idx_ptr.read_volatile() > 0 } {
            break;
        }
        poll_count += 1;
        if poll_count > 10_000_000 {
            crate::console::puts("  BLK_WR: timeout, status=");
            let st = vr_read(VR_REG_STATUS);
            let isr_raw = unsafe { ((VIRTIO_BASE + 0x60) as *const u32).read_volatile() };
            crate::console::puts(" st=");
            hex_dbg(st as usize);
            crate::console::puts(" isr=");
            hex_dbg(isr_raw as usize);
            crate::console::puts("\r\n");
            return Err("device timeout");
        }
        core::hint::spin_loop();
    }

    // 16. Read used ring element
    let _used_elem_len = unsafe { ((used_ring + 8) as *const u32).read_volatile() };
    let _used_elem_id = unsafe { ((used_ring + 4) as *const u32).read_volatile() };

    // 17. Check VirtIO block status byte (0 = OK)
    let blk_status = unsafe { status_buf.read() };
    if blk_status != 0 {
        return Err("virtio block error");
    }

    Ok(512) // 512 bytes written
}

pub fn sys_getuid() -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    let procs = crate::proc::PROCESSES.lock();
    let proc = procs.iter().find(|p| p.pid == pid).ok_or("not found")?;
    Ok(proc.uid as usize)
}

pub fn sys_setuid(uid: u32) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    let mut procs = crate::proc::PROCESSES.lock();
    let proc = procs.iter_mut().find(|p| p.pid == pid).ok_or("not found")?;
    // Only root can change UID
    if proc.uid != 0 { return Err("permission denied"); }
    proc.uid = uid;
    Ok(0)
}

pub fn sys_chmod(_path: usize, _mode: u16) -> Result<usize, &'static str> {
    // Simplified: always succeed for root
    Ok(0)
}

// ── Signal handling (V12.0D) ────────────────────────────────────────────────

pub const SIGCHLD: u32 = 17;
pub const SIGTERM: u32 = 15;
pub const SIGKILL: u32 = 9;
pub const SIG_IGN: usize = 1;
pub const SIG_DFL: usize = 0;

/// Register a signal handler for the calling process.
/// For basic implementation, just acknowledge the request.
/// Returns 0 on success.
pub fn sys_signal(sig: u32, handler: usize) -> Result<usize, &'static str> {
    let _ = (sig, handler);
    Ok(0)
}

/// Wait for a child process to exit.
/// pid == -1: wait for any child
/// pid > 0:   wait for a specific child
/// Returns (child_pid) on success, 0 if no dead children yet.
pub fn sys_waitpid(pid: i32, _status_ptr: usize, _options: usize) -> Result<usize, &'static str> {
    let caller_pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no proc")?;

    let procs = crate::proc::PROCESSES.lock();

    if pid == -1 {
        // Wait for any child
        for proc in procs.iter() {
            if proc.parent == Some(caller_pid) && proc.state == ProcessState::Dead {
                let child_pid = proc.pid;
                drop(procs);
                return Ok(child_pid as usize);
            }
        }
    } else if pid > 0 {
        // Wait for specific child
        let target = pid as u32;
        if let Some(proc) = procs.iter().find(|p| p.pid == target && p.parent == Some(caller_pid)) {
            if proc.state == ProcessState::Dead {
                let child_pid = proc.pid;
                drop(procs);
                return Ok(child_pid as usize);
            }
        }
    }

    // No dead children — return 0 (no status change yet)
    Ok(0)
}

fn hex_dbg(val: usize) {
    for i in (0..16).rev() {
        let nibble = (val >> (i * 4)) & 0xF;
        let c = if nibble < 10 {
            b'0' + nibble as u8
        } else {
            b'a' + (nibble - 10) as u8
        };
        unsafe {
            core::arch::asm!("ecall", in("a7") 1usize, in("a0") c as usize);
        }
    }
}
