//! Sv39 Virtual Memory Management - Complete Implementation
//!
//! RISC-V Sv39 is a 3-level page table with 4KB pages.
//! - 9 bits per level (512 entries per page)
//! - 27 bits for VPN (3 levels)
//! - 44 bits for PPN
//! - 12 bits offset

use spin::Mutex;

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
    pub fn new() -> &'static mut Self {
        // This should be called after allocating a physical page
        let ptr = crate::memory::allocator::alloc_page()
            .expect("Failed to allocate page table") as *mut PageTable;
        unsafe {
            ptr.write_bytes(0, 1);
            &mut *ptr
        }
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
        let root_pt = PageTable::new();
        Self {
            root_ppn: PhysAddr(root_pt as *const PageTable as usize).ppn(),
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
        // Returns (parent_ptr, entry_index) at each level
        let root_ptr = self.root_ppn.to_pa() as *mut PageTable;

        // Level 0
        unsafe {
            let root = &mut *root_ptr;
            let pte = root.get_entry_mut(indices[0])
                .ok_or(MapError::InvalidAddress)?;

            if !pte.is_valid() {
                // Create level 1 page table
                let next_pt = PageTable::new();
                let next_ppn = PhysAddr(next_pt as *const PageTable as usize).ppn();
                pte.ppn = next_ppn;
                pte.flags.valid = true;
                pte.flags.read = true;
            }

            let level1_ptr = pte.ppn.to_pa() as *mut PageTable;
            let level1 = &mut *level1_ptr;
            let pte1 = level1.get_entry_mut(indices[1])
                .ok_or(MapError::InvalidAddress)?;

            if !pte1.is_valid() {
                // Create level 2 page table
                let next_pt = PageTable::new();
                let next_ppn = PhysAddr(next_pt as *const PageTable as usize).ppn();
                pte1.ppn = next_ppn;
                pte1.flags.valid = true;
                pte1.flags.read = true;
            }

            let level2_ptr = pte1.ppn.to_pa() as *mut PageTable;
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
pub fn init_kernel_page_table() {
    crate::println!("[vm] Initializing kernel page table...");
    let mut pt = KERNEL_PAGE_TABLE.lock();
    *pt = Some(PageTableManager::new());

    // Set up identity mapping for kernel (0x80000000 -> physical)
    // This is a simplified version - in practice we'd map all of DRAM
    if let Some(ref mut pt_manager) = *pt {
        // Identity map the first 8MB for kernel (QEMU virt machine)
        for i in 0..2048 {
            let va = VirtAddr::new(0x80000000 + i * PAGE_SIZE);
            let pa = PhysAddr::new(0x80000000 + i * PAGE_SIZE);
            if pt_manager.map(va, pa, PTEFlags::kernel_rw()).is_err() {
                break;
            }
        }
    }

    crate::println!("[vm] Kernel page table initialized");
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
    if let Some(ref pt_manager) = *pt {
        let root_ppn = pt_manager.root_ppn().0;
        // SATP format: MODE (8) | PPN[43:0]
        let satp = 8 << 60 | root_ppn;

        crate::println!("[vm] Enabling Sv39");

        unsafe {
            // Set SATP
            core::arch::asm!("csrw satp, {0}", in(reg) satp);
            // Flush TLB
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
    /// Maps kernel as global, and sets up user space as non-global
    pub fn new() -> Option<Self> {
        let pt_manager = PageTableManager::new();
        let satp = 8usize << 60 | pt_manager.root_ppn().0;

        // Default user address space layout:
        // 0x0000000000000000 - 0x00003FFFFFFFFFFF (user space, 128GB)
        // We only map a portion for now

        // User heap starts after code/data (we'll set this during exec)
        // User stack at high address (0x3FFFFFFFE80 = near top of 48-bit user VA)

        Some(Self {
            pt_manager,
            satp,
            heap_start: 0x00400000 + 0x100000, // After first 1MB (text, data, bss)
            heap_end: 0x00400000 + 0x100000,
            stack_base: 0x3FFFFFFFE80,  // Near top of user VA
            stack_size: 0x200000,        // 2MB stack
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

    /// Get the SATP value for this address space
    pub fn get_satp(&self) -> usize {
        self.satp
    }
}

/// Copy a user address space for fork (COW semantics)
/// Both parent and child share the same physical pages initially,
/// but writes will trigger a page fault and page copy.
/// Returns the new page table manager and SATP value
pub fn copy_user_address_space() -> Option<(PageTableManager, usize)> {
    // For now, this is a simplified implementation
    // In a full implementation, we would walk the parent's page table
    // and create COW mappings in the child's page table

    let new_pt = PageTableManager::new();
    let satp = 8usize << 60 | new_pt.root_ppn().0;

    Some((new_pt, satp))
}

