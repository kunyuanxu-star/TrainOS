use crate::mem::{buddy, sv39};
use crate::proc::process::ProcessState;

pub fn sys_spawn(elf_ptr: usize, elf_len: usize) -> Result<usize, &'static str> {
    if elf_ptr == 0 || elf_len == 0 || elf_len > 0x10_0000 {
        return Err("invalid elf args");
    }
    // Read ELF data from user space (SUM=1 allows S-mode to access U pages)
    let elf_data = unsafe { core::slice::from_raw_parts(elf_ptr as *const u8, elf_len) };
    // Spawn the process from the ELF data (default priority 32)
    let pid = crate::proc::spawn(elf_data, 32).ok_or("spawn failed")?;
    Ok(pid as usize)
}
pub fn sys_exec(path_ptr: usize) -> Result<usize, &'static str> {
    // Read path from user space
    if path_ptr == 0 { return Err("null path"); }
    let mut path_buf = [0u8; 32];
    let plen = unsafe {
        let mut len = 0;
        let src = path_ptr as *const u8;
        while len < 31 {
            let c = src.add(len).read_volatile();
            if c == 0 { break; }
            path_buf[len] = c;
            len += 1;
        }
        if len == 0 { return Err("empty path"); }
        len
    };

    // Read ELF from VFS
    let sender_pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    let reply_ep = crate::ipc::create_endpoint();
    use crate::ipc::message::Message;
    let mut msg = Message::new(sender_pid, 2); // READ
    msg.payload[0] = reply_ep as u8;
    msg.payload[1] = (reply_ep >> 8) as u8;
    msg.payload[2] = plen as u8;
    for i in 0..plen { msg.payload[3 + i] = path_buf[i]; }
    msg.payload_len = 3 + plen;
    crate::ipc::endpoint::send(2, sender_pid, msg).ok().ok_or("vfs send failed")?;

    // Receive ELF data
    let mut elf_buf = [0u8; 512];
    let elf_len = loop {
        match crate::ipc::endpoint::recv(reply_ep, sender_pid) {
            Ok(resp) => {
                let len = core::cmp::min(resp.payload_len, 512);
                for i in 0..len { elf_buf[i] = resp.payload[i]; }
                break len;
            }
            Err(_) => { crate::sched::schedule(); }
        }
    };

    if elf_len < 52 { return Err("ELF too small"); }

    // Get current process page table root
    let (old_pt, pid) = {
        let procs = crate::proc::PROCESSES.lock();
        let proc = procs.iter().find(|p| {
            let cur = crate::sched::current_thread()
                .map(|t| unsafe { (*t).owner }).unwrap_or(0);
            p.pid == cur
        }).ok_or("no proc")?;
        (proc.page_table_root, proc.pid)
    };

    // Allocate new page table and load
    let new_pt = crate::mem::buddy::alloc_page().ok_or("OOM")?;
    unsafe {
        core::ptr::write_bytes(crate::mem::sv39::pa_to_kva(new_pt) as *mut u8, 0, 4096);
        crate::mem::sv39::copy_kernel_mappings(new_pt);
    }
    let (entry, user_sp) = crate::proc::elf::load_elf(&elf_buf[..elf_len], new_pt)
        .ok_or("elf load failed")?;
    let new_satp = crate::mem::sv39::make_satp(new_pt);

    // Update process and thread
    {
        let mut procs = crate::proc::PROCESSES.lock();
        if let Some(proc) = procs.iter_mut().find(|p| p.pid == pid) {
            proc.page_table_root = new_pt;
        }
    }
    let current = crate::sched::current_thread().ok_or("no thread")?;
    unsafe {
        (*current).task_ctx.satp = new_satp;
        if let Some(ref mut tf) = (*current).trap_frame {
            tf.sepc = entry;
            tf.user_sp = user_sp;
            tf.a0 = 0;
        }
    }

    crate::sched::schedule();
    Ok(0)
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

// ── Extended process syscalls (V14.0) ────────────────────────────────────────

/// sys_getppid() — get parent PID.
pub fn sys_getppid() -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no proc")?;
    let procs = crate::proc::PROCESSES.lock();
    let parent = procs.iter()
        .find(|p| p.pid == pid)
        .and_then(|p| p.parent)
        .unwrap_or(0);
    Ok(parent as usize)
}

/// sys_gettid() — get thread ID (same as PID in single-threaded V14).
pub fn sys_gettid() -> Result<usize, &'static str> {
    let tid = crate::sched::current_thread()
        .map(|t| t as usize)
        .unwrap_or(0);
    Ok(tid)
}

/// sys_umask(mask) — set file mode creation mask. Returns previous mask.
pub fn sys_umask(mask: u16) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no proc")?;
    let mut procs = crate::proc::PROCESSES.lock();
    let proc = procs.iter_mut().find(|p| p.pid == pid).ok_or("not found")?;
    let old = proc.umask;
    proc.umask = mask;
    Ok(old as usize)
}

/// sys_setsid() — create a new session. Returns session ID (same as PID).
pub fn sys_setsid() -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no proc")?;
    Ok(pid as usize)
}


/// sys_sysinfo(buf_ptr) — fill sysinfo structure.
/// struct sysinfo {
///     uptime: i64,       // seconds since boot
///     loads: [u64; 3],   // 1, 5, 15 min load averages
///     totalram: u64,     // total usable RAM in bytes
///     freeram: u64,      // available RAM in bytes
///     procs: u16,        // number of processes
/// }
pub fn sys_sysinfo(buf_ptr: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 { return Err("null pointer"); }

    let ticks = unsafe { crate::trap::TICK_COUNT };
    let uptime = (ticks as u64 * 10) / 1000; // seconds

    let total_pages = crate::mem::buddy::total_pages() as u64;
    let free_pages = (total_pages - crate::mem::buddy::allocated_pages() as u64);
    let total_ram = total_pages * 4096;
    let free_ram = free_pages * 4096;

    let procs = crate::proc::PROCESSES.lock();
    let proc_count = procs.iter().filter(|p| p.state != ProcessState::Dead).count() as u16;
    drop(procs);

    unsafe {
        let ptr = buf_ptr as *mut u64;

        ptr.write_volatile(uptime);           // uptime (seconds)

        ptr.add(1).write_volatile(0);   // load1
        ptr.add(2).write_volatile(0);   // load5
        ptr.add(3).write_volatile(0);   // load15

        ptr.add(4).write_volatile(total_ram);  // totalram
        ptr.add(5).write_volatile(free_ram);   // freeram

        let proc_ptr = buf_ptr as *mut u16;
        proc_ptr.add(48).write_volatile(proc_count); // procs (offset 48 bytes in)
    }

    Ok(0)
}

// ── Namespace syscalls (V15.0) ───────────────────────────────────────────────

/// sys_unshare(flags) — disassociate parts of process execution context.
pub fn sys_unshare(flags: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no proc")?;

    if flags & crate::ns::CLONE_NEWUTS != 0 {
        let ns_id = crate::ns::new_uts_ns().ok_or("no uts ns")?;
        crate::ns::set_process_uts(pid, ns_id);
    }
    if flags & crate::ns::CLONE_NEWPID != 0 {
        let ns_id = crate::ns::new_pid_ns(pid).ok_or("no pid ns")?;
        crate::ns::set_process_pid_ns(pid, ns_id);
    }
    if flags & crate::ns::CLONE_NEWNS != 0 { crate::ns::new_mount_ns(); }
    if flags & crate::ns::CLONE_NEWNET != 0 { crate::ns::new_net_ns(); }
    if flags & crate::ns::CLONE_NEWIPC != 0 { crate::ns::new_ipc_ns(); }
    if flags & crate::ns::CLONE_NEWUSER != 0 { crate::ns::new_user_ns(); }

    Ok(0)
}

/// sys_sethostname(name_ptr, len) — set system hostname
pub fn sys_sethostname(name_ptr: usize, len: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no proc")?;
    let ns_id = crate::ns::get_process_uts(pid);

    if name_ptr == 0 || len == 0 { return Err("invalid args"); }
    let mut name = [0u8; 64];
    let copy_len = core::cmp::min(len, 64);
    unsafe {
        let src = core::slice::from_raw_parts(name_ptr as *const u8, copy_len);
        name[..copy_len].copy_from_slice(src);
    }
    if crate::ns::set_hostname(ns_id, &name[..copy_len]) { Ok(0) } else { Err("sethostname failed") }
}

/// sys_gethostname(buf_ptr, len) — get system hostname
pub fn sys_gethostname(buf_ptr: usize, len: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no proc")?;
    let ns_id = crate::ns::get_process_uts(pid);
    let mut name = [0u8; 64];
    let nlen = crate::ns::get_hostname(ns_id, &mut name);
    let copy_len = core::cmp::min(nlen, len);
    if buf_ptr != 0 && copy_len > 0 {
        unsafe {
            let dst = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, copy_len);
            dst.copy_from_slice(&name[..copy_len]);
        }
    }
    Ok(copy_len)
}

/// sys_setns(fd, nstype) — reassociate with a namespace
pub fn sys_setns(fd: usize, nstype: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no proc")?;
    let target_ns = if nstype == crate::ns::CLONE_NEWPID {
        crate::ns::get_process_pid_ns(fd as u32)
    } else if nstype == crate::ns::CLONE_NEWUTS {
        crate::ns::get_process_uts(fd as u32)
    } else { return Err("unsupported ns type"); };

    if nstype == crate::ns::CLONE_NEWPID {
        crate::ns::set_process_pid_ns(pid, target_ns);
    } else {
        crate::ns::set_process_uts(pid, target_ns);
    }
    Ok(0)
}

// ── CPU affinity (V15.0) ─────────────────────────────────────────────────────

static mut CPU_AFFINITY: [(u32, u64); 64] = [(0, 0xFFFF_FFFF_FFFF_FFFFu64); 64];
static mut AFFINITY_COUNT: usize = 0;

pub fn sys_sched_setaffinity(pid: usize, _size: usize, mask_ptr: usize) -> Result<usize, &'static str> {
    if mask_ptr == 0 { return Err("null mask"); }
    let mask: u64 = unsafe { (mask_ptr as *const u64).read_volatile() };
    unsafe {
        for i in 0..AFFINITY_COUNT {
            if CPU_AFFINITY[i].0 == pid as u32 { CPU_AFFINITY[i].1 = mask; return Ok(0); }
        }
        if AFFINITY_COUNT >= 64 { return Err("affinity table full"); }
        CPU_AFFINITY[AFFINITY_COUNT] = (pid as u32, mask);
        AFFINITY_COUNT += 1;
    }
    Ok(0)
}

pub fn sys_sched_getaffinity(pid: usize, _size: usize, mask_ptr: usize) -> Result<usize, &'static str> {
    if mask_ptr == 0 { return Err("null mask ptr"); }
    unsafe {
        for i in 0..AFFINITY_COUNT {
            if CPU_AFFINITY[i].0 == pid as u32 {
                (mask_ptr as *mut u64).write_volatile(CPU_AFFINITY[i].1);
                return Ok(0);
            }
        }
        (mask_ptr as *mut u64).write_volatile(0xFFFF_FFFF_FFFF_FFFFu64);
    }
    Ok(0)
}

// ── Resource usage (V15.0) ───────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy)]
struct ProcTime { pid: u32, utime: u64, stime: u64, start_time: u64 }
const EMPTY_PROCTIME: ProcTime = ProcTime { pid: 0, utime: 0, stime: 0, start_time: 0 };
static mut PROC_TIMES: [ProcTime; 32] = [EMPTY_PROCTIME; 32];
static mut PROC_TIME_COUNT: usize = 0;

pub fn init_proc_time(pid: u32) {
    unsafe {
        for i in 0..PROC_TIME_COUNT { if PROC_TIMES[i].pid == pid { return; } }
        if PROC_TIME_COUNT >= 32 { return; }
        let tick = crate::trap::TICK_COUNT;
        PROC_TIMES[PROC_TIME_COUNT] = ProcTime { pid, utime: 0, stime: 0, start_time: tick as u64 };
        PROC_TIME_COUNT += 1;
    }
}

pub fn account_utime() {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).unwrap_or(0);
    unsafe {
        for i in 0..PROC_TIME_COUNT {
            if PROC_TIMES[i].pid == pid { PROC_TIMES[i].utime += 1; break; }
        }
    }
}

pub fn account_stime() {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).unwrap_or(0);
    unsafe {
        for i in 0..PROC_TIME_COUNT {
            if PROC_TIMES[i].pid == pid { PROC_TIMES[i].stime += 1; break; }
        }
    }
}

pub fn sys_times(buf_ptr: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    unsafe {
        for i in 0..PROC_TIME_COUNT {
            if PROC_TIMES[i].pid == pid {
                if buf_ptr != 0 {
                    let ptr = buf_ptr as *mut u64;
                    let tick = crate::trap::TICK_COUNT;
                    ptr.write_volatile(PROC_TIMES[i].utime);
                    ptr.add(1).write_volatile(PROC_TIMES[i].stime);
                    ptr.add(2).write_volatile(0);
                    ptr.add(3).write_volatile(0);
                }
                return Ok(crate::trap::TICK_COUNT);
            }
        }
    }
    Err("no time entry")
}

pub fn sys_getrusage(who: usize, buf_ptr: usize) -> Result<usize, &'static str> {
    if who > 1 { return Err("bad who"); }
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    if buf_ptr == 0 { return Err("null buf"); }
    unsafe {
        for i in 0..PROC_TIME_COUNT {
            if PROC_TIMES[i].pid == pid {
                let ptr = buf_ptr as *mut u64;
                let utime_ticks = PROC_TIMES[i].utime;
                ptr.write_volatile(utime_ticks / 100);
                ptr.add(1).write_volatile((utime_ticks * 10000000) % 1000000);
                let stime_ticks = PROC_TIMES[i].stime;
                ptr.add(2).write_volatile(stime_ticks / 100);
                ptr.add(3).write_volatile((stime_ticks * 10000000) % 1000000);
                return Ok(0);
            }
        }
    }
    Ok(0)
}

// ── Device driver syscalls (V15.0) ───────────────────────────────────────────

pub fn sys_register_drv(name_ptr: usize, drv_type: usize, probe_ep: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    let mut name = [0u8; 32];
    if name_ptr != 0 {
        unsafe {
            let src = name_ptr as *const u8;
            let mut nlen = 0;
            while nlen < 31 { let c = src.add(nlen).read(); if c == 0 { break; } name[nlen] = c; nlen += 1; }
        }
    }
    crate::device::register(&name, drv_type as u32, pid, probe_ep).ok_or("driver table full").map(|id| id)
}

pub fn sys_unregister_drv(drv_id: usize) -> Result<usize, &'static str> {
    if crate::device::unregister(drv_id) { Ok(0) } else { Err("unregister failed") }
}

pub fn sys_list_drvs(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 || buf_len == 0 { return Err("null buf"); }
    let mut buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };
    Ok(crate::device::list(&mut buf))
}

/// sys_sync() — sync filesystem caches
pub fn sys_sync() -> Result<usize, &'static str> { Ok(0) }

/// sys_reboot(magic, cmd) — reboot/halt/poweroff
pub fn sys_reboot(magic: usize, _cmd: usize) -> Result<usize, &'static str> {
    if magic != 0xfee1_dead { return Err("bad magic"); }
    unsafe { core::arch::asm!("ecall", in("a7") 8usize, in("a0") 0usize, in("a1") 0usize); }
    Err("reset failed")
}

// ── V21 Security syscalls ────────────────────────────────────────────────────

/// sys_seccomp_add(syscall_nr, action) — add a seccomp rule for the calling process.
/// action: 0=allow, 1=kill, 2=log
pub fn sys_seccomp_add(syscall_nr: u32, action: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no proc")?;
    crate::security::seccomp_add_rule(pid, syscall_nr as usize, action as u8)?;
    Ok(0)
}

/// sys_cap_audit(buf_ptr, buf_len) — read capability audit log.
pub fn sys_cap_audit(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 || buf_len == 0 { return Err("null buf"); }
    let mut buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };
    Ok(crate::security::cap_audit_read(&mut buf))
}


// ── V22 io_uring syscalls ────────────────────────────────────────────────────
pub fn sys_io_uring_setup(entries: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    crate::iouring::setup(pid, entries).ok_or("setup failed")
}
pub fn sys_io_uring_enter(ring_id: usize, _to_submit: usize, _min_complete: usize) -> Result<usize, &'static str> {
    Ok(crate::iouring::submit(ring_id))
}
pub fn sys_io_uring_register(_ring_id: usize, _opcode: usize, _arg: usize) -> Result<usize, &'static str> { Ok(0) }

// ── V23 Virtualization syscalls ──────────────────────────────────────────────
pub fn sys_vm_create(memory_mb: usize) -> Result<usize, &'static str> {
    crate::hypervisor::vm_create(memory_mb).map(|id| id as usize).ok_or("vm_create failed")
}
pub fn sys_vm_destroy(vm_id: u32) -> Result<usize, &'static str> {
    if crate::hypervisor::vm_destroy(vm_id) { Ok(0) } else { Err("not found") }
}
pub fn sys_vm_start(vm_id: u32) -> Result<usize, &'static str> {
    if crate::hypervisor::vm_start(vm_id) { Ok(0) } else { Err("start failed") }
}
pub fn sys_vm_list(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 { return Err("null buf"); }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };
    Ok(crate::hypervisor::vm_list(buf))
}

// ── V24 Kernel extension syscalls ────────────────────────────────────────────
pub fn sys_ext_register(hook_type: usize, bytecode_ptr: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    let bc = unsafe { core::slice::from_raw_parts(bytecode_ptr as *const u8, 256) };
    crate::extension::register(pid, hook_type as u8, bc).map(|id| id).ok_or("register failed")
}
pub fn sys_ext_unregister(ext_id: usize) -> Result<usize, &'static str> {
    if crate::extension::unregister(ext_id) { Ok(0) } else { Err("not found") }
}
pub fn sys_ext_list(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 { return Err("null buf"); }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };
    Ok(crate::extension::list(buf))
}

// ── V25 NUMA syscalls ────────────────────────────────────────────────────────
pub fn sys_numa_nodes(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    crate::numa::discover();
    if buf_ptr == 0 { return Ok(crate::numa::node_count()); }
    let cnt = crate::numa::node_count();
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len.min(cnt * 8)) };
    for i in 0..cnt {
        let mask = crate::numa::node_cpu_mask(i as u8);
        let off = i * 8;
        buf[off] = i as u8; buf[off+1] = mask as u8; buf[off+2] = (mask>>8) as u8;
    }
    Ok(cnt)
}
pub fn sys_numa_alloc(node: u8) -> Result<usize, &'static str> {
    crate::numa::node_alloc_page(node).ok_or("OOM")
}

// ── V26 Distributed IPC syscalls ─────────────────────────────────────────────
pub fn sys_remote_node_add(ip_ptr: usize, port: usize) -> Result<usize, &'static str> {
    let ip = unsafe { core::slice::from_raw_parts(ip_ptr as *const u8, 16) };
    crate::distributed::add_remote_node(ip, port as u16).map(|id| id as usize).ok_or("full")
}
pub fn sys_remote_ep_publish(local_ep: usize, remote_node: usize, remote_ep: usize) -> Result<usize, &'static str> {
    if crate::distributed::publish_endpoint(local_ep, remote_node as u32, remote_ep) { Ok(0) } else { Err("full") }
}
pub fn sys_remote_ep_lookup(local_ep: usize, buf_ptr: usize) -> Result<usize, &'static str> {
    let result = crate::distributed::lookup_remote_ep(local_ep);
    if let Some((node, ep)) = result {
        if buf_ptr != 0 {
            let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, 8) };
            buf[0] = node as u8; buf[1] = (node>>8) as u8;
            buf[4] = ep as u8; buf[5] = (ep>>8) as u8;
        }
        Ok(0)
    } else { Err("not found") }
}
pub fn sys_remote_send(node_id: u32, ep: usize, data_ptr: usize, data_len: usize) -> Result<usize, &'static str> {
    let data = unsafe { core::slice::from_raw_parts(data_ptr as *const u8, data_len) };
    crate::distributed::remote_send(node_id, ep, data)?;
    Ok(0)
}
