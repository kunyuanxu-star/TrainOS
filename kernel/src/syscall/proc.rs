use crate::mem::{sv39, buddy};

pub fn sys_spawn(_elf_ptr: usize, _elf_len: usize) -> Result<usize, &'static str> {
    // In a real implementation, copy ELF from user space
    // For now, this is a placeholder
    Err("spawn not implemented via syscall")
}

pub fn sys_exit(code: i32) -> Result<usize, &'static str> {
    let current = crate::sched::current_thread().ok_or("no thread")?;
    unsafe { (*current).state = crate::proc::thread::ThreadState::Dead; }
    crate::sched::schedule();
    // Never returns
    loop { unsafe { core::arch::asm!("wfi"); } }
}

/// Map a physical MMIO region into the current process's page table.
/// phys: physical base address (must be page-aligned)
/// size: region size in bytes (will be rounded up to page boundary)
/// Returns: virtual address of the mapping
pub fn sys_mmio_map(phys: usize, size: usize) -> Result<usize, &'static str> {
    if phys == 0 || size == 0 { return Err("invalid args"); }
    if phys & 0xFFF != 0 { return Err("phys not page-aligned"); }

    // Get current process
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).ok_or("no current process")?;

    let procs = crate::proc::PROCESSES.lock();
    let proc = procs.iter().find(|p| p.pid == pid).ok_or("process not found")?;
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
            crate::proc::elf::map_into_pt(
                pt_root, va, pa, true, true, false, true
            );
        }
    }

    Ok(vbase)
}

/// Fork the current process. Returns 0 in child, child_pid in parent.
/// `parent_sepc` is the saved sepc from the current trap frame (address of ecall instruction).
pub fn sys_fork(parent_sepc: usize) -> Result<usize, &'static str> {
    // Get current process
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).ok_or("no current process")?;

    let (pt_root, user_sp, satp_val, priority) = {
        let procs = crate::proc::PROCESSES.lock();
        let proc = procs.iter().find(|p| p.pid == pid).ok_or("process not found")?;
        let thread = proc.thread.as_ref().ok_or("no thread")?;
        (proc.page_table_root,
         thread.trap_frame.as_ref().unwrap().user_sp,
         thread.trap_frame.as_ref().unwrap().satp,
         thread.effective_priority)
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
    let child_pid = crate::proc::fork_child(child_pt, pt_root, child_entry, user_sp, child_satp, priority).ok_or("fork_child failed")?;

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
        if !l2_entry.is_valid() { continue; }
        if l2_entry.is_leaf() { continue; } // skip superpages at L2

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
            if !l1_entry.is_valid() { continue; }

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
                    if !l0_entry.is_valid() || !l0_entry.is_leaf() { continue; }

                    if l0_entry.is_writable() || l0_entry.is_dirty() {
                        // Allocate a new physical page for the child
                        let new_page = buddy::alloc_page().ok_or("OOM")?;
                        let old_kva = sv39::pa_to_kva(l0_entry.phys_addr());
                        let new_kva = sv39::pa_to_kva(new_page);
                        // Copy page content from parent to child
                        core::ptr::copy_nonoverlapping(old_kva as *const u8, new_kva as *mut u8, 4096);

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
