use crate::cap::ops;
use crate::cap::types;
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
        // V21: Cap leak detection — count remaining caps before destruction
        {
            let cnode_id = proc.cnode_id;
            let leak_count = ops::get_resource(cnode_id).map_or(0, |res| {
                if let types::ResourceData::CNode { ref slots } = &res.data {
                    let s = slots.lock();
                    s.iter().filter(|slot| slot.cap_type != types::CapType::Null).count()
                } else {
                    0
                }
            });
            if leak_count > 0 {
                crate::security::cap_audit_log(pid, 5, leak_count);
                crate::println!("CAP: pid={} leaked {} capabilities on exit", pid, leak_count);
            }
        }

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
///   flags   — RWF_* flags (RWF_ATOMIC enables all-or-nothing write)
///
/// Returns: 512 on success, or an error string.
pub fn sys_blk_write(sector: usize, buf_ptr: usize, buf_len: usize, flags: usize) -> Result<usize, &'static str> {
    if buf_len < 512 {
        return Err("buffer too small");
    }

    // V35: RWF_ATOMIC — save original data before write for rollback
    if flags & crate::syscall::ioflags::RWF_ATOMIC as usize != 0 {
        crate::syscall::ioflags::atomic_write_begin(sector as u64, 512)?;
        // Read current sector data into backup
        match crate::syscall::ioflags::atomic_read_sector(sector) {
            Ok(sector_data) => {
                crate::syscall::ioflags::atomic_write_save_original(&sector_data);
            }
            Err(e) => return Err(e),
        }
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

    // V35: Atomic write commit/rollback
    if flags & crate::syscall::ioflags::RWF_ATOMIC as usize != 0 {
        if blk_status == 0 {
            // Write succeeded — commit
            crate::syscall::ioflags::atomic_write_commit();
        } else {
            // Write failed — rollback
            let _ = crate::syscall::ioflags::atomic_write_rollback();
            return Err("virtio block error (rolled back)");
        }
    }

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

/// sys_syscall_stats(buf_ptr, buf_len) — read syscall statistics counters.
pub fn sys_syscall_stats(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 { return Err("null buf"); }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };
    Ok(crate::syscall::syscall_stats_read(buf))
}


// ── V22 io_uring syscalls ────────────────────────────────────────────────────
pub fn sys_io_uring_setup(entries: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    let ring_id = crate::iouring::setup(pid, entries).ok_or("setup failed")?;
    // Pack: low 16 bits = ring_id, bits 16+ = sq_va
    let sq_va = crate::iouring::get_sq_va(ring_id);
    Ok(ring_id | (sq_va << 16))
}
pub fn sys_io_uring_enter(ring_id: usize, _to_submit: usize, _min_complete: usize) -> Result<usize, &'static str> {
    Ok(crate::iouring::submit(ring_id))
}
pub fn sys_io_uring_register(_ring_id: usize, _opcode: usize, _arg: usize) -> Result<usize, &'static str> { Ok(0) }

// ── V23 Virtualization syscalls ──────────────────────────────────────────────
pub fn sys_vm_create(memory_mb: usize) -> Result<usize, &'static str> {
    let default_name = b"guest\0";
    crate::hypervisor::vm_create(default_name, memory_mb).map(|id| id as usize).ok_or("vm_create failed")
}
pub fn sys_vm_destroy(vm_id: u32) -> Result<usize, &'static str> {
    if crate::hypervisor::vm_destroy(vm_id) { Ok(0) } else { Err("not found") }
}
pub fn sys_vm_start(vm_id: u32, entry_pc: usize) -> Result<usize, &'static str> {
    if crate::hypervisor::vm_start(vm_id, entry_pc) { Ok(0) } else { Err("start failed") }
}
pub fn sys_vm_pause(vm_id: u32) -> Result<usize, &'static str> {
    if crate::hypervisor::vm_pause(vm_id) { Ok(0) } else { Err("pause failed") }
}
pub fn sys_vm_resume(vm_id: u32) -> Result<usize, &'static str> {
    if crate::hypervisor::vm_resume(vm_id) { Ok(0) } else { Err("resume failed") }
}
pub fn sys_vm_list(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 { return Err("null buf"); }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };
    Ok(crate::hypervisor::vm_list(buf))
}

// ── V24 Kernel extension syscalls ────────────────────────────────────────────
pub fn sys_ext_register(hook_type: usize, bytecode_ptr: usize, bytecode_len: usize, name_ptr: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    if bytecode_ptr == 0 { return Err("null bytecode"); }
    if bytecode_len < 8 || bytecode_len > crate::extension::MAX_BYTECODE { return Err("invalid bytecode len"); }
    let bc = unsafe { core::slice::from_raw_parts(bytecode_ptr as *const u8, bytecode_len) };
    // Read name from user space (up to 15 bytes + null)
    let mut name_buf = [0u8; 16];
    if name_ptr != 0 {
        for i in 0..15 {
            let c = unsafe { (name_ptr as *const u8).add(i).read_volatile() };
            name_buf[i] = c;
            if c == 0 { break; }
        }
    }
    let name = &name_buf;
    crate::extension::register(pid, hook_type as u8, bc, name).map(|id| id).ok_or("register failed")
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
    let cnt = crate::numa::node_count();
    if buf_ptr == 0 {
        return Ok(cnt);
    }
    let needed = cnt * 25; // 25 bytes per node
    let copy_len = buf_len.min(needed);
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, copy_len) };
    Ok(crate::numa::numa_state_buf(buf))
}
pub fn sys_numa_alloc(node: u8) -> Result<usize, &'static str> {
    crate::numa::node_alloc_page(node).ok_or("OOM")
}

/// Migrate a physical page to a target NUMA node.
/// Arguments: phys_addr, from_node, to_node.
pub fn sys_numa_migrate(phys: usize, from_node: u8, to_node: u8) -> Result<usize, &'static str> {
    crate::numa::migrate_page(phys, from_node, to_node)
}

/// Read EEVDF thread information into a buffer.
/// Format per thread: [pid:4][vruntime:8][weight:4][deadline:8][node:1] = 25 bytes.
pub fn sys_numa_info(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 || buf_len < 25 {
        return Err("invalid buffer");
    }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };
    let mut written = 0;

    // Iterate over all processes and collect EEVDF info.
    let procs = crate::proc::PROCESSES.lock();
    for proc in procs.iter() {
        if written + 25 > buf_len {
            break;
        }
        if let Some(ref thread) = proc.thread {
            let off = written;
            let pid_bytes = proc.pid.to_le_bytes();
            buf[off..off + 4].copy_from_slice(&pid_bytes);
            let vruntime_bytes = thread.vruntime.to_le_bytes();
            buf[off + 4..off + 12].copy_from_slice(&vruntime_bytes);
            let weight_bytes = thread.weight.to_le_bytes();
            buf[off + 12..off + 16].copy_from_slice(&weight_bytes);
            let deadline_bytes = thread.deadline.to_le_bytes();
            buf[off + 16..off + 24].copy_from_slice(&deadline_bytes);
            buf[off + 24] = thread.node_id;
            written += 25;
        }
    }
    drop(procs);
    Ok(written)
}

/// Manually trigger NUMA load balancing.
pub fn sys_numa_balance() -> Result<usize, &'static str> {
    crate::numa::try_balance();
    Ok(0)
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

/// Probe a remote node for liveness.
pub fn sys_remote_probe(node_id: u32) -> Result<usize, &'static str> {
    if crate::distributed::node_probe(node_id) {
        Ok(1)
    } else {
        Ok(0)
    }
}

/// Receive a message from a remote node.
pub fn sys_remote_recv(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    let (src_node, src_ep, payload, payload_len) = crate::distributed::remote_recv()?;
    if buf_ptr != 0 && buf_len >= 4 {
        let copy_len = (payload_len + 4).min(buf_len).min(68);
        unsafe {
            let buf = buf_ptr as *mut u8;
            buf.write(src_node);
            let ep_bytes = src_ep.to_le_bytes();
            buf.add(1).write(ep_bytes[0]);
            buf.add(2).write(ep_bytes[1]);
            buf.add(3).write(payload_len as u8);
            if copy_len > 4 {
                core::ptr::copy_nonoverlapping(payload.as_ptr(), buf.add(4), copy_len - 4);
            }
        }
        Ok(copy_len)
    } else {
        Ok(4)
    }
}

/// Allocate pages from a remote node memory pool.
pub fn sys_remote_mem_alloc(node_id: u8, num_pages: usize) -> Result<usize, &'static str> {
    crate::distributed::remote_alloc_pages(node_id, num_pages).ok_or("OOM")
}

/// Free a remote page by handle.
pub fn sys_remote_mem_free(handle: u64) -> Result<usize, &'static str> {
    if crate::distributed::remote_free_page(handle) { Ok(0) } else { Err("free failed") }
}

/// Request process list from a remote node.
pub fn sys_remote_proclist(node_id: u32, buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 || buf_len < 6 { return Err("invalid buffer"); }
    let mut entries = [crate::distributed::protocol::RemoteProcEntry { pid: 0, state: 0, name: [0u8; 48], name_len: 0 }; 32];
    let count = crate::distributed::remote_proclist(node_id, &mut entries)?;
    let mut written = 0usize;
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };
    for i in 0..count {
        let entry = &entries[i];
        let entry_size = 6 + entry.name_len;
        if written + entry_size > buf_len { break; }
        let pid_bytes = entry.pid.to_le_bytes();
        buf[written] = pid_bytes[0]; buf[written + 1] = pid_bytes[1];
        buf[written + 2] = pid_bytes[2]; buf[written + 3] = pid_bytes[3];
        buf[written + 4] = entry.state;
        buf[written + 5] = entry.name_len as u8;
        buf[written + 6..written + 6 + entry.name_len].copy_from_slice(&entry.name[..entry.name_len]);
        written += entry_size;
    }
    Ok(written)
}

/// Mint a capability and send it to a remote node.
pub fn sys_remote_mint(node_id: u32, local_slot: u32, remote_cnode: u32) -> Result<usize, &'static str> {
    crate::distributed::remote_mint(node_id, local_slot, remote_cnode)?;
    Ok(0)
}

/// Migrate a page from one node to another.
pub fn sys_remote_migrate_page(phys: usize, from_node: u8, to_node: u8) -> Result<usize, &'static str> {
    crate::distributed::migrate_page_local(phys, from_node, to_node)
}

/// Get the local cluster node ID.
pub fn sys_node_id() -> Result<usize, &'static str> {
    Ok(crate::distributed::get_cluster_node_id() as usize)
}


// ── V27 ASLR/Cheri/Sandbox syscalls ─────────────────────────────────────────
pub fn sys_aslr_init() -> Result<usize, &'static str> { crate::aslr::aslr_init(); Ok(0) }

/// Create a CHERI capability for the current process.
pub fn sys_cheri_cap_create(addr: usize, len: usize, perms: u16) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    let cap_id = crate::aslr::cap_create(pid, addr, len, perms)?;
    Ok(cap_id as usize)
}

/// Check if [addr, addr+len) is authorized by any CHERI capability.
pub fn sys_cheri_cap_check(addr: usize, len: usize, perms: u16) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    let valid = crate::aslr::validate_ptr(pid, addr, len, perms);
    Ok(if valid { 1 } else { 0 })
}

/// Delete a CHERI capability by cap_id.
pub fn sys_cheri_cap_delete(pid: u32, cap_id: u8) -> Result<usize, &'static str> {
    crate::aslr::cap_delete(pid, cap_id)?;
    Ok(0)
}

/// Read CHERI capability status into a buffer (for /proc/cheri).
pub fn sys_cheri_status(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 { return Err("null buf"); }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };
    Ok(crate::aslr::cheri_status_format(buf))
}

/// Add a path-based sandbox rule for the current process.
pub fn sys_sandbox_add(path_ptr: usize, mode: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    let mut path_buf = [0u8; 32];
    let plen = {
        let mut len = 0;
        if path_ptr == 0 { return Err("null path"); }
        unsafe {
            let src = path_ptr as *const u8;
            while len < 31 {
                let c = src.add(len).read_volatile();
                if c == 0 { break; }
                path_buf[len] = c;
                len += 1;
            }
        }
        len
    };
    let allow_w = (mode & 1) != 0;
    let allow_r = (mode & 2) != 0;
    crate::aslr::sandbox_add(pid, &path_buf[..plen], allow_r, allow_w)?;
    Ok(0)
}

/// Check if a path is allowed by the sandbox for a given process.
pub fn sys_sandbox_check(pid: usize, path_ptr: usize) -> Result<usize, &'static str> {
    let mut path_buf = [0u8; 32];
    let plen = {
        let mut len = 0;
        if path_ptr == 0 { return Err("null path"); }
        unsafe {
            let src = path_ptr as *const u8;
            while len < 31 {
                let c = src.add(len).read_volatile();
                if c == 0 { break; }
                path_buf[len] = c;
                len += 1;
            }
        }
        len
    };
    Ok(crate::aslr::sandbox_check(pid as u32, &path_buf[..plen], false) as usize)
}

/// Add a network port sandbox rule for a process.
pub fn sys_sandbox_net_add(pid: u32, port_start: u16, port_end: u16, allow: usize) -> Result<usize, &'static str> {
    crate::aslr::sandbox_net_add(pid, port_start, port_end, allow != 0)?;
    Ok(0)
}

/// Add a UID mapping entry (inner_uid -> outer_uid) for a process.
pub fn sys_sandbox_uid_map(pid: u32, inner_uid: u32, outer_uid: u32) -> Result<usize, &'static str> {
    crate::aslr::sandbox_uid_map(pid, inner_uid, outer_uid)?;
    Ok(0)
}

/// Report bits of entropy from the ASLR subsystem.
pub fn sys_aslr_entropy() -> Result<usize, &'static str> {
    Ok(crate::aslr::aslr_entropy() as usize)
}

// ── V28 WASM syscalls ────────────────────────────────────────────────────────
pub fn sys_wasm_load(name_ptr: usize, bytecode_ptr: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    let name = unsafe { core::slice::from_raw_parts(name_ptr as *const u8, 32) };
    let bc = unsafe { core::slice::from_raw_parts(bytecode_ptr as *const u8, 4096) };
    crate::wasm::wasm_load(pid, name, bc).ok_or("load failed")
}
pub fn sys_wasm_unload(module_id: usize) -> Result<usize, &'static str> {
    if crate::wasm::wasm_unload(module_id) { Ok(0) } else { Err("unload failed") }
}
pub fn sys_wasm_list(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 { return Err("null buf"); }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };
    Ok(crate::wasm::wasm_list(buf))
}
pub fn sys_wasm_execute(module_id: usize, name_ptr: usize) -> Result<usize, &'static str> {
    if name_ptr == 0 { return Err("null name"); }
    let mut name_buf = [0u8; 32];
    let nlen = unsafe {
        let mut len = 0;
        while len < 31 {
            let c = (name_ptr as *const u8).add(len).read_volatile();
            name_buf[len] = c;
            if c == 0 { break; }
            len += 1;
        }
        len
    };
    if nlen == 0 { return Err("empty name"); }
    let name = core::str::from_utf8(&name_buf[..nlen]).map_err(|_| "invalid utf8")?;
    match crate::wasm::wasm_execute(module_id, name, &[]) {
        Ok(result) => Ok(result as usize),
        Err(e) => Err(e),
    }
}
pub fn sys_wasm_mem_read(module_id: usize, offset: usize, buf_ptr: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 { return Err("null buf"); }
    let mut buf = [0u8; 256];
    let to_read = buf.len();
    let buf_slice = &mut buf[..to_read];
    crate::wasm::wasm_memory_read(module_id, offset, buf_slice)?;
    unsafe {
        core::ptr::copy_nonoverlapping(buf.as_ptr(), buf_ptr as *mut u8, to_read);
    }
    Ok(to_read)
}
pub fn sys_wasm_mem_write(module_id: usize, offset: usize, data_ptr: usize, data_len: usize) -> Result<usize, &'static str> {
    if data_ptr == 0 { return Err("null data"); }
    if data_len > 256 { return Err("data too large"); }
    let data = unsafe { core::slice::from_raw_parts(data_ptr as *const u8, data_len) };
    crate::wasm::wasm_memory_write(module_id, offset, data)?;
    Ok(data_len)
}

// ── V29 AI/GPU syscalls ──────────────────────────────────────────────────────

/// Register a GPU device (mmio_base, memory_base, memory_size) -> gpu_id
pub fn sys_gpu_register(mmio: usize, mem_base: usize, mem_size: usize) -> Result<usize, &'static str> {
    crate::ai::gpu_register(mmio, mem_base, mem_size).map(|id| id as usize).ok_or("full")
}

/// List GPU devices into a buffer
pub fn sys_gpu_list(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 { return Err("null buf"); }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };
    Ok(crate::ai::gpu_list(buf))
}

/// Submit an AI workload to the GPU queue
pub fn sys_ai_submit(gpu_id: u32, priority: usize, batch_size: u8, _data_ptr: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).ok_or("no proc")?;
    crate::ai::ai_submit(pid, gpu_id, priority as u8, batch_size as usize).ok_or("queue full")
}

/// Get next queued AI workload (for scheduler polling service)
pub fn sys_ai_next(_buf_ptr: usize, _buf_len: usize) -> Result<usize, &'static str> {
    crate::ai::ai_next_workload().ok_or("no work")
}

/// Submit GPU commands: gpu_id, cmd_buf_ptr, cmd_len
pub fn sys_gpu_submit_cmd(gpu_id: u32, cmd_buf_ptr: usize, cmd_len: usize) -> Result<usize, &'static str> {
    if cmd_buf_ptr == 0 || cmd_len == 0 || cmd_len > 4096 {
        return Err("invalid cmd args");
    }
    let cmd_buf = unsafe { core::slice::from_raw_parts(cmd_buf_ptr as *const u8, cmd_len) };
    crate::ai::gpu_submit_command(gpu_id as usize, cmd_buf, cmd_len)?;
    Ok(0)
}

/// Wait for a fence value on a GPU
pub fn sys_gpu_wait_fence(gpu_id: u32, fence: u64) -> Result<usize, &'static str> {
    crate::ai::gpu_wait_fence(gpu_id as usize, fence)?;
    Ok(0)
}

/// Allocate GPU memory: gpu_id, size -> gpu_va
pub fn sys_gpu_alloc(gpu_id: u32, size: usize) -> Result<usize, &'static str> {
    crate::ai::gpu_alloc(gpu_id, size).ok_or("gpu alloc failed")
}

/// Free GPU memory: gpu_id, gpu_va
pub fn sys_gpu_free(gpu_id: u32, gpu_va: usize) -> Result<usize, &'static str> {
    if crate::ai::gpu_free(gpu_id, gpu_va) { Ok(0) } else { Err("gpu free failed") }
}

/// Get GPU utilization (0-1000)
pub fn sys_gpu_utilization(gpu_id: u32) -> Result<usize, &'static str> {
    Ok(crate::ai::gpu_utilization(gpu_id) as usize)
}

/// Get number of active workloads on a GPU
pub fn sys_gpu_active_wl(gpu_id: u32) -> Result<usize, &'static str> {
    Ok(crate::ai::gpu_active_workloads(gpu_id))
}

/// Mark an AI workload as completed
pub fn sys_ai_complete(workload_id: usize, result: usize) -> Result<usize, &'static str> {
    if crate::ai::ai_complete(workload_id, result != 0) { Ok(0) } else { Err("bad workload id") }
}

/// Preempt a running AI workload
pub fn sys_ai_preempt(workload_id: usize) -> Result<usize, &'static str> {
    if crate::ai::ai_preempt(workload_id) { Ok(0) } else { Err("preempt failed") }
}

/// Load an ML model into GPU memory
pub fn sys_model_load(gpu_id: u32, model_data_ptr: usize, model_len: usize) -> Result<usize, &'static str> {
    if model_data_ptr == 0 || model_len == 0 || model_len > 0x100000 {
        return Err("invalid model data");
    }
    let model_data = model_data_ptr as *const u8;
    crate::ai::model_load(gpu_id, model_data, model_len)
        .map(|id| id as usize)
        .ok_or("model load failed")
}

/// Unload a model from GPU memory
pub fn sys_model_unload(model_id: u32) -> Result<usize, &'static str> {
    if crate::ai::model_unload(model_id) { Ok(0) } else { Err("unload failed") }
}

/// List loaded models into a buffer
pub fn sys_model_list(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 { return Err("null buf"); }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };
    Ok(crate::ai::model_list(buf))
}

/// Submit an inference job: model_id, input_gpu_va, output_gpu_va -> workload_id
pub fn sys_inference_submit(model_id: u32, input_tensor: u64, output_tensor: u64) -> Result<usize, &'static str> {
    crate::ai::inference_submit(model_id, input_tensor, output_tensor).ok_or("inference submit failed")
}

/// Get inference statistics for a model into a buffer
/// Returns: [count:8][total_us:8][max_us:8] = 24 bytes
pub fn sys_inference_stats(model_id: u32, buf_ptr: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 { return Err("null buf"); }
    match crate::ai::inference_stats(model_id) {
        Some((count, total_us, max_us)) => {
            let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, 24) };
            buf[0..8].copy_from_slice(&count.to_le_bytes());
            buf[8..16].copy_from_slice(&total_us.to_le_bytes());
            buf[16..24].copy_from_slice(&max_us.to_le_bytes());
            Ok(24)
        }
        None => Err("model not found"),
    }
}

// ── V30 System V Semaphores ──────────────────────────────────────────────────

const MAX_SEMS: usize = 16;
const MAX_SEM_NSEMS: usize = 4;

#[derive(Clone, Copy)]
struct Semaphore {
    key: u32,
    id: u32,
    values: [i16; MAX_SEM_NSEMS],
    nsems: usize,
    pid: u32,
}

static mut SEMAPHORES: [Semaphore; MAX_SEMS] = [Semaphore { key: 0, id: 0, values: [0; MAX_SEM_NSEMS], nsems: 0, pid: 0 }; MAX_SEMS];
static mut SEM_COUNT: usize = 0;
static mut SEM_NEXT_ID: u32 = 1;

/// sys_semget(key, nsems, semflg) — get/create a System V semaphore set.
pub fn sys_semget(key: u32, nsems: usize, _semflg: usize) -> Result<usize, &'static str> {
    unsafe {
        // IPC_PRIVATE (key=0): always create new
        if key == 0 {
            if SEM_COUNT >= MAX_SEMS { return Err("sem table full"); }
            let id = SEM_NEXT_ID;
            SEM_NEXT_ID += 1;
            let ns = nsems.min(MAX_SEM_NSEMS);
            SEMAPHORES[SEM_COUNT] = Semaphore { key, id, values: [0; MAX_SEM_NSEMS], nsems: ns, pid: 0 };
            SEM_COUNT += 1;
            return Ok(id as usize);
        }
        // Lookup existing
        for i in 0..SEM_COUNT {
            if SEMAPHORES[i].key == key {
                return Ok(SEMAPHORES[i].id as usize);
            }
        }
        // Create new with key
        if SEM_COUNT >= MAX_SEMS { return Err("sem table full"); }
        let id = SEM_NEXT_ID;
        SEM_NEXT_ID += 1;
        let ns = nsems.min(MAX_SEM_NSEMS);
        SEMAPHORES[SEM_COUNT] = Semaphore { key, id, values: [0; MAX_SEM_NSEMS], nsems: ns, pid: 0 };
        SEM_COUNT += 1;
        Ok(id as usize)
    }
}

/// sys_semop(semid, sops_ptr, nsops) — perform semaphore operations.
pub fn sys_semop(semid: u32, sops_ptr: usize, _nsops: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).unwrap_or(0);
    unsafe {
        for i in 0..SEM_COUNT {
            if SEMAPHORES[i].id == semid {
                if sops_ptr == 0 { return Err("null sops"); }
                // struct sembuf { sem_num: u16, sem_op: i16, sem_flg: i16 }
                let sem_num = (sops_ptr as *const u16).read_volatile() as usize;
                let sem_op = (sops_ptr as *const i16).add(1).read_volatile();
                if sem_num >= SEMAPHORES[i].nsems { return Err("bad sem_num"); }
                // Simple implementation: block if operation would go negative
                loop {
                    let val = SEMAPHORES[i].values[sem_num];
                    let new_val = val.wrapping_add(sem_op);
                    if new_val >= 0 || sem_op == 0 || sem_op == -1 {
                        SEMAPHORES[i].values[sem_num] = new_val;
                        SEMAPHORES[i].pid = pid;
                        return Ok(0);
                    }
                    // Would block — yield and retry
                    crate::sched::schedule();
                }
            }
        }
    }
    Err("sem not found")
}

/// sys_semctl(semid, semnum, cmd, arg) — semaphore control operations.
pub fn sys_semctl(semid: u32, semnum: u32, cmd: usize, _arg: usize) -> Result<usize, &'static str> {
    unsafe {
        for i in 0..SEM_COUNT {
            if SEMAPHORES[i].id == semid {
                let result = match cmd {
                    0 => { // SETVAL
                        SEMAPHORES[i].values[semnum as usize] = _arg as i16;
                        Ok(1)
                    }
                    1 => Ok(SEMAPHORES[i].values[semnum as usize] as usize), // GETVAL
                    6 => Ok(0), // GETPID
                    2 => Ok(0), // GETNCNT
                    3 => Ok(0), // GETZCNT
                    9 => { // IPC_STAT
                        let pid = crate::sched::current_thread()
                            .map(|t| unsafe { (*t).owner }).unwrap_or(0);
                        let _ = pid;
                        Ok(0)
                    }
                    8 => { // IPC_SET
                        Ok(0)
                    }
                    10 => { // IPC_RMID
                        for j in i..SEM_COUNT - 1 {
                            SEMAPHORES[j] = SEMAPHORES[j + 1];
                        }
                        SEM_COUNT -= 1;
                        Ok(0)
                    }
                    _ => Err("unsupported semctl cmd"),
                };
                return result;
            }
        }
    }
    Err("sem not found")
}

// ── V30 System V Message Queues ──────────────────────────────────────────────

const MAX_MSGQS: usize = 8;
const MAX_MSG_SLOTS: usize = 64;
const MAX_MSG_SIZE: usize = 60;

#[derive(Clone, Copy)]
struct Msg {
    mtype: i64,
    data: [u8; MAX_MSG_SIZE],
    len: usize,
    valid: bool,
}

#[derive(Clone, Copy)]
struct MsgQueue {
    key: u32,
    id: u32,
    msgs: [Msg; MAX_MSG_SLOTS],
    write_idx: usize,
    read_idx: usize,
    count: usize,
}

static mut MSG_QUEUES: [MsgQueue; MAX_MSGQS] = [MsgQueue {
    key: 0, id: 0,
    msgs: [Msg { mtype: 0, data: [0; MAX_MSG_SIZE], len: 0, valid: false }; MAX_MSG_SLOTS],
    write_idx: 0, read_idx: 0, count: 0,
}; MAX_MSGQS];
static mut MSGQ_COUNT: usize = 0;
static mut MSGQ_NEXT_ID: u32 = 1;

/// sys_msgget(key, msgflg) — get/create a message queue.
pub fn sys_msgget(key: u32, _msgflg: usize) -> Result<usize, &'static str> {
    unsafe {
        if key == 0 {
            if MSGQ_COUNT >= MAX_MSGQS { return Err("msgq table full"); }
            let id = MSGQ_NEXT_ID;
            MSGQ_NEXT_ID += 1;
            MSG_QUEUES[MSGQ_COUNT].key = key;
            MSG_QUEUES[MSGQ_COUNT].id = id;
            MSG_QUEUES[MSGQ_COUNT].write_idx = 0;
            MSG_QUEUES[MSGQ_COUNT].read_idx = 0;
            MSG_QUEUES[MSGQ_COUNT].count = 0;
            MSGQ_COUNT += 1;
            return Ok(id as usize);
        }
        for i in 0..MSGQ_COUNT {
            if MSG_QUEUES[i].key == key { return Ok(MSG_QUEUES[i].id as usize); }
        }
        if MSGQ_COUNT >= MAX_MSGQS { return Err("msgq table full"); }
        let id = MSGQ_NEXT_ID;
        MSGQ_NEXT_ID += 1;
        MSG_QUEUES[MSGQ_COUNT].key = key;
        MSG_QUEUES[MSGQ_COUNT].id = id;
        MSG_QUEUES[MSGQ_COUNT].write_idx = 0;
        MSG_QUEUES[MSGQ_COUNT].read_idx = 0;
        MSG_QUEUES[MSGQ_COUNT].count = 0;
        MSGQ_COUNT += 1;
        Ok(id as usize)
    }
}

/// sys_msgsnd(msqid, msgp_ptr, msgsz, msgflg) — send a message.
pub fn sys_msgsnd(msqid: u32, msgp_ptr: usize, msgsz: usize, _msgflg: usize) -> Result<usize, &'static str> {
    if msgp_ptr == 0 { return Err("null msgp"); }
    let copy_len = msgsz.min(MAX_MSG_SIZE);
    unsafe {
        for i in 0..MSGQ_COUNT {
            if MSG_QUEUES[i].id == msqid {
                // Wait for space
                loop {
                    if MSG_QUEUES[i].count < MAX_MSG_SLOTS { break; }
                    crate::sched::schedule();
                }
                let idx = MSG_QUEUES[i].write_idx;
                let mtype = (msgp_ptr as *const i64).read_volatile();
                MSG_QUEUES[i].msgs[idx].mtype = mtype;
                MSG_QUEUES[i].msgs[idx].len = copy_len;
                MSG_QUEUES[i].msgs[idx].valid = true;
                core::ptr::copy_nonoverlapping(
                    (msgp_ptr as *const u8).add(8),
                    MSG_QUEUES[i].msgs[idx].data.as_mut_ptr(),
                    copy_len,
                );
                MSG_QUEUES[i].write_idx = (idx + 1) % MAX_MSG_SLOTS;
                MSG_QUEUES[i].count += 1;
                return Ok(0);
            }
        }
    }
    Err("msgq not found")
}

/// sys_msgrcv(msqid, msgp_ptr, msgsz, msgtyp, msgflg) — receive a message.
pub fn sys_msgrcv(msqid: u32, msgp_ptr: usize, msgsz: usize, msgtyp: i64, _msgflg: usize) -> Result<usize, &'static str> {
    if msgp_ptr == 0 { return Err("null msgp"); }
    unsafe {
        for i in 0..MSGQ_COUNT {
            if MSG_QUEUES[i].id == msqid {
                loop {
                    // Find matching message
                    let mut found = false;
                    let mut found_idx = 0;
                    for scan in 0..MAX_MSG_SLOTS {
                        let idx = (MSG_QUEUES[i].read_idx + scan) % MAX_MSG_SLOTS;
                        if !MSG_QUEUES[i].msgs[idx].valid { continue; }
                        if msgtyp == 0 || MSG_QUEUES[i].msgs[idx].mtype == msgtyp {
                            found = true;
                            found_idx = idx;
                            break;
                        }
                    }
                    if found {
                        let copy_len = MSG_QUEUES[i].msgs[found_idx].len.min(msgsz);
                        let mtype = MSG_QUEUES[i].msgs[found_idx].mtype;
                        let data = MSG_QUEUES[i].msgs[found_idx].data;
                        MSG_QUEUES[i].msgs[found_idx].valid = false;
                        MSG_QUEUES[i].count -= 1;
                        // Write mtype
                        (msgp_ptr as *mut i64).write_volatile(mtype);
                        core::ptr::copy_nonoverlapping(
                            data.as_ptr(),
                            (msgp_ptr as *mut u8).add(8),
                            copy_len,
                        );
                        MSG_QUEUES[i].read_idx = (found_idx + 1) % MAX_MSG_SLOTS;
                        return Ok(copy_len);
                    }
                    // No message — block
                    crate::sched::schedule();
                }
            }
        }
    }
    Err("msgq not found")
}

/// sys_msgctl(msqid, cmd, buf) — message queue control.
pub fn sys_msgctl(msqid: u32, cmd: u32, _buf: usize) -> Result<usize, &'static str> {
    unsafe {
        for i in 0..MSGQ_COUNT {
            if MSG_QUEUES[i].id == msqid {
                let result = match cmd {
                    0 => { // IPC_RMID
                        for j in i..MSGQ_COUNT - 1 { MSG_QUEUES[j] = MSG_QUEUES[j + 1]; }
                        MSGQ_COUNT -= 1;
                        Ok(0)
                    }
                    _ => Ok(0), // IPC_STAT, IPC_SET
                };
                return result;
            }
        }
    }
    Err("msgq not found")
}

// ── V30 Signal Enhancements ──────────────────────────────────────────────────

const MAX_SIGNAL_HANDLERS: usize = 32;
const SIG_BLOCK: u32 = 0;
const SIG_UNBLOCK: u32 = 1;
const SIG_SETMASK: u32 = 2;

#[derive(Clone, Copy)]
struct SignalHandler {
    pid: u32,
    signum: u32,
    handler: usize,   // user-space handler address
    flags: usize,
    mask: u64,        // signal mask during handler
}

static mut SIGNAL_HANDLERS: [SignalHandler; MAX_SIGNAL_HANDLERS] = [
    SignalHandler { pid: 0, signum: 0, handler: 0, flags: 0, mask: 0 }; MAX_SIGNAL_HANDLERS
];
static mut SIGNAL_HANDLER_COUNT: usize = 0;
static mut SIGNAL_MASKS: [(u32, u64); 64] = [(0, 0); 64];
static mut SIGNAL_MASK_COUNT: usize = 0;

fn get_signal_mask(pid: u32) -> u64 {
    unsafe {
        for i in 0..SIGNAL_MASK_COUNT {
            if SIGNAL_MASKS[i].0 == pid { return SIGNAL_MASKS[i].1; }
        }
    }
    0
}

fn set_signal_mask(pid: u32, mask: u64) {
    unsafe {
        for i in 0..SIGNAL_MASK_COUNT {
            if SIGNAL_MASKS[i].0 == pid {
                SIGNAL_MASKS[i].1 = mask;
                return;
            }
        }
        if SIGNAL_MASK_COUNT < 64 {
            SIGNAL_MASKS[SIGNAL_MASK_COUNT] = (pid, mask);
            SIGNAL_MASK_COUNT += 1;
        }
    }
}

/// sys_sigaction(signum, act_ptr, oldact_ptr) — examine/change signal action.
pub fn sys_sigaction(signum: u32, act_ptr: usize, oldact_ptr: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).unwrap_or(0);
    if signum == 9 || signum == 19 { return Err("cannot change SIGKILL/SIGSTOP"); }

    unsafe {
        // Return old action if requested
        if oldact_ptr != 0 {
            let mut found = false;
            for i in 0..SIGNAL_HANDLER_COUNT {
                if SIGNAL_HANDLERS[i].pid == pid && SIGNAL_HANDLERS[i].signum == signum {
                    let old_ptr = oldact_ptr as *mut usize;
                    old_ptr.write_volatile(SIGNAL_HANDLERS[i].handler);
                    old_ptr.add(1).write_volatile(SIGNAL_HANDLERS[i].flags);
                    found = true;
                    break;
                }
            }
            if !found {
                let old_ptr = oldact_ptr as *mut usize;
                old_ptr.write_volatile(0); // SIG_DFL
                old_ptr.add(1).write_volatile(0);
            }
        }

        // Set new action
        if act_ptr != 0 {
            let new_handler = (act_ptr as *const usize).read_volatile();
            let new_flags = (act_ptr as *const usize).add(1).read_volatile();
            let mut found = false;
            for i in 0..SIGNAL_HANDLER_COUNT {
                if SIGNAL_HANDLERS[i].pid == pid && SIGNAL_HANDLERS[i].signum == signum {
                    SIGNAL_HANDLERS[i].handler = new_handler;
                    SIGNAL_HANDLERS[i].flags = new_flags;
                    found = true;
                    break;
                }
            }
            if !found && SIGNAL_HANDLER_COUNT < MAX_SIGNAL_HANDLERS {
                SIGNAL_HANDLERS[SIGNAL_HANDLER_COUNT] = SignalHandler {
                    pid, signum, handler: new_handler, flags: new_flags, mask: 0,
                };
                SIGNAL_HANDLER_COUNT += 1;
            }
        }
    }
    Ok(0)
}

/// sys_sigprocmask(how, set_ptr, oldset_ptr) — examine/change signal mask.
pub fn sys_sigprocmask(how: u32, set_ptr: usize, oldset_ptr: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).unwrap_or(0);

    let old_mask = get_signal_mask(pid);

    // Return old mask if requested
    if oldset_ptr != 0 {
        unsafe { (oldset_ptr as *mut u64).write_volatile(old_mask); }
    }

    // Update mask
    if set_ptr != 0 {
        let set_val = unsafe { (set_ptr as *const u64).read_volatile() };
        match how {
            SIG_BLOCK => set_signal_mask(pid, old_mask | set_val),
            SIG_UNBLOCK => set_signal_mask(pid, old_mask & !set_val),
            SIG_SETMASK => set_signal_mask(pid, set_val),
            _ => return Err("bad how"),
        }
    }

    Ok(0)
}

/// sys_sigreturn() — return from signal handler.
pub fn sys_sigreturn() -> Result<usize, &'static str> {
    Ok(0)
}

/// sys_sigpending(set_ptr) — examine pending signals.
pub fn sys_sigpending(set_ptr: usize) -> Result<usize, &'static str> {
    if set_ptr != 0 {
        unsafe { (set_ptr as *mut u64).write_volatile(0); }
    }
    Ok(0)
}

// ── V30 Poll/Select ──────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

/// sys_poll(fds_ptr, nfds, timeout) — wait for I/O events.
pub fn sys_poll(fds_ptr: usize, nfds: usize, _timeout: isize) -> Result<usize, &'static str> {
    if fds_ptr == 0 { return Err("null fds"); }
    if nfds == 0 { return Ok(0); }

    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).unwrap_or(0);
    let max_retries = 1000; // limit spinning

    for _ in 0..max_retries {
        let mut ready = 0;
        unsafe {
            for i in 0..nfds {
                let entry_ptr = (fds_ptr + i * 8) as *const i16; // sizeof(pollfd)=8
                let fd = (entry_ptr as *const i32).read_volatile();
                let events = entry_ptr.add(2).read_volatile();

                if fd < 0 { continue; }

                let mut revents: i16 = 0;
                // Check if fd can read (POLLIN=1)
                if events & 1 != 0 {
                    // For STDIN (fd=0): always ready
                    if fd == 0 { revents |= 1; }
                    // Check if there's data to read (simplified: always ready for files)
                    else { revents |= 1; }
                }
                // Check if fd can write (POLLOUT=4)
                if events & 4 != 0 {
                    revents |= 4;
                }

                let rp = entry_ptr.add(4) as *mut i16;
                rp.write_volatile(revents);
                if revents != 0 { ready += 1; }
            }
        }
        if ready > 0 { return Ok(ready); }
        crate::sched::schedule();
    }
    Ok(0)
}

/// sys_ppoll(fds_ptr, nfds, timeout, sigmask) — poll with signal mask.
pub fn sys_ppoll(fds_ptr: usize, nfds: usize, timeout: usize, _sigmask: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).unwrap_or(0);
    let _ = pid;
    // Delegate to poll
    sys_poll(fds_ptr, nfds, timeout as isize)
}

/// sys_pselect6(nfds, readfds, writefds, exceptfds, timeout) — synchronous I/O multiplexing.
pub fn sys_pselect6(nfds: usize, _readfds: usize, _writefds: usize, _exceptfds: usize, _timeout: usize) -> Result<usize, &'static str> {
    // Simplified: return ready fds count
    let mut ready = 0;
    if nfds > 0 {
        // Fd 0 (stdin) is always readable
        if _readfds != 0 {
            unsafe {
                let byte = (_readfds as *const u8).read_volatile();
                if byte != 0 { ready = 1; }
            }
        }
    }
    if ready == 0 {
        crate::sched::schedule();
    }
    Ok(ready)
}

// ── V30 Process Control (prctl, priority) ────────────────────────────────────

const PR_SET_NAME: usize = 15;
const PR_GET_NAME: usize = 16;
pub static mut PROCESS_NAMES: [(u32, [u8; 16]); 64] = [(0, [0u8; 16]); 64];
pub static mut PROCESS_NAME_COUNT: usize = 0;

/// sys_prctl(option, arg2, arg3) — process control.
pub fn sys_prctl(option: usize, arg2: usize, arg3: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).unwrap_or(0);
    match option {
        PR_SET_NAME => {
            if arg2 == 0 { return Err("null name"); }
            let mut name = [0u8; 16];
            unsafe {
                let src = arg2 as *const u8;
                for i in 0..15 {
                    let c = src.add(i).read_volatile();
                    name[i] = c;
                    if c == 0 { break; }
                }
            }
            unsafe {
                for i in 0..PROCESS_NAME_COUNT {
                    if PROCESS_NAMES[i].0 == pid {
                        PROCESS_NAMES[i].1 = name;
                        return Ok(0);
                    }
                }
                if PROCESS_NAME_COUNT < 64 {
                    PROCESS_NAMES[PROCESS_NAME_COUNT] = (pid, name);
                    PROCESS_NAME_COUNT += 1;
                }
            }
            Ok(0)
        }
        PR_GET_NAME => {
            if arg2 == 0 { return Err("null buf"); }
            unsafe {
                let mut found = false;
                for i in 0..PROCESS_NAME_COUNT {
                    if PROCESS_NAMES[i].0 == pid {
                        core::ptr::copy_nonoverlapping(
                            PROCESS_NAMES[i].1.as_ptr(),
                            arg2 as *mut u8, 16,
                        );
                        found = true;
                        break;
                    }
                }
                if !found {
                    let default = b"trainos\0";
                    core::ptr::copy_nonoverlapping(default.as_ptr(), arg2 as *mut u8, 8);
                }
            }
            Ok(0)
        }
        _ => Err("unsupported prctl option"),
    }
}

/// sys_getpriority(which, who) — get process priority (nice value).
pub fn sys_getpriority(which: usize, who: usize) -> Result<usize, &'static str> {
    let _ = (which, who);
    Ok(0) // return default nice value of 0
}

/// sys_setpriority(which, who, prio) — set process priority.
pub fn sys_setpriority(which: usize, who: usize, prio: usize) -> Result<usize, &'static str> {
    let _ = (which, who, prio);
    Ok(0)
}

/// sys_sched_getparam(pid, param_ptr) — get scheduling parameters.
pub fn sys_sched_getparam(pid: u32, param_ptr: usize) -> Result<usize, &'static str> {
    if param_ptr == 0 { return Err("null param"); }
    let procs = crate::proc::PROCESSES.lock();
    for proc in procs.iter() {
        if proc.pid == pid {
            unsafe { (param_ptr as *mut u32).write_volatile(proc.base_priority as u32); }
            return Ok(0);
        }
    }
    Err("process not found")
}

/// sys_sched_setparam(pid, param_ptr) — set scheduling parameters.
pub fn sys_sched_setparam(pid: u32, param_ptr: usize) -> Result<usize, &'static str> {
    if param_ptr == 0 { return Err("null param"); }
    let new_priority = unsafe { (param_ptr as *const u32).read_volatile() as u8 };
    let mut procs = crate::proc::PROCESSES.lock();
    for proc in procs.iter_mut() {
        if proc.pid == pid {
            proc.base_priority = new_priority;
            if let Some(ref mut thread) = proc.thread {
                thread.effective_priority = new_priority;
            }
            return Ok(0);
        }
    }
    Err("process not found")
}

// ── V30 Linux compat syscalls ────────────────────────────────────────────────
pub fn sys_compat_init() -> Result<usize, &'static str> { crate::compat::compat_init(); Ok(0) }
pub fn sys_compat_translate(linux_nr: usize) -> Result<usize, &'static str> {
    crate::compat::translate_syscall(linux_nr).map(|(nr, _needs_trans)| nr).ok_or("no mapping")
}
pub fn sys_compat_setup_auxv(stack_top: usize, entry: usize, phdr: usize, phent: usize, phnum: usize) -> Result<usize, &'static str> {
    Ok(crate::compat::setup_auxv(stack_top, entry, phdr, phent, phnum))
}

// ── V34 AI-Native Scheduling syscalls ──────────────────────────────────────

/// Submit a P/D workload pair.
pub fn sys_pd_submit(ctx_ptr: usize, ctx_len: usize, model_id: u32, gpu_id: u32) -> Result<usize, &'static str> {
    let ctx = if ctx_ptr != 0 && ctx_len > 0 {
        let copy_len = ctx_len.min(64);
        unsafe { core::slice::from_raw_parts(ctx_ptr as *const u8, copy_len) }
    } else {
        &[]
    };
    match crate::ai::pd_sched::pd_submit(ctx, model_id, gpu_id) {
        Some((prefill_id, decode_id)) => Ok(prefill_id | (decode_id << 16)),
        None => Err("pd submit failed"),
    }
}

/// Get the next decode step to execute.
pub fn sys_pd_next_decode() -> Result<usize, &'static str> {
    crate::ai::pd_sched::pd_next_decode_step().ok_or("no decode ready")
}

/// Get the next prefill batch.
pub fn sys_pd_next_prefill(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    let batch = crate::ai::pd_sched::pd_next_prefill_batch();
    if buf_ptr == 0 || buf_len < 4 {
        return Ok(batch.len());
    }
    let write_count = batch.len().min(buf_len / 4);
    unsafe {
        for i in 0..write_count {
            (buf_ptr as *mut u32).add(i).write_volatile(batch[i] as u32);
        }
    }
    Ok(write_count)
}

/// Preempt a decode workload.
pub fn sys_pd_preempt(workload_id: usize) -> Result<usize, &'static str> {
    crate::ai::pd_sched::pd_preempt_decode(workload_id);
    Ok(0)
}

/// Resume a preempted decode workload.
pub fn sys_pd_resume(workload_id: usize) -> Result<usize, &'static str> {
    crate::ai::pd_sched::pd_resume_decode(workload_id);
    Ok(0)
}

/// Allocate KV-cache pages for a token sequence.
pub fn sys_kv_alloc(token_count: usize) -> Result<usize, &'static str> {
    match crate::ai::kvcache::kv_alloc_pages(token_count) {
        Some(pages) => {
            if pages.is_empty() {
                Ok(0)
            } else {
                let first = pages[0];
                let count = pages.len();
                if first < 0x10000 && count < 0x10000 {
                    Ok(first | (count << 16))
                } else {
                    Ok(first)
                }
            }
        }
        None => Err("kv alloc failed"),
    }
}

/// Free KV-cache pages.
pub fn sys_kv_free(pages_ptr: usize, count: usize) -> Result<usize, &'static str> {
    if pages_ptr == 0 {
        return Err("null ptr");
    }
    let count = count.min(64);
    let mut pages = [0usize; 64];
    unsafe {
        for i in 0..count {
            pages[i] = (pages_ptr as *const u32).add(i).read_volatile() as usize;
        }
    }
    crate::ai::kvcache::kv_free_pages(&pages[..count]);
    Ok(0)
}

/// Share KV-cache pages between workloads.
pub fn sys_kv_share(pages_ptr: usize, count: usize) -> Result<usize, &'static str> {
    if pages_ptr == 0 {
        return Err("null ptr");
    }
    let count = count.min(64);
    let mut pages = [0usize; 64];
    unsafe {
        for i in 0..count {
            pages[i] = (pages_ptr as *const u32).add(i).read_volatile() as usize;
        }
    }
    crate::ai::kvcache::kv_share_pages(&pages[..count])?;
    Ok(0)
}

/// Get KV-cache statistics.
pub fn sys_kv_stats(buf_ptr: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 {
        return Err("null buf");
    }
    let util = crate::ai::kvcache::kv_utilization();
    let util_int = (util * 1000.0) as u32;
    unsafe {
        (buf_ptr as *mut u32).write_volatile(crate::ai::kvcache::kv_allocated_page_count() as u32);
        (buf_ptr as *mut u32).add(1).write_volatile(crate::ai::kvcache::kv_dirty_page_count() as u32);
        (buf_ptr as *mut u32).add(2).write_volatile(util_int);
    }
    Ok(12)
}

/// GPU-CPU heterogeneous scheduling.
pub fn sys_gpu_hetero_sched(gpu_id: u32, workload_id: usize) -> Result<usize, &'static str> {
    let wl = unsafe {
        crate::ai::pd_sched::PD_SCHEDULER.get_workload(workload_id)
            .ok_or("workload not found")?
    };
    let sched = unsafe { &crate::ai::HETERO_SCHEDULER };
    match sched.schedule(wl) {
        Some((best_gpu, best_node)) => Ok(best_gpu as usize | ((best_node as usize) << 16)),
        None => Err("no suitable gpu found"),
    }
}

/// Migrate a workload to a different GPU.
pub fn sys_gpu_migrate(workload_id: usize, to_gpu: u32) -> Result<usize, &'static str> {
    unsafe {
        crate::ai::HETERO_SCHEDULER.migrate_workload(workload_id, to_gpu)?;
    }
    unsafe {
        crate::ai::AI_SCHED_STATS.page_migrations =
            crate::ai::AI_SCHED_STATS.page_migrations.wrapping_add(1);
    }
    Ok(0)
}

/// Balance GPU load across available devices.
pub fn sys_gpu_balance() -> Result<usize, &'static str> {
    unsafe {
        crate::ai::HETERO_SCHEDULER.balance_gpu_load();
    }
    Ok(0)
}

/// Get AI scheduling statistics.
pub fn sys_ai_sched_stats(buf_ptr: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 {
        return Err("null buf");
    }
    let stats = crate::ai::ai_sched_stats();
    unsafe {
        let ptr = buf_ptr as *mut u64;
        ptr.add(0).write_volatile(stats.prefill_workloads);
        ptr.add(1).write_volatile(stats.decode_steps);
        ptr.add(2).write_volatile(stats.kv_cache_hits);
        ptr.add(3).write_volatile(stats.kv_cache_misses);
        ptr.add(4).write_volatile(stats.kv_cache_evictions);
        ptr.add(5).write_volatile(stats.page_migrations);
        ptr.add(6).write_volatile(stats.gpu_balance_operations);
        ptr.add(7).write_volatile(stats.avg_prefill_latency_us);
        ptr.add(8).write_volatile(stats.avg_decode_latency_us);
        ptr.add(9).write_volatile(stats.p99_decode_latency_us);
    }
    Ok(80)
}

/// Reset AI scheduling statistics.
pub fn sys_ai_sched_reset() -> Result<usize, &'static str> {
    crate::ai::ai_sched_reset_stats();
    Ok(0)
}

// ────────────────────────────────────────────────────────────────────────────
// V36a — RVV 1.0 Vector Extension syscalls
// ────────────────────────────────────────────────────────────────────────────

/// Grant vector capability to a process.
/// The calling process must have CAP_VECTOR capability itself (root).
/// Returns 0 on success.
pub fn sys_cap_vector_enable(pid: u32) -> Result<usize, &'static str> {
    let caller_pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no proc")?;

    // Only root (uid 0) processes can grant vector capability
    let caller_uid = {
        let procs = crate::proc::PROCESSES.lock();
        let caller = procs.iter().find(|p| p.pid == caller_pid).ok_or("caller not found")?;
        caller.uid
    };
    if caller_uid != 0 {
        return Err("permission denied: need root");
    }

    // Find target process and allocate vector capability resource
    let mut procs = crate::proc::PROCESSES.lock();
    let target = procs.iter_mut().find(|p| p.pid == pid).ok_or("process not found")?;

    // Allocate a vector capability resource
    let cnode_id = target.cnode_id;
    let resource_id = crate::cap::ops::alloc_resource(
        crate::cap::types::CapType::Vector,
        crate::cap::types::ResourceData::Vector { pid },
    );

    // Insert into target process's CNode
    use crate::cap::types::{CapRef, Rights, RIGHT_EXEC};
    let cap_ref = CapRef {
        cap_type: crate::cap::types::CapType::Vector,
        rights: RIGHT_EXEC,  // EXEC right = permission to use vector instructions
        resource_id,
    };
    if let Some(res) = crate::cap::ops::get_resource(cnode_id) {
        if let crate::cap::types::ResourceData::CNode { ref slots } = &res.data {
            let mut slots = slots.lock();
            // Find first empty slot
            for i in 0..slots.len() {
                if slots[i].cap_type == crate::cap::types::CapType::Null {
                    slots[i] = crate::cap::types::Slot {
                        cap_type: crate::cap::types::CapType::Vector,
                        rights: RIGHT_EXEC,
                        resource_id,
                    };
                    drop(slots);
                    crate::println!("  VECTOR: cap granted to pid={} by pid={}", pid, caller_pid);
                    return Ok(0);
                }
            }
            return Err("cnode full");
        }
    }

    Err("cnode not found")
}

/// Read vector extension statistics into a user buffer.
/// Format: [vlen:8][tasks:8][saves:8][restores:8][lazy_traps:8] = 40 bytes
pub fn sys_vector_stats(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 { return Err("null buf"); }
    if buf_len < 40 { return Err("buffer too small"); }

    let stats = &crate::mem::vector::VECTOR_STATS;
    unsafe {
        let ptr = buf_ptr as *mut u64;
        ptr.add(0).write_volatile(stats.vlen.load(core::sync::atomic::Ordering::Relaxed));
        ptr.add(1).write_volatile(stats.vector_tasks.load(core::sync::atomic::Ordering::Relaxed));
        ptr.add(2).write_volatile(stats.vector_saves.load(core::sync::atomic::Ordering::Relaxed));
        ptr.add(3).write_volatile(stats.vector_restores.load(core::sync::atomic::Ordering::Relaxed));
        ptr.add(4).write_volatile(stats.vector_lazy_traps.load(core::sync::atomic::Ordering::Relaxed));
    }
    Ok(40)
}
