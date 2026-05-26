// V35: Memory Sealing (mseal) — prevent future mprotect/munmap/mmap on sealed ranges
//
// Once a virtual address range is sealed, the kernel rejects any attempt to
// change its protections (mprotect), unmap it (munmap), or re-map over it
// (mmap with MAP_FIXED).  This is the TrainOS equivalent of Linux's
// mseal() system call (Linux 6.10+, 2024).

const MAX_SEALS: usize = 64;

#[derive(Clone, Copy)]
struct MemSeal {
    pid: u32,
    start: usize,
    end: usize,
}

// Global seal table.  Protected by the fact that kernel code runs with
// interrupts disabled during syscall handling (single logical thread
// within the kernel at any given time on this hart).
static mut SEALS: [MemSeal; MAX_SEALS] = [MemSeal { pid: 0, start: 0, end: 0 }; MAX_SEALS];
static mut SEAL_COUNT: usize = 0;

/// Seal a virtual address range [start, start+len) for a process.
/// Once sealed, mprotect/munmap/mmap (MAP_FIXED) on this range will fail.
pub fn mseal(pid: u32, start: usize, len: usize) -> Result<(), &'static str> {
    if start & 0xFFF != 0 {
        return Err("mseal: unaligned address");
    }
    if len == 0 {
        return Err("mseal: zero length");
    }
    let end = start + len;
    if end < start {
        return Err("mseal: wrapped end");
    }
    unsafe {
        // Check for overlap with existing seals for this pid
        for i in 0..SEAL_COUNT {
            let s = &SEALS[i];
            if s.pid == pid && s.start < end && s.end > start {
                return Err("mseal: range overlaps existing seal");
            }
        }
        if SEAL_COUNT >= MAX_SEALS {
            return Err("mseal: too many seals (max 64)");
        }
        SEALS[SEAL_COUNT] = MemSeal { pid, start, end };
        SEAL_COUNT += 1;
    }
    Ok(())
}

/// Remove a seal that matches exactly [start, start+len) for the given pid.
pub fn munseal(pid: u32, start: usize, len: usize) -> Result<(), &'static str> {
    if start & 0xFFF != 0 {
        return Err("munseal: unaligned address");
    }
    if len == 0 {
        return Err("munseal: zero length");
    }
    let end = start + len;
    unsafe {
        let mut i = 0;
        while i < SEAL_COUNT {
            if SEALS[i].pid == pid && SEALS[i].start == start && SEALS[i].end == end {
                // Remove by shifting remaining entries left
                for j in i..SEAL_COUNT - 1 {
                    SEALS[j] = SEALS[j + 1];
                }
                SEAL_COUNT -= 1;
                return Ok(());
            }
            i += 1;
        }
    }
    Err("munseal: no matching seal found")
}

/// Check whether any part of [addr, addr+len) is sealed for this pid.
/// Returns `true` if **any** overlapping sealed region is found.
pub fn is_sealed(pid: u32, addr: usize, len: usize) -> bool {
    if len == 0 {
        return false;
    }
    let end = addr.saturating_add(len);
    unsafe {
        for i in 0..SEAL_COUNT {
            if SEALS[i].pid == pid {
                let s = &SEALS[i];
                // Overlap: s.start < end && s.end > addr
                if s.start < end && s.end > addr {
                    return true;
                }
            }
        }
    }
    false
}

/// Count sealed pages for a given process.
pub fn count_sealed(pid: u32) -> usize {
    let mut count = 0usize;
    unsafe {
        for i in 0..SEAL_COUNT {
            if SEALS[i].pid == pid {
                count += (SEALS[i].end - SEALS[i].start) / 4096;
            }
        }
    }
    count
}

/// Total sealed pages across all processes (for /proc/meminfo style stats).
pub fn total_sealed_pages() -> usize {
    let mut count = 0usize;
    unsafe {
        for i in 0..SEAL_COUNT {
            count += (SEALS[i].end - SEALS[i].start) / 4096;
        }
    }
    count
}

/// Number of active seal entries.
pub fn seal_count() -> usize {
    unsafe { SEAL_COUNT }
}

