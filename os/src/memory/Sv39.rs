//! Sv39 Virtual Memory Management - Complete Implementation
//!
//! RISC-V Sv39 is a 3-level page table with 4KB pages.
//! - 9 bits per level (512 entries per page)
//! - 27 bits for VPN (3 levels)
//! - 44 bits for PPN
//! - 12 bits offset

use spin::Mutex;

/// Number of page table pages to reserve for the page table allocator
/// Each page table is 4KB, so 64 pages = 256KB for page tables
/// This should be enough for all page tables in the system
const PAGE_TABLE_POOL_SIZE: usize = 64;

/// Page table allocator - pre-allocated pool for page table frames
/// This solves the "chicken-and-egg" problem where we need page tables
/// to map memory, but need memory to create page tables.
/// By pre-allocating a pool in identity-mapped memory, we ensure
/// all page table allocations are always accessible.
static PAGE_TABLE_ALLOCATOR: Mutex<Option<PageTablePool>> = Mutex::new(None);

/// Page table pool - a simple bump allocator for page table frames
struct PageTablePool {
    /// Array of pre-allocated page table frame PAs
    frames: [usize; PAGE_TABLE_POOL_SIZE],
    /// Next free index (bump pointer)
    next_free: usize,
    /// Number of allocated frames
    allocated: usize,
    /// Track which frames have been allocated
    allocated_frames: [bool; PAGE_TABLE_POOL_SIZE],
}

impl PageTablePool {
    /// Create a new page table pool (const-compatible)
    const fn new() -> Self {
        Self {
            frames: [0; PAGE_TABLE_POOL_SIZE],
            next_free: 0,
            allocated: 0,
            allocated_frames: [false; PAGE_TABLE_POOL_SIZE],
        }
    }

    /// Initialize the pool with pre-allocated page table frames
    /// Must be called during early boot before any page tables are created.
    /// The frames must be in identity-mapped memory!
    fn init(&mut self, base_pa: usize, count: usize) {
        assert!(count <= PAGE_TABLE_POOL_SIZE, "Too many page table frames requested");
        for i in 0..count {
            self.frames[i] = base_pa + i * PAGE_SIZE;
            self.allocated_frames[i] = false;
        }
        self.next_free = count;
        self.allocated = 0;
    }

    /// Allocate a page table frame (returns PA)
    fn alloc(&mut self) -> Option<usize> {
        // Find next unallocated frame
        for i in 0..self.next_free {
            if !self.allocated_frames[i] {
                let frame = self.frames[i];
                self.allocated_frames[i] = true;
                return Some(frame);
            }
        }
        None
    }

    /// Check if the pool is initialized
    fn is_initialized(&self) -> bool {
        self.next_free > 0
    }
}

/// Initialize the page table allocator with a pre-allocated pool
/// This should be called during memory::init() before any page tables are created
pub fn init_page_table_allocator_with_pool(base_pa: usize, count: usize) {
    let mut pool = PAGE_TABLE_ALLOCATOR.lock();
    if let Some(ref mut p) = *pool {
        p.init(base_pa, count);
    } else {
        // Create new pool and initialize
        let mut new_pool = PageTablePool::new();
        new_pool.init(base_pa, count);
        *pool = Some(new_pool);
    }
}

/// Page size (4KB)
pub const PAGE_SIZE: usize = 4096;
/// Entries per page table
pub const PTE_COUNT: usize = 512;
/// Bits per level
pub const BITS_PER_LEVEL: usize = 9;
/// Mask for one level
pub const LEVEL_MASK: usize = 0x1FF;

/// Virtual Page Number
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VPN(pub usize);

/// Physical Page Number
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PPN(pub usize);

impl PPN {
    /// Create PPN from physical address
    pub fn from_pa(pa: usize) -> Self {
        Self(pa >> 12)
    }

    /// Convert to physical address
    pub fn to_pa(&self) -> usize {
        self.0 << 12
    }
}

/// Page table entry flags
#[derive(Debug, Clone, Copy)]
pub struct PTEFlags {
    pub valid: bool,       // V - Valid
    pub read: bool,       // R - Readable
    pub write: bool,      // W - Writable
    pub execute: bool,    // X - Executable
    pub user: bool,       // U - User accessible
    pub global: bool,     // G - Global mapping
    pub accessed: bool,   // A - Accessed
    pub dirty: bool,      // D - Dirty
}

impl PTEFlags {
    pub fn new() -> Self {
        Self {
            valid: false,
            read: false,
            write: false,
            execute: false,
            user: false,
            global: false,
            accessed: false,
            dirty: false,
        }
    }

    /// Create flags for kernel readable
    pub fn kernel_r() -> Self {
        Self {
            valid: true, read: true, write: false, execute: false,
            user: false, global: true, accessed: false, dirty: false,
        }
    }

    /// Create flags for kernel readable + writable
    pub fn kernel_rw() -> Self {
        Self {
            valid: true, read: true, write: true, execute: false,
            user: false, global: true, accessed: false, dirty: false,
        }
    }

    /// Create flags for user readable
    pub fn user_r() -> Self {
        Self {
            valid: true, read: true, write: false, execute: false,
            user: true, global: false, accessed: false, dirty: false,
        }
    }

    /// Create flags for user readable + writable (COW)
    pub fn user_cow() -> Self {
        Self {
            valid: true, read: true, write: false, execute: false,
            user: true, global: false, accessed: false, dirty: false,
        }
    }

    /// Create flags for user readable + writable
    pub fn user_rw() -> Self {
        Self {
            valid: true, read: true, write: true, execute: false,
            user: true, global: false, accessed: false, dirty: false,
        }
    }

    /// Create flags for user executable
    pub fn user_rx() -> Self {
        Self {
            valid: true, read: true, write: false, execute: true,
            user: true, global: false, accessed: false, dirty: false,
        }
    }

    pub fn bits(&self) -> usize {
        let mut bits = 0usize;
        if self.valid    { bits |= 1 << 0; }
        if self.read     { bits |= 1 << 1; }
        if self.write    { bits |= 1 << 2; }
        if self.execute  { bits |= 1 << 3; }
        if self.user     { bits |= 1 << 4; }
        if self.global   { bits |= 1 << 5; }
        if self.accessed { bits |= 1 << 6; }
        if self.dirty    { bits |= 1 << 7; }
        bits
    }

    pub fn from_bits(bits: usize) -> Self {
        Self {
            valid:    (bits & (1 << 0)) != 0,
            read:     (bits & (1 << 1)) != 0,
            write:    (bits & (1 << 2)) != 0,
            execute:  (bits & (1 << 3)) != 0,
            user:     (bits & (1 << 4)) != 0,
            global:   (bits & (1 << 5)) != 0,
            accessed: (bits & (1 << 6)) != 0,
            dirty:    (bits & (1 << 7)) != 0,
        }
    }

    /// Check if this is a leaf PTE (has page mapping)
    pub fn is_leaf(&self) -> bool {
        self.read || self.execute
    }

    /// Check if this is a valid leaf PTE
    pub fn is_valid_leaf(&self) -> bool {
        self.valid && self.is_leaf()
    }
}

/// Page Table Entry
#[derive(Debug, Clone, Copy)]
pub struct PageTableEntry {
    pub ppn: PPN,
    pub flags: PTEFlags,
}

impl PageTableEntry {
    pub fn new() -> Self {
        Self {
            ppn: PPN(0),
            flags: PTEFlags::new(),
        }
    }

    pub fn bits(&self) -> usize {
        self.ppn.0 << 10 | self.flags.bits()
    }

    pub fn from_bits(bits: usize) -> Self {
        Self {
            ppn: PPN(bits >> 10),
            flags: PTEFlags::from_bits(bits & 0xFF),
        }
    }

    /// Create a PTE pointing to a physical page
    pub fn new_page(ppn: PPN, flags: PTEFlags) -> Self {
        Self { ppn, flags }
    }

    /// Check if this PTE is valid
    pub fn is_valid(&self) -> bool {
        self.flags.valid
    }

    /// Check if this is a leaf PTE (points to a page)
    pub fn is_leaf(&self) -> bool {
        self.flags.is_leaf()
    }
}

/// Virtual address for Sv39
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtAddr(pub usize);

/// Physical address
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(pub usize);

impl VirtAddr {
    pub fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub fn page_offset(&self) -> usize {
        self.0 & 0xFFF
    }

    pub fn vpn(&self) -> VPN {
        VPN(self.0 >> 12)
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    /// Check if address is in user space
    pub fn is_user(&self) -> bool {
        self.0 < 0xFFFF_FFFF_C000_0000
    }

    /// Check if address is in kernel space
    pub fn is_kernel(&self) -> bool {
        !self.is_user()
    }
}

impl PhysAddr {
    pub fn new(addr: usize) -> Self {
        Self(addr)
    }

    pub fn as_usize(&self) -> usize {
        self.0
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    pub fn page_offset(&self) -> usize {
        self.0 & 0xFFF
    }

    pub fn ppn(&self) -> PPN {
        PPN(self.0 >> 12)
    }
}

/// Extract VPN indices from a VPN
pub fn vpn_indices(vpn: VPN) -> [usize; 3] {
    let vpn_bits = vpn.0;
    [
        (vpn_bits >> 18) & LEVEL_MASK,  // Level 0 (root)
        (vpn_bits >> 9) & LEVEL_MASK,    // Level 1
        vpn_bits & LEVEL_MASK,            // Level 2 (leaf)
    ]
}

/// Virtual address indices
pub fn va_indices(va: VirtAddr) -> [usize; 3] {
    vpn_indices(va.vpn())
}

/// Page table for Sv39 - represents one level
/// This is 4096 bytes (512 8-byte entries)
pub const PAGE_TABLE_SIZE: usize = PAGE_SIZE;

/// Sv39 Page Table
/// A page table is stored in a 4KB page and contains 512 PTE entries
#[repr(C)]
pub struct PageTable {
    pub entries: [PageTableEntry; PTE_COUNT],
}

impl PageTable {
    /// Create a new empty page table (all invalid)
    pub fn new() -> Option<&'static mut Self> {
        // First try the dedicated page table allocator
        let page = {
            let mut pool = PAGE_TABLE_ALLOCATOR.lock();
            if let Some(ref mut p) = *pool {
                p.alloc()
            } else {
                None
            }
        };

        // Fall back to general allocator if pool not initialized or empty
        let page = page.or_else(|| crate::memory::allocator::alloc_page())?;

        let ptr = page as *mut PageTable;
        unsafe {
            ptr.write_bytes(0, 1);
        }
        Some(unsafe { &mut *ptr })
    }

    /// Get entry at index
    pub fn get_entry(&self, index: usize) -> Option<&PageTableEntry> {
        if index < PTE_COUNT {
            Some(&self.entries[index])
        } else {
            None
        }
    }

    /// Get mutable entry at index
    pub fn get_entry_mut(&mut self, index: usize) -> Option<&mut PageTableEntry> {
        if index < PTE_COUNT {
            Some(&mut self.entries[index])
        } else {
            None
        }
    }

    /// Set entry at index
    pub fn set_entry(&mut self, index: usize, pte: PageTableEntry) {
        if index < PTE_COUNT {
            self.entries[index] = pte;
        }
    }

    /// Clear entry at index
    pub fn clear_entry(&mut self, index: usize) {
        if index < PTE_COUNT {
            self.entries[index] = PageTableEntry::new();
        }
    }
}

/// Address translation result
#[derive(Debug)]
pub struct TranslateResult {
    pub pa: PhysAddr,
    pub flags: PTEFlags,
}

/// Page table manager - manages the entire Sv39 page table structure
pub struct PageTableManager {
    /// Root page table physical address (for SATP)
    root_ppn: PPN,
}

impl PageTableManager {
    /// Create a new page table manager with a fresh root page table
    pub fn new() -> Self {
        let root_pt = PageTable::new().expect("Failed to allocate root page table");
        let pa = root_pt as *const PageTable as usize;
        let ppn = PhysAddr(pa).ppn();
        Self {
            root_ppn: ppn,
        }
    }

    /// Create a page table manager wrapping an existing root PPN
    /// This is used to take over the page table from the bootloader
    pub fn from_existing_root(root_ppn_val: usize) -> Self {
        Self {
            root_ppn: PPN(root_ppn_val),
        }
    }

    /// Get root PPN for SATP register
    pub fn root_ppn(&self) -> PPN {
        self.root_ppn
    }

    /// Map a virtual page to a physical page with given flags
    pub fn map(&mut self, va: VirtAddr, pa: PhysAddr, flags: PTEFlags) -> Result<(), MapError> {
        let indices = va_indices(va);
        let ppn = pa.ppn();

        // Walk down the page table, creating levels as needed
        let root_ptr = self.root_ppn.to_pa() as *mut PageTable;

        // Level 0
        unsafe {
            let root = &mut *root_ptr;
            let pte = root.get_entry_mut(indices[0])
                .ok_or(MapError::InvalidAddress)?;

            if !pte.is_valid() {
                // Create level 1 page table
                let next_pt = PageTable::new().ok_or(MapError::NoMemory)?;
                let next_pa = next_pt as *const PageTable as usize;
                let next_ppn = PhysAddr(next_pa).ppn();
                pte.ppn = next_ppn;
                pte.flags.valid = true;
                pte.flags.read = true;
            }

            let level1_pa = pte.ppn.to_pa();
            let level1_ptr = level1_pa as *mut PageTable;
            let level1 = &mut *level1_ptr;
            let pte1 = level1.get_entry_mut(indices[1])
                .ok_or(MapError::InvalidAddress)?;

            if !pte1.is_valid() {
                // Create level 2 page table
                let next_pt = PageTable::new().ok_or(MapError::NoMemory)?;
                let next_pa = next_pt as *const PageTable as usize;
                let next_ppn = PhysAddr(next_pa).ppn();
                pte1.ppn = next_ppn;
                pte1.flags.valid = true;
                pte1.flags.read = true;
            }

            let level2_pa = pte1.ppn.to_pa();
            let level2_ptr = level2_pa as *mut PageTable;
            let level2 = &mut *level2_ptr;
            let pte2 = level2.get_entry_mut(indices[2])
                .ok_or(MapError::InvalidAddress)?;

            if pte2.is_valid() {
                return Err(MapError::AlreadyMapped);
            }

            pte2.ppn = ppn;
            pte2.flags = flags;
        }

        Ok(())
    }

    /// Unmap a virtual address
    pub fn unmap(&mut self, va: VirtAddr) -> Result<(), MapError> {
        let indices = va_indices(va);

        let root_ptr = self.root_ppn.to_pa() as *mut PageTable;

        unsafe {
            let root = &mut *root_ptr;
            let pte0 = root.get_entry(indices[0]).ok_or(MapError::NotMapped)?;
            if !pte0.is_valid() {
                return Err(MapError::NotMapped);
            }

            let level1_ptr = pte0.ppn.to_pa() as *mut PageTable;
            let level1 = &mut *level1_ptr;
            let pte1 = level1.get_entry(indices[1]).ok_or(MapError::NotMapped)?;
            if !pte1.is_valid() {
                return Err(MapError::NotMapped);
            }

            let level2_ptr = pte1.ppn.to_pa() as *mut PageTable;
            let level2 = &mut *level2_ptr;
            let pte2 = level2.get_entry_mut(indices[2]).ok_or(MapError::NotMapped)?;

            if !pte2.is_valid() {
                return Err(MapError::NotMapped);
            }

            // Clear the PTE
            pte2.flags.valid = false;
            pte2.ppn = PPN(0);
        }

        Ok(())
    }

    /// Translate a virtual address to physical address
    pub fn translate(&self, va: VirtAddr) -> Option<TranslateResult> {
        let indices = va_indices(va);
        let root = self.get_root();

        // Walk the page table
        let mut pte = root.get_entry(indices[0])?;

        for level in 0..3 {
            if !pte.is_valid() {
                return None;
            }

            if level < 2 {
                // Continue walking
                let next_pt = self.walk_pte_readonly(pte, level)?;
                pte = next_pt.get_entry(indices[level + 1])?;
            } else {
                // Leaf PTE
                if !pte.flags.is_leaf() {
                    return None;
                }
                let offset = va.page_offset();
                let pa = PhysAddr(pte.ppn.to_pa() | offset);
                return Some(TranslateResult {
                    pa,
                    flags: pte.flags,
                });
            }
        }

        None
    }

    /// Check if an address is COW (Copy-on-Write)
    pub fn is_cow(&self, va: VirtAddr) -> bool {
        if let Some(result) = self.translate(va) {
            // COW pages are marked R=1, W=0
            result.flags.read && !result.flags.write
        } else {
            false
        }
    }

    /// Make a page writable (break COW)
    pub fn make_writable(&mut self, va: VirtAddr) -> Result<(), MapError> {
        let indices = va_indices(va);
        let root = self.get_root_mut();

        // Walk to leaf PTE
        let pte = self.walk_to_leaf_mut(root, indices).ok_or(MapError::NotMapped)?;

        if !pte.is_valid() {
            return Err(MapError::NotMapped);
        }

        // Break COW - set writable
        pte.flags.write = true;

        Ok(())
    }

    /// Get root page table (read-only)
    fn get_root(&self) -> &'static PageTable {
        let ptr = self.root_ppn.to_pa() as *const PageTable;
        unsafe { &*ptr }
    }

    /// Get root page table (mutable)
    fn get_root_mut(&mut self) -> &'static mut PageTable {
        let ptr = self.root_ppn.to_pa() as *mut PageTable;
        unsafe { &mut *ptr }
    }

    /// Walk to a PTE at the given level (read-only)
    fn walk_pte_readonly<'a>(&self, parent: &'a PageTableEntry, _level: usize) -> Option<&'a PageTable> {
        if parent.is_valid() && !parent.is_leaf() {
            let ptr = parent.ppn.to_pa() as *const PageTable;
            Some(unsafe { &*ptr })
        } else {
            None
        }
    }

    /// Walk to a PTE at the given level
    fn walk_pte<'a>(&self, parent: &'a PageTableEntry, _level: usize) -> Option<&'a mut PageTable> {
        if parent.is_valid() && !parent.is_leaf() {
            let ptr = parent.ppn.to_pa() as *mut PageTable;
            Some(unsafe { &mut *ptr })
        } else {
            None
        }
    }

    /// Walk to leaf PTE
    fn walk_to_leaf<'a>(&self, root: &'a PageTable, indices: [usize; 3]) -> Option<&'a PageTableEntry> {
        let mut pte = root.get_entry(indices[0])?;

        for level in 0..3 {
            if !pte.is_valid() {
                return None;
            }

            if level < 2 {
                let next_pt = self.walk_pte_readonly(pte, level)?;
                pte = next_pt.get_entry(indices[level + 1])?;
            }
        }

        Some(pte)
    }

    /// Walk to leaf PTE (mutable)
    fn walk_to_leaf_mut<'a>(&self, root: &'a mut PageTable, indices: [usize; 3]) -> Option<&'a mut PageTableEntry> {
        // Simplified: just return the leaf
        let ptr = root as *mut PageTable as usize;
        let mut ptr_ref = ptr;

        for level in 0..3 {
            let pt = unsafe { &*((ptr_ref as *const PageTable)) };
            let pte = pt.get_entry(indices[level])?;
            if !pte.is_valid() {
                return None;
            }
            if level < 2 {
                ptr_ref = pte.ppn.to_pa();
            }
        }

        unsafe {
            let pt = &mut *(ptr_ref as *mut PageTable);
            pt.get_entry_mut(indices[2])
        }
    }
}

impl Default for PageTableManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Map error types
#[derive(Debug)]
pub enum MapError {
    AlreadyMapped,
    NotMapped,
    InvalidAddress,
    NoMemory,
}

/// Kernel page table instance
static KERNEL_PAGE_TABLE: Mutex<Option<PageTableManager>> = Mutex::new(None);

/// Initialize the kernel page table
/// We create a new page table and add identity mappings for the kernel.
pub fn init_kernel_page_table() {
    // Read the current SATP
    let satp: usize;
    unsafe {
        core::arch::asm!("csrr {0}, satp", out(reg) satp);
    }

    // Create a new page table
    let root_pt = PageTable::new();
    if root_pt.is_none() {
        crate::println!("[vm] ERROR: Failed to allocate root page table");
        return;
    }

    let root_pt = root_pt.unwrap();
    let pa = root_pt as *const PageTable as usize;
    let ppn = PhysAddr(pa).ppn();

    let mut pt_manager = PageTableManager::from_existing_root(ppn.0);

    // Map the entire low memory region including page tables and kernel
    // PT pool is at 0x80080000-0x80090000 (256KB = 64 pages)
    // Kernel is at 0x80200000-0x80400000
    // We need to map from 0x80000000 to 0x80090000 (9MB) to cover everything
    let region_base = 0x80000000usize;
    let region_size = 0x00900000usize; // 9 MB - covers PT pool (0x80080000-0x80090000) and kernel

    // Map each page
    let pages = region_size / PAGE_SIZE;
    for i in 0..pages {
        let va = VirtAddr::new(region_base + i * PAGE_SIZE);
        let pa = PhysAddr::new(region_base + i * PAGE_SIZE);
        if pt_manager.map(va, pa, PTEFlags::kernel_rw()).is_err() {
            break;
        }
    }

    *KERNEL_PAGE_TABLE.lock() = Some(pt_manager);
}

/// Get kernel page table manager
pub fn get_kernel_pt() -> &'static Mutex<Option<PageTableManager>> {
    &KERNEL_PAGE_TABLE
}

/// Translate a kernel virtual address to physical
pub fn translate_va(va: VirtAddr) -> Option<PhysAddr> {
    let pt = KERNEL_PAGE_TABLE.lock();
    if let Some(ref pt_manager) = *pt {
        pt_manager.translate(va).map(|r| r.pa)
    } else {
        None
    }
}

/// Map a kernel virtual address to physical
pub fn map_kernel(va: VirtAddr, pa: PhysAddr, flags: PTEFlags) -> Result<(), MapError> {
    let mut pt = KERNEL_PAGE_TABLE.lock();
    if let Some(ref mut pt_manager) = *pt {
        pt_manager.map(va, pa, flags)
    } else {
        Err(MapError::InvalidAddress)
    }
}

/// Enable Sv39 virtual memory by setting SATP
pub fn enable_sv39() {
    let pt = KERNEL_PAGE_TABLE.lock();
    if let Some(ref _pt_manager) = *pt {
        let root_ppn = _pt_manager.root_ppn().0;
        // SATP format: MODE (8) | PPN[43:0]
        let satp = 8usize << 60 | root_ppn;

        // Debug output before SATP write
        unsafe {
            let s = "[vm] enabling sv39\n";
            let mut ptr = s.as_ptr() as usize;
            let mut len = s.len();
            core::arch::asm!(
                "1: lbu a0, 0(a1)",
                "   li a7, 1",
                "   ecall",
                "   addi a1, a1, 1",
                "   addi a2, a2, -1",
                "   bnez a2, 1b",
                inout("a1") ptr, inout("a2") len
            );

            core::arch::asm!("csrw satp, {0}", in(reg) satp);

            let s2 = "[vm] sv39 enabled\n";
            let mut ptr2 = s2.as_ptr() as usize;
            let mut len2 = s2.len();
            core::arch::asm!(
                "1: lbu a0, 0(a1)",
                "   li a7, 1",
                "   ecall",
                "   addi a1, a1, 1",
                "   addi a2, a2, -1",
                "   bnez a2, 1b",
                inout("a1") ptr2, inout("a2") len2
            );

            core::arch::asm!("sfence.vma zero, zero");
        }
    }
}

// ============================================
// COW (Copy-on-Write) Page Handling
// ============================================

/// Handle a COW page fault
/// Returns true if the page was successfully handled
pub fn handle_cow_page(fault_addr: usize) -> bool {
    let va = VirtAddr::new(fault_addr & !0xFFF);  // Page-aligned

    let mut pt = KERNEL_PAGE_TABLE.lock();
    if let Some(ref mut pt_manager) = *pt {
        // Check if this is a COW page
        if pt_manager.is_cow(va) {
            // This is a COW page - need to copy it
            if let Some(result) = pt_manager.translate(va) {
                let old_pa = result.pa;

                // Allocate a new physical page
                if let Some(new_pa) = crate::memory::allocator::alloc_page() {
                    // Copy the content
                    unsafe {
                        let src = old_pa.as_ptr::<u8>();
                        let dst = new_pa as *mut u8;
                        core::ptr::copy_nonoverlapping(src, dst, PAGE_SIZE);
                    }

                    // Unmap the old COW page
                    if pt_manager.unmap(va).is_ok() {
                        // Map the new page as writable
                        if pt_manager.map(va, PhysAddr::new(new_pa), PTEFlags::user_rw()).is_ok() {
                            crate::println!("[vm] COW page broken");
                            return true;
                        }
                    }
                }
            }
        } else {
            // Not a COW page - might be a demand page
            // For now, try to allocate and zero a new page
            if let Some(new_pa) = crate::memory::allocator::alloc_page() {
                unsafe {
                    // Zero the new page
                    let dst = new_pa as *mut u8;
                    core::ptr::write_bytes(dst, 0, PAGE_SIZE);
                }

                if pt_manager.unmap(va).is_ok() {
                    if pt_manager.map(va, PhysAddr::new(new_pa), PTEFlags::user_rw()).is_ok() {
                        crate::println!("[vm] Demand page allocated");
                        return true;
                    }
                }
            }
        }
    }

    false
}

// ============================================
// User Address Space Management
// ============================================

/// User address space configuration
pub struct UserAddressSpace {
    /// Page table manager for this address space
    pub pt_manager: PageTableManager,
    /// SATP value for this address space
    pub satp: usize,
    /// User heap start
    pub heap_start: usize,
    /// User heap end (current brk)
    pub heap_end: usize,
    /// User stack base
    pub stack_base: usize,
    /// User stack size
    pub stack_size: usize,
}

impl UserAddressSpace {
    /// Create a new user address space
    /// Creates a fresh page table
    pub fn new() -> Option<Self> {
        let pt_manager = PageTableManager::new();
        let satp = 8usize << 60 | pt_manager.root_ppn().0;

        Some(Self {
            pt_manager,
            satp,
            heap_start: 0x00400000 + 0x100000,
            heap_end: 0x00400000 + 0x100000,
            stack_base: 0x3FFFFFFFE80,
            stack_size: 0x200000,
        })
    }

    /// Map a user page with COW (Copy-on-Write) semantics
    /// The page is readable but not writable - writes trigger COW fault
    pub fn map_user_cow(&mut self, va: VirtAddr, pa: PhysAddr) -> Result<(), MapError> {
        self.pt_manager.map(va, pa, PTEFlags::user_cow())
    }

    /// Map a user page as writable (used after COW break)
    pub fn map_user_writable(&mut self, va: VirtAddr, pa: PhysAddr) -> Result<(), MapError> {
        self.pt_manager.map(va, pa, PTEFlags::user_rw())
    }

    /// Map a user page as readable+executable (for code)
    pub fn map_user_rx(&mut self, va: VirtAddr, pa: PhysAddr) -> Result<(), MapError> {
        self.pt_manager.map(va, pa, PTEFlags::user_rx())
    }

    /// Allocate and map a user page
    /// Returns the virtual address of the allocated page
    pub fn alloc_user_page(&mut self, va: VirtAddr) -> Result<PhysAddr, MapError> {
        let pa = crate::memory::allocator::alloc_page()
            .ok_or(MapError::NoMemory)?;

        // Zero the page
        unsafe {
            core::ptr::write_bytes(pa as *mut u8, 0, PAGE_SIZE);
        }

        self.map_user_cow(va, PhysAddr::new(pa))?;
        Ok(PhysAddr::new(pa))
    }

    /// Set up the initial user stack
    pub fn setup_user_stack(&mut self) -> Result<usize, MapError> {
        let stack_top = self.stack_base + self.stack_size;

        // Allocate and map stack pages (growing down from stack_top)
        let pages = self.stack_size / PAGE_SIZE;
        for i in 0..pages {
            let va = VirtAddr::new(stack_top - (i + 1) * PAGE_SIZE);
            let pa = crate::memory::allocator::alloc_page()
                .ok_or(MapError::NoMemory)?;

            // Zero the page
            unsafe {
                core::ptr::write_bytes(pa as *mut u8, 0, PAGE_SIZE);
            }

            self.map_user_writable(va, PhysAddr::new(pa))?;
        }

        Ok(stack_top)
    }

    /// Set up the initial user stack at a high VA base
    /// This version allows specifying a high VA base to avoid conflicts
    pub fn setup_user_stack_high_va(&mut self, high_va_base: usize) -> Result<usize, MapError> {
        // Use high_va_base for stack - this should be a kernel VA (bits 63:39 = 1)
        // Stack grows downward from stack_top
        let stack_size = 0x40000; // 256KB stack (64 pages)
        let stack_base = high_va_base;  // Use the provided high VA base
        let stack_top = stack_base + stack_size;

        crate::print!("[stack] setup\r\n");

        // Allocate and map stack pages (growing down from stack_top)
        let pages = stack_size / PAGE_SIZE;
        for i in 0..pages {
            let va = VirtAddr::new(stack_top - (i + 1) * PAGE_SIZE);

            let pa = crate::memory::allocator::alloc_page()
                .ok_or(MapError::NoMemory)?;

            // Zero the page
            unsafe {
                core::ptr::write_bytes(pa as *mut u8, 0, PAGE_SIZE);
            }

            self.map_user_writable(va, PhysAddr::new(pa))?;
        }

        crate::print!("[stack] done\r\n");
        Ok(stack_top)
    }

    /// Get the SATP value for this address space
    pub fn get_satp(&self) -> usize {
        self.satp
    }
}

/// Copy a user address space for fork (COW semantics)
/// Both parent and child share the same physical pages initially,
/// but writes will trigger a page fault and page copy.
/// Returns the new page table manager and SATP value
pub fn copy_user_address_space(parent_pt: &PageTableManager) -> Option<(PageTableManager, usize)> {
    let mut new_pt = PageTableManager::new();

    // Walk the parent's page table and copy user mappings with COW semantics
    // We need to find all valid user pages and create COW mappings

    // Get kernel page table to access the parent's root
    // For now, we'll copy from the current kernel page table
    // In a proper implementation, the parent PT would be passed explicitly

    // For simplicity, we'll identity-map a fixed range for user space
    // This allows fork to work until we have proper page table walking
    const USER_BASE: usize = 0x00400000;
    const USER_SIZE: usize = 0x100000; // 1MB for now

    // Map user pages as COW - they start read-only and will be copied on write
    for page_addr in (USER_BASE..USER_BASE + USER_SIZE).step_by(PAGE_SIZE) {
        let va = VirtAddr::new(page_addr);
        let pa = PhysAddr::new(page_addr); // Identity mapping for now

        // Map as COW (read-only initially)
        if new_pt.map(va, pa, PTEFlags::user_cow()).is_err() {
            crate::print!("[sv39] COW fork: failed to map page\r\n");
            break;
        }
    }

    let satp = 8usize << 60 | new_pt.root_ppn().0;
    Some((new_pt, satp))
}

/// Copy user address space from a specific parent page table root
/// This version takes the parent's root PPN directly
pub fn copy_user_address_space_from_root(parent_root_ppn: usize) -> Option<(PageTableManager, usize)> {
    let mut new_pt = PageTableManager::new();

    // Get pointers to both page tables
    let parent_root = parent_root_ppn as *const PageTable;
    let parent = unsafe { &*parent_root };

    // Walk parent's page table and copy user mappings as COW
    // Level 0 (root)
    for i in 0..PTE_COUNT {
        let pte = parent.get_entry(i)?;
        if !pte.is_valid() || pte.is_leaf() {
            continue; // Skip invalid or non-leaf entries
        }

        // Level 1
        let level1 = unsafe { &*((pte.ppn.to_pa()) as *const PageTable) };
        for j in 0..PTE_COUNT {
            let pte1 = level1.get_entry(j)?;
            if !pte1.is_valid() || pte1.is_leaf() {
                continue;
            }

            // Level 2 (leaf)
            let level2 = unsafe { &*((pte1.ppn.to_pa()) as *const PageTable) };
            for k in 0..PTE_COUNT {
                let pte2 = level2.get_entry(k)?;
                if !pte2.is_valid() || !pte2.is_leaf() {
                    continue;
                }

                // This is a valid user page - copy it as COW
                // VPN = (i << 18) | (j << 9) | k, VA = VPN << 12
                let vpn = (i << 18) | (j << 9) | k;
                let va = VirtAddr::new(vpn << 12);
                let pa = PhysAddr::new(pte2.ppn.to_pa());

                // Only copy user pages (U bit set)
                if pte2.flags.user {
                    // Map as COW (read-only) in the new page table
                    if new_pt.map(va, pa, PTEFlags::user_cow()).is_err() {
                        crate::print!("[sv39] COW fork: failed to copy page\r\n");
                    }
                }
            }
        }
    }

    let satp = 8usize << 60 | new_pt.root_ppn().0;
    Some((new_pt, satp))
}

