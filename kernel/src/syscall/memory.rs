// Memory management syscalls: mmap, munmap, mprotect, brk

use crate::mem::{buddy, sv39, layout::PAGE_SIZE};

fn current_pid() -> Result<u32, &'static str> {
    crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .ok_or("no proc")
}

/// Get the page table root for a process. Returns the physical frame.
fn get_page_table_root(pid: u32) -> Option<usize> {
    let procs = crate::proc::PROCESSES.lock();
    procs.iter().find(|p| p.pid == pid).map(|p| p.page_table_root)
}

/// sys_brk(addr) — set program break (heap end).
pub fn sys_brk(addr: usize) -> Result<usize, &'static str> {
    let pid = current_pid()?;

    static mut BRK_TABLE: [(u32, usize); 8] = [(0, 0); 8];
    static mut BRK_COUNT: usize = 0;

    let default_brk: usize = 0x40_0000; // 4MB user heap start

    unsafe {
        let mut entry_idx = BRK_COUNT;
        for i in 0..BRK_COUNT {
            if BRK_TABLE[i].0 == pid { entry_idx = i; break; }
        }
        if entry_idx == BRK_COUNT {
            if BRK_COUNT >= 8 { return Err("too many brk entries"); }
            BRK_TABLE[BRK_COUNT] = (pid, default_brk);
            BRK_COUNT += 1;
        }

        let current_brk = BRK_TABLE[entry_idx].1;
        if addr == 0 { return Ok(current_brk); }

        let pt_root = get_page_table_root(pid).ok_or("no proc pt")?;

        if addr > current_brk {
            let old_pages = (current_brk + 0xFFF) >> 12;
            let new_pages = (addr + 0xFFF) >> 12;
            for page_idx in old_pages..new_pages {
                let va = page_idx << 12;
                let page = buddy::alloc_page().ok_or("OOM")?;
                sv39::map_user_page(pt_root, va, page, true, true)?;
            }
        } else if addr < current_brk {
            let old_pages = (current_brk + 0xFFF) >> 12;
            let new_pages = (addr + 0xFFF) >> 12;
            for page_idx in new_pages..old_pages {
                let va = page_idx << 12;
                sv39::unmap_user_page(pt_root, va);
            }
        }

        BRK_TABLE[entry_idx].1 = addr;
        Ok(addr)
    }
}

/// sys_mmap(addr, length, prot, flags, fd, offset) — map memory
pub fn sys_mmap(
    addr: usize, length: usize, _prot: usize, _flags: usize, _fd: isize, _offset: isize,
) -> Result<usize, &'static str> {
    let pid = current_pid()?;
    let pt_root = get_page_table_root(pid).ok_or("no proc pt")?;
    let pages = (length + PAGE_SIZE - 1) >> 12;
    let va = if addr == 0 { 0x1000_0000 } else { addr };

    // V27.1: CHERI validation — check that the mmap range is authorized
    let required_perms = crate::aslr::CHERI_PERM_R | crate::aslr::CHERI_PERM_W;
    if !crate::aslr::validate_ptr(pid, va, length, required_perms) {
        return Err("cheri: mmap range not authorized");
    }

    for i in 0..pages {
        let page_va = va + i * PAGE_SIZE;
        let page = buddy::alloc_page().ok_or("OOM")?;
        unsafe {
            sv39::map_user_page(pt_root, page_va, page, true, true)?;
        }
    }

    Ok(va)
}

/// sys_munmap(addr, length) — unmap memory
pub fn sys_munmap(addr: usize, length: usize) -> Result<usize, &'static str> {
    let pid = current_pid()?;
    let pt_root = get_page_table_root(pid).ok_or("no proc pt")?;
    let pages = (length + PAGE_SIZE - 1) >> 12;

    for i in 0..pages {
        let va = addr + i * PAGE_SIZE;
        unsafe {
            sv39::unmap_user_page(pt_root, va);
        }
    }

    Ok(0)
}

/// sys_mprotect(addr, length, prot) — change page protection
pub fn sys_mprotect(_addr: usize, _length: usize, _prot: usize) -> Result<usize, &'static str> {
    Ok(0)
}
