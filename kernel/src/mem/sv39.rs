use core::sync::atomic::{AtomicBool, Ordering};

use super::layout::PAGE_SIZE;

/// Sv39 Page Table Entry
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct PTE(usize);

impl PTE {
    pub const fn empty() -> Self { PTE(0) }

    pub fn is_valid(&self) -> bool { self.0 & 1 != 0 }
    pub fn is_leaf(&self) -> bool {
        self.is_valid() && (self.0 & 0b1110 != 0) // R, W, or X set
    }
    pub fn is_branch(&self) -> bool {
        self.is_valid() && (self.0 & 0b1110 == 0) // valid but no R/W/X => pointer
    }

    pub fn ppn(&self) -> usize { (self.0 >> 10) & ((1 << 44) - 1) }
    pub fn phys_addr(&self) -> usize { self.ppn() << 12 }

    pub fn set_ppn(&mut self, ppn: usize) {
        self.0 = (self.0 & 0x3FF) | ((ppn & ((1 << 44) - 1)) << 10);
    }

    pub fn set_flags(&mut self, r: bool, w: bool, x: bool, u: bool) {
        let mut flags = 1u8; // V
        if r { flags |= 1 << 1; }
        if w { flags |= 1 << 2; }
        if x { flags |= 1 << 3; }
        if u { flags |= 1 << 4; }
        self.0 = (self.0 & !0xFF) | flags as usize;
    }

    // A and D bits (hardware-managed, but may need explicit setting)
    pub fn is_accessed(&self) -> bool { (self.0 >> 6) & 1 != 0 }
    pub fn set_accessed(&mut self, a: bool) {
        if a { self.0 |= 1 << 6; } else { self.0 &= !(1 << 6); }
    }
    pub fn is_dirty(&self) -> bool { (self.0 >> 7) & 1 != 0 }
    pub fn set_dirty(&mut self, d: bool) {
        if d { self.0 |= 1 << 7; } else { self.0 &= !(1 << 7); }
    }

    // RSW bits (software-defined)
    pub fn is_cow(&self) -> bool { (self.0 >> 8) & 1 != 0 }
    pub fn set_cow(&mut self, cow: bool) {
        if cow { self.0 |= 1 << 8; } else { self.0 &= !(1 << 8); }
    }
    pub fn is_shared(&self) -> bool { (self.0 >> 9) & 1 != 0 }
    pub fn set_shared(&mut self, shared: bool) {
        if shared { self.0 |= 1 << 9; } else { self.0 &= !(1 << 9); }
    }

    pub fn is_writable(&self) -> bool { (self.0 >> 2) & 1 != 0 }
    pub fn is_readable(&self) -> bool { (self.0 >> 1) & 1 != 0 }
    pub fn is_executable(&self) -> bool { (self.0 >> 3) & 1 != 0 }
    pub fn is_user(&self) -> bool { (self.0 >> 4) & 1 != 0 }
}

/// Sv39 virtual address decomposition
pub const VPN2_SHIFT: usize = 30;
pub const VPN1_SHIFT: usize = 21;
pub const VPN0_SHIFT: usize = 12;
pub const VPN_MASK: usize = 0x1FF;

pub fn vpn2(va: usize) -> usize { (va >> VPN2_SHIFT) & VPN_MASK }
pub fn vpn1(va: usize) -> usize { (va >> VPN1_SHIFT) & VPN_MASK }
pub fn vpn0(va: usize) -> usize { (va >> VPN0_SHIFT) & VPN_MASK }
pub fn offset(va: usize) -> usize { va & (PAGE_SIZE - 1) }
pub fn page_align_down(va: usize) -> usize { va & !(PAGE_SIZE - 1) }
pub fn page_align_up(va: usize) -> usize { (va + PAGE_SIZE - 1) & !(PAGE_SIZE - 1) }

/// Kernel virtual base: physical DRAM is identity-mapped at this VA.
pub const KERNEL_VBASE: usize = 0xFFFF_FFC0_0000_0000;

pub fn pa_to_kva(pa: usize) -> usize {
    pa + KERNEL_VBASE - super::layout::DRAM_BASE
}
pub fn kva_to_pa(kva: usize) -> usize {
    kva - KERNEL_VBASE + super::layout::DRAM_BASE
}

/// Whether the MMU has been enabled. Before MMU is on, page table
/// code must use physical addresses directly (pa_to_kva would fault).
static MMU_ENABLED: AtomicBool = AtomicBool::new(false);

/// The root page table. Points to an L2 page allocated from the buddy allocator.
use spin::Mutex;
static ROOT_PT: Mutex<Option<usize>> = Mutex::new(None);

pub fn root_pt_phys() -> usize {
    ROOT_PT.lock().expect("root page table not initialized")
}

/// Get a mutable reference to a page table page by its physical address.
/// Before the MMU is enabled, uses the physical address directly.
unsafe fn page_table_page(phys: usize) -> &'static mut [PTE; 512] {
    let addr = if MMU_ENABLED.load(Ordering::Relaxed) {
        pa_to_kva(phys)
    } else {
        phys
    };
    &mut *(addr as *mut [PTE; 512])
}

unsafe fn page_table_page_ref(phys: usize) -> &'static [PTE; 512] {
    let addr = if MMU_ENABLED.load(Ordering::Relaxed) {
        pa_to_kva(phys)
    } else {
        phys
    };
    &*(addr as *const [PTE; 512])
}

/// Initialize root page table. Allocates an L2 page.
pub fn init_root_pt() {
    let l2_page = super::buddy::alloc_page()
        .expect("failed to allocate root PT");
    unsafe {
        let pt = page_table_page(l2_page);
        for i in 0..512 { pt[i] = PTE::empty(); }
    }
    *ROOT_PT.lock() = Some(l2_page);
}

/// Walk the page table for a virtual address.
/// Returns (phys_addr_of_l0_page_table, index_in_l0_pt).
/// Creates intermediate page table pages if `alloc` is true.
pub unsafe fn walk(va: usize, alloc: bool) -> Option<(usize, usize)> {
    let root = root_pt_phys();
    let vpn2_idx = vpn2(va);
    let vpn1_idx = vpn1(va);
    let vpn0_idx = vpn0(va);

    // L2 -> L1
    let l2 = page_table_page(root); // mutable
    let l1_phys = if !l2[vpn2_idx].is_valid() {
        if !alloc { return None; }
        let new_page = super::buddy::alloc_page()?;
        let new_pt = page_table_page(new_page);
        for i in 0..512 { new_pt[i] = PTE::empty(); }
        let mut entry = PTE::empty();
        entry.set_ppn(new_page >> 12);
        entry.set_flags(false, false, false, false); // non-leaf: R=W=X=0
        l2[vpn2_idx] = entry;
        new_page
    } else if l2[vpn2_idx].is_leaf() {
        // Superpage at L2 level -- not handling here, treat as end node
        return None;
    } else {
        l2[vpn2_idx].phys_addr()
    };

    // L1 -> L0 (check for superpage)
    let l1 = page_table_page(l1_phys); // mutable
    if l1[vpn1_idx].is_leaf() {
        // 2MB superpage -- already a leaf
        return None; // For map/unmap we need L0 level
    }
    let l0_phys = if !l1[vpn1_idx].is_valid() {
        if !alloc { return None; }
        let new_page = super::buddy::alloc_page()?;
        let new_pt = page_table_page(new_page);
        for i in 0..512 { new_pt[i] = PTE::empty(); }
        let mut entry = PTE::empty();
        entry.set_ppn(new_page >> 12);
        entry.set_flags(false, false, false, false); // non-leaf: R=W=X=0
        l1[vpn1_idx] = entry;
        new_page
    } else {
        l1[vpn1_idx].phys_addr()
    };

    Some((l0_phys, vpn0_idx))
}

/// Map a virtual address to a physical page.
pub unsafe fn map(va: usize, pa: usize, r: bool, w: bool, x: bool, u: bool) {
    let (l0_phys, idx) = walk(va, true).expect("failed to walk page table");
    let l0 = page_table_page(l0_phys);
    let mut pte = PTE::empty();
    pte.set_ppn(pa >> 12);
    pte.set_flags(r, w, x, u);
    pte.set_accessed(true);
    pte.set_dirty(true);
    l0[idx] = pte;
}

/// Unmap a virtual address. Returns the physical address that was mapped.
pub unsafe fn unmap(va: usize) -> Option<usize> {
    let (l0_phys, idx) = walk(va, false)?;
    let l0 = page_table_page(l0_phys);
    let pte = l0[idx];
    if pte.is_valid() && pte.is_leaf() {
        l0[idx] = PTE::empty();
        Some(pte.phys_addr())
    } else {
        None
    }
}

/// Translate a virtual address to its physical address.
pub unsafe fn virt_to_phys(va: usize) -> Option<usize> {
    let (l0_phys, idx) = walk(va, false)?;
    let l0 = page_table_page_ref(l0_phys);
    let pte = l0[idx];
    if pte.is_valid() && pte.is_leaf() {
        Some(pte.phys_addr() | offset(va))
    } else {
        None
    }
}

/// Enable Sv39 MMU by setting satp.
/// Gated behind not(test) because it uses RISC-V-specific assembly instructions.
#[cfg(not(test))]
#[inline(never)]
pub fn enable_mmu() {
    let root_ppn = root_pt_phys() >> 12;
    let satp_val: usize = (8usize << 60) | root_ppn;
    unsafe {
        core::arch::asm!(
            "csrw satp, {satp}",
            "sfence.vma zero, zero",
            "fence.i",
            satp = in(reg) satp_val,
        );
    }
    // After satp is written, all subsequent addresses go through the MMU.
    // Allow page_table_page/page_table_page_ref to use pa_to_kva.
    MMU_ENABLED.store(true, Ordering::SeqCst);
}

/// Copy all kernel L2-level page table entries into a target root page table.
/// This ensures kernel memory remains accessible when satp points to the process PT.
pub unsafe fn copy_kernel_mappings(target_root_phys: usize) {
    let kernel_root = root_pt_phys();
    let kernel_l2 = page_table_page_ref(kernel_root);
    let target_l2 = page_table_page(target_root_phys);
    for i in 0..512 {
        target_l2[i] = kernel_l2[i];
    }
}

/// Build an Sv39 satp value for a given root page table physical address.
pub fn make_satp(root_phys: usize) -> usize {
    (8usize << 60) | (root_phys >> 12)
}

/// Set up kernel identity mapping for all physical memory.
/// KERNEL_VBASE is aligned so that VPN2=0 for the first 1GB.
/// We use 2MB superpages (L1 entries with R/W/X set, no L0 page).
pub unsafe fn setup_kernel_mapping() {
    let dram_base = super::layout::DRAM_BASE;
    let root = root_pt_phys();

    // Allocate ONE L1 page
    let l1_page = super::buddy::alloc_page()
        .expect("failed to allocate L1 page for kernel mapping");
    let l1 = page_table_page(l1_page);
    for j in 0..512 { l1[j] = PTE::empty(); }

    // Point L2[vpn2(KERNEL_VBASE)] to the L1 page.
    let l2 = page_table_page(root);
    let l2_kva_idx = vpn2(KERNEL_VBASE);
    let mut l2_entry = PTE::empty();
    l2_entry.set_ppn(l1_page >> 12);
    l2_entry.set_flags(false, false, false, false);
    l2[l2_kva_idx] = l2_entry;

    // Identity-map the DRAM region [DRAM_BASE, DRAM_END) at low VAs.
    // The kernel's code and data symbols are linked at 0x8020xxxx, and
    // after csrw satp the first data access (e.g. to MMU_ENABLED) uses
    // those low virtual addresses.  Without this mapping the very first
    // post-MMU store would fault.
    // dram_base = 0x80000000 → vpn2 = (0x80000000 >> 30) & 0x1FF = 2.
    let l2_phys_idx = vpn2(dram_base);
    l2[l2_phys_idx] = l2_entry;

    // Fill 64 L1 entries, each a 2MB superpage.
    // VPN1 for KERNEL_VBASE is 0, so entries start at l1[0].
    // Set A (Accessed) and D (Dirty) bits explicitly.
    for i in 0..64 {
        let pa = dram_base + i * 0x20_0000; // 2MB-aligned
        let mut pte = PTE::empty();
        pte.set_ppn(pa >> 12);
        pte.set_flags(true, true, true, false); // R+W+X, kernel
        pte.set_accessed(true);
        pte.set_dirty(true);
        l1[i] = pte;
    }

    // Identity-map the CLINT MMIO region at 0x02000000 so that
    // clint_set_next_timer() (which reads/writes CLINT_MTIME/MTIMECMP
    // via physical addresses) does not fault after MMU is enabled.
    // CLINT_BASE = 0x02000000 -> vpn2 = (0x02000000 >> 30) & 0x1FF = 0.
    let l1_mmio_page = super::buddy::alloc_page()
        .expect("failed to allocate L1 page for CLINT mapping");
    let l1_mmio = page_table_page(l1_mmio_page);
    for j in 0..512 { l1_mmio[j] = PTE::empty(); }

    let l2_mmio_idx = vpn2(0x02000000);
    let mut l2_mmio_entry = PTE::empty();
    l2_mmio_entry.set_ppn(l1_mmio_page >> 12);
    l2_mmio_entry.set_flags(false, false, false, false);
    l2[l2_mmio_idx] = l2_mmio_entry;

    // 2MB superpage at CLINT_BASE
    let clint_l1_idx = vpn1(0x02000000);
    let mut clint_pte = PTE::empty();
    clint_pte.set_ppn(0x02000000 >> 12);
    clint_pte.set_flags(true, true, false, false); // R+W, kernel-only
    clint_pte.set_accessed(true);
    clint_pte.set_dirty(true);
    l1_mmio[clint_l1_idx] = clint_pte;
}
