//! Syscall memory management
//!
//! Implements memory-related syscalls

use spin::Mutex;

/// Memory mapping flags
pub const MAP_SHARED: usize = 0x01;
pub const MAP_PRIVATE: usize = 0x02;
pub const MAP_FIXED: usize = 0x10;
pub const MAP_ANONYMOUS: usize = 0x20;
pub const MAP_GROWSDOWN: usize = 0x0100;
pub const MAP_DENYWRITE: usize = 0x0800;
pub const MAP_EXECUTABLE: usize = 0x1000;
pub const MAP_LOCKED: usize = 0x2000;
pub const MAP_NORESERVE: usize = 0x4000;

/// Memory protection flags
pub const PROT_READ: usize = 0x1;
pub const PROT_WRITE: usize = 0x2;
pub const PROT_EXEC: usize = 0x4;
pub const PROT_NONE: usize = 0x0;

/// Brk heap management
static HEAP_END: Mutex<usize> = Mutex::new(0x80400000);

/// Initial brk value (start of heap)
pub const INITIAL_BRK: usize = 0x80400000;

/// Memory-mapped region tracking
static MAPPED_REGIONS: Mutex<[Option<MappedRegion>; 64]> = Mutex::new([None; 64]);

#[derive(Debug, Clone, Copy)]
pub struct MappedRegion {
    pub addr: usize,
    pub len: usize,
    pub prot: usize,
    pub flags: usize,
}

impl MappedRegion {
    pub fn new(addr: usize, len: usize, prot: usize, flags: usize) -> Self {
        Self { addr, len, prot, flags }
    }
}

/// Mmap syscall
pub fn sys_mmap(addr: usize, len: usize, prot: usize, flags: usize, fd: usize, _offset: usize) -> isize {
    // Validate length
    if len == 0 {
        return -1;
    }

    // Handle anonymous mappings (no file descriptor)
    if flags & MAP_ANONYMOUS != 0 {
        return sys_mmap_anon(addr, len, prot, flags);
    }

    // For non-anonymous mappings, we don't support files yet
    crate::println!("[mmap] Non-anonymous mmap not supported");
    -1
}

/// Anonymous mmap
fn sys_mmap_anon(addr: usize, len: usize, prot: usize, flags: usize) -> isize {
    // Align length to page size
    let page_size = 4096;
    let aligned_len = ((len + page_size - 1) / page_size) * page_size;

    // Determine actual address
    let actual_addr = if addr == 0 || flags & MAP_FIXED != 0 {
        // Let kernel choose or use specified address
        if addr == 0 {
            allocate_virtual_pages(aligned_len)
        } else {
            addr
        }
    } else {
        allocate_virtual_pages(aligned_len)
    };

    if actual_addr == 0 {
        crate::println!("[mmap] Failed to allocate memory");
        return -1;
    }

    // Track the mapped region (simplified - just skip for now)
    let _region = MappedRegion::new(actual_addr, aligned_len, prot, flags);

    crate::println!("[mmap] Allocated anon memory");
    actual_addr as isize
}

/// Allocate virtual pages (simplified - just return physical addresses for now)
/// In a real OS, this would manage virtual address space
fn allocate_virtual_pages(len: usize) -> usize {
    let mut heap_end = HEAP_END.lock();
    let addr = *heap_end;
    *heap_end += len;
    addr
}

/// Find an empty slot for tracking mapped regions
fn find_empty_slot() -> Option<&'static Mutex<[Option<MappedRegion>; 64]>> {
    None  // Simplified - tracking not fully implemented
}

/// Munmap syscall
pub fn sys_munmap(addr: usize, len: usize) -> isize {
    if addr == 0 {
        return 0;  //munmap(0, 0) is valid no-op
    }

    crate::println!("[munmap] Called");

    // In a real implementation, we would:
    // 1. Find the mapped region
    // 2. Unmap the pages from the page table
    // 3. Release physical memory if not shared

    0
}

/// Mprotect syscall
pub fn sys_mprotect(addr: usize, len: usize, prot: usize) -> isize {
    crate::println!("[mprotect] Called");

    // In a real implementation, this would modify the page table entries
    // to change the protection bits

    0
}

/// Brk syscall - change data segment size
pub fn sys_brk(addr: usize) -> isize {
    let mut heap_end = HEAP_END.lock();

    if addr == 0 {
        // brk(0) returns the current break value
        return *heap_end as isize;
    }

    if addr < INITIAL_BRK {
        // Cannot shrink below initial brk
        return INITIAL_BRK as isize;
    }

    // Simple implementation: just set the break
    crate::println!("[brk] Setting heap end");
    *heap_end = addr;

    addr as isize
}

/// Get current heap end
pub fn get_heap_end() -> usize {
    *HEAP_END.lock()
}
