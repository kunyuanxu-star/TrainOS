use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use super::layout::PAGE_SIZE;

/// Sv39 Page Table Entry
#[derive(Clone, Copy, Debug)]
#[repr(transparent)]
pub struct PTE(usize);

impl PTE {
    pub const fn empty() -> Self {
        PTE(0)
    }

    pub fn is_valid(&self) -> bool {
        self.0 & 1 != 0
    }
    pub fn is_leaf(&self) -> bool {
        self.is_valid() && (self.0 & 0b1110 != 0) // R, W, or X set
    }
    pub fn is_branch(&self) -> bool {
        self.is_valid() && (self.0 & 0b1110 == 0) // valid but no R/W/X => pointer
    }

    pub fn ppn(&self) -> usize {
        (self.0 >> 10) & ((1 << 44) - 1)
    }
    pub fn phys_addr(&self) -> usize {
        self.ppn() << 12
    }

    pub fn set_ppn(&mut self, ppn: usize) {
        self.0 = (self.0 & 0x3FF) | ((ppn & ((1 << 44) - 1)) << 10);
    }

    pub fn set_flags(&mut self, r: bool, w: bool, x: bool, u: bool) {
        let mut flags = 1u8; // V
        if r {
            flags |= 1 << 1;
        }
        if w {
            flags |= 1 << 2;
        }
        if x {
            flags |= 1 << 3;
        }
        if u {
            flags |= 1 << 4;
        }
        self.0 = (self.0 & !0xFF) | flags as usize;
    }

    // A and D bits (hardware-managed, but may need explicit setting)
    pub fn is_accessed(&self) -> bool {
        (self.0 >> 6) & 1 != 0
    }
    pub fn set_accessed(&mut self, a: bool) {
        if a {
            self.0 |= 1 << 6;
        } else {
            self.0 &= !(1 << 6);
        }
    }
    pub fn is_dirty(&self) -> bool {
        (self.0 >> 7) & 1 != 0
    }
    pub fn set_dirty(&mut self, d: bool) {
        if d {
            self.0 |= 1 << 7;
        } else {
            self.0 &= !(1 << 7);
        }
    }

    // RSW bits (software-defined)
    pub fn is_cow(&self) -> bool {
        (self.0 >> 8) & 1 != 0
    }
    pub fn set_cow(&mut self, cow: bool) {
        if cow {
            self.0 |= 1 << 8;
        } else {
            self.0 &= !(1 << 8);
        }
    }
    pub fn is_shared(&self) -> bool {
        (self.0 >> 9) & 1 != 0
    }
    pub fn set_shared(&mut self, shared: bool) {
        if shared {
            self.0 |= 1 << 9;
        } else {
            self.0 &= !(1 << 9);
        }
    }

    pub fn is_writable(&self) -> bool {
        (self.0 >> 2) & 1 != 0
    }
    pub fn is_readable(&self) -> bool {
        (self.0 >> 1) & 1 != 0
    }
    pub fn is_executable(&self) -> bool {
        (self.0 >> 3) & 1 != 0
    }
    pub fn is_user(&self) -> bool {
        (self.0 >> 4) & 1 != 0
    }
}

/// Sv39 virtual address decomposition
pub const VPN2_SHIFT: usize = 30;
pub const VPN1_SHIFT: usize = 21;
pub const VPN0_SHIFT: usize = 12;
pub const VPN_MASK: usize = 0x1FF;

pub fn vpn2(va: usize) -> usize {
    (va >> VPN2_SHIFT) & VPN_MASK
}
pub fn vpn1(va: usize) -> usize {
    (va >> VPN1_SHIFT) & VPN_MASK
}
pub fn vpn0(va: usize) -> usize {
    (va >> VPN0_SHIFT) & VPN_MASK
}
pub fn offset(va: usize) -> usize {
    va & (PAGE_SIZE - 1)
}
pub fn page_align_down(va: usize) -> usize {
    va & !(PAGE_SIZE - 1)
}
pub fn page_align_up(va: usize) -> usize {
    (va + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)
}

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
    let l2_page = super::buddy::alloc_page().expect("failed to allocate root PT");
    unsafe {
        let pt = page_table_page(l2_page);
        for pte in pt.iter_mut() {
            *pte = PTE::empty();
        }
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
        if !alloc {
            return None;
        }
        let new_page = super::buddy::alloc_page()?;
        let new_pt = page_table_page(new_page);
        for pte in new_pt.iter_mut() {
            *pte = PTE::empty();
        }
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
        if !alloc {
            return None;
        }
        let new_page = super::buddy::alloc_page()?;
        let new_pt = page_table_page(new_page);
        for pte in new_pt.iter_mut() {
            *pte = PTE::empty();
        }
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

/// Check if a user-space page is present (mapped) in a given page table.
/// Returns 1 if present, 0 if not.
pub fn is_user_page_present(pt_root: usize, va: usize) -> u8 {
    unsafe {
        // Walk manually using the given page table root
        let vpn2_idx = vpn2(va);
        let vpn1_idx = vpn1(va);
        let vpn0_idx = vpn0(va);

        if vpn2_idx >= 256 {
            // Kernel space — skip
            return 0;
        }

        let l2 = &*(pa_to_kva(pt_root) as *const [PTE; 512]);
        if !l2[vpn2_idx].is_valid() || l2[vpn2_idx].is_leaf() {
            return 0;
        }
        let l1_phys = l2[vpn2_idx].phys_addr();
        let l1 = &*(pa_to_kva(l1_phys) as *const [PTE; 512]);
        if !l1[vpn1_idx].is_valid() {
            return 0;
        }
        if l1[vpn1_idx].is_leaf() {
            // 2MB superpage — present
            return if l1[vpn1_idx].is_user() { 1 } else { 0 };
        }
        let l0_phys = l1[vpn1_idx].phys_addr();
        let l0 = &*(pa_to_kva(l0_phys) as *const [PTE; 512]);
        if !l0[vpn0_idx].is_valid() || !l0[vpn0_idx].is_leaf() {
            return 0;
        }
        if l0[vpn0_idx].is_user() { 1 } else { 0 }
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

/// Copy kernel page table entries into a target root page table.
/// Each kernel L1 page table page is **deep-copied** so that the process
/// does not share L1 pages with the kernel (or other processes).
///
/// Without deep copy, the ELF loader would treat kernel L1 pages as
/// writable and would corrupt the page table when mapping user code at
/// low VPN2 indices that share an L1 page with a kernel mapping
/// (e.g. the CLINT mapping at L2[0]).
pub unsafe fn copy_kernel_mappings(target_root_phys: usize) {
    let kernel_root = root_pt_phys();
    let kernel_l2 = page_table_page_ref(kernel_root);
    let target_l2 = page_table_page(target_root_phys);

    for i in 0..512 {
        let entry = kernel_l2[i];
        if entry.is_branch() {
            // Deep-copy the L1 page so the process gets its own copy.
            let kernel_l1 = page_table_page_ref(entry.phys_addr());
            let new_l1 =
                super::buddy::alloc_page().expect("copy_kernel_mappings: OOM allocating L1 copy");
            let new_l1_pt = page_table_page(new_l1);
            new_l1_pt.copy_from_slice(kernel_l1);
            let mut new_entry = PTE::empty();
            new_entry.set_ppn(new_l1 >> 12);
            new_entry.set_flags(false, false, false, false);
            target_l2[i] = new_entry;
        } else {
            // Leaf or invalid: copy verbatim.
            target_l2[i] = entry;
        }
    }
}

/// Map a virtual address to a physical page in a process-specific page table.
pub unsafe fn map_user_page(
    root_phys: usize, va: usize, pa: usize, r: bool, w: bool,
) -> Result<(), &'static str> {
    // Walk the process's page table, allocating missing levels
    let (l0_phys, idx) = walk_process_pt(root_phys, va, true)
        .ok_or("map_user_page: walk failed")?;
    let l0 = &mut *(pa_to_kva(l0_phys) as *mut [PTE; 512]);
    let mut pte = PTE::empty();
    pte.set_ppn(pa >> 12);
    pte.set_flags(r, w, false, true); // U=1 (user-accessible)
    pte.set_accessed(true);
    pte.set_dirty(true);
    l0[idx] = pte;
    Ok(())
}

/// Unmap a virtual address from a process-specific page table.
pub unsafe fn unmap_user_page(root_phys: usize, va: usize) {
    if let Some((l0_phys, idx)) = walk_process_pt(root_phys, va, false) {
        let l0 = &mut *(pa_to_kva(l0_phys) as *mut [PTE; 512]);
        l0[idx] = PTE::empty();
    }
}

/// Walk a process-specific page table (not the kernel's ROOT_PT).
pub unsafe fn walk_process_pt(
    root_phys: usize, va: usize, alloc: bool,
) -> Option<(usize, usize)> {
    let vpn2_idx = vpn2(va);
    let vpn1_idx = vpn1(va);
    let vpn0_idx = vpn0(va);

    let l2 = &mut *(pa_to_kva(root_phys) as *mut [PTE; 512]);

    // L2 → L1
    let l1_phys = if !l2[vpn2_idx].is_valid() {
        if !alloc { return None; }
        let new_page = super::buddy::alloc_page()?;
        core::ptr::write_bytes(pa_to_kva(new_page) as *mut u8, 0, PAGE_SIZE);
        let mut entry = PTE::empty();
        entry.set_ppn(new_page >> 12);
        entry.set_flags(false, false, false, false);
        l2[vpn2_idx] = entry;
        new_page
    } else if l2[vpn2_idx].is_leaf() {
        return None;
    } else {
        l2[vpn2_idx].phys_addr()
    };

    // L1 → L0
    let l1 = &mut *(pa_to_kva(l1_phys) as *mut [PTE; 512]);
    if l1[vpn1_idx].is_leaf() {
        return None;
    }
    let l0_phys = if !l1[vpn1_idx].is_valid() {
        if !alloc { return None; }
        let new_page = super::buddy::alloc_page()?;
        core::ptr::write_bytes(pa_to_kva(new_page) as *mut u8, 0, PAGE_SIZE);
        let mut entry = PTE::empty();
        entry.set_ppn(new_page >> 12);
        entry.set_flags(false, false, false, false);
        l1[vpn1_idx] = entry;
        new_page
    } else {
        l1[vpn1_idx].phys_addr()
    };

    Some((l0_phys, vpn0_idx))
}
pub fn make_satp(root_phys: usize) -> usize {
    (8usize << 60) | (root_phys >> 12)
}

/// Set up kernel identity mapping for all physical memory.
/// KERNEL_VBASE is aligned so that VPN2=0 for the first 1GB.
/// We use 2MB superpages (L1 entries with R/W/X set, no L0 page).
/// Count user pages (V=1, U=1) in a page table. For invariant checks.
pub fn count_user_pages(root_phys: usize) -> usize {
    let mut count = 0usize;
    unsafe {
        let l2 = &*(pa_to_kva(root_phys) as *const [PTE; 512]);
        for vpn2 in 0..256 {
            let l2e = l2[vpn2];
            if !l2e.is_valid() || l2e.is_leaf() { continue; }
            let l1 = &*(pa_to_kva(l2e.phys_addr()) as *const [PTE; 512]);
            for vpn1 in 0..512 {
                let l1e = l1[vpn1];
                if !l1e.is_valid() { continue; }
                if l1e.is_leaf() {
                    if l1e.is_user() { count += 1; }
                    continue;
                }
                let l0 = &*(pa_to_kva(l1e.phys_addr()) as *const [PTE; 512]);
                for vpn0 in 0..512 {
                    let l0e = l0[vpn0];
                    if l0e.is_valid() && l0e.is_user() { count += 1; }
                }
            }
        }
    }
    count
}

/// Check whether a single user-page at `va` is valid (mapped and user-accessible)
/// in the page table rooted at `root_phys`.
fn is_user_addr_valid(root_phys: usize, va: usize) -> bool {
    unsafe {
        if let Some((l0_phys, idx)) = walk_process_pt(root_phys, va, false) {
            let l0 = &*(pa_to_kva(l0_phys) as *const [PTE; 512]);
            l0[idx].is_valid() && l0[idx].is_user()
        } else {
            false
        }
    }
}

/// V21.9: Validate that a user-space buffer [va, va+len) resides entirely within
/// mapped, user-accessible pages.  Returns `true` if every page touched by the
/// range is valid, `false` otherwise.
pub fn is_user_range_valid(root_phys: usize, va: usize, len: usize) -> bool {
    if len == 0 {
        return true;
    }
    let start_page = page_align_down(va);
    let end = va.saturating_add(len);
    let end_page = page_align_down(end.saturating_sub(1));
    let mut page = start_page;
    while page <= end_page {
        if !is_user_addr_valid(root_phys, page) {
            return false;
        }
        page = page.saturating_add(PAGE_SIZE);
    }
    true
}

/// V21.10: Walk all user PTEs in the page table rooted at `root_phys` and clear
/// the X (execute) bit on any page that has both W and X set (W^X violation).
/// Returns the number of pages fixed.  Each fix is logged.
pub fn force_wxorx(root_phys: usize) -> usize {
    let mut fixed = 0usize;
    unsafe {
        let l2 = &mut *(pa_to_kva(root_phys) as *mut [PTE; 512]);
        for vpn2 in 0..256 {
            let l2e = &l2[vpn2];
            if !l2e.is_valid() || l2e.is_leaf() {
                continue;
            }
            let l1 = &mut *(pa_to_kva(l2e.phys_addr()) as *mut [PTE; 512]);
            for vpn1 in 0..512 {
                let l1e = &mut l1[vpn1];
                if !l1e.is_valid() {
                    continue;
                }
                if l1e.is_leaf() {
                    // 2 MB superpage
                    if l1e.is_user() && l1e.is_writable() && l1e.is_executable() {
                        let r = l1e.is_readable();
                        let w = l1e.is_writable();
                        let u = l1e.is_user();
                        l1e.set_flags(r, w, false, u);
                        let va = (vpn2 << 30) | (vpn1 << 21);
                        crate::println!("W^X: cleared X at va=0x{:x}", va);
                        fixed += 1;
                    }
                    continue;
                }
                // L0 level — 4 KiB pages
                let l0 = &mut *(pa_to_kva(l1e.phys_addr()) as *mut [PTE; 512]);
                for vpn0 in 0..512 {
                    let l0e = &mut l0[vpn0];
                    if !l0e.is_valid() {
                        continue;
                    }
                    if l0e.is_user() && l0e.is_writable() && l0e.is_executable() {
                        let r = l0e.is_readable();
                        let w = l0e.is_writable();
                        let u = l0e.is_user();
                        l0e.set_flags(r, w, false, u);
                        let va = (vpn2 << 30) | (vpn1 << 21) | (vpn0 << 12);
                        crate::println!("W^X: cleared X at va=0x{:x}", va);
                        fixed += 1;
                    }
                }
            }
        }
    }
    fixed
}

pub unsafe fn setup_kernel_mapping() {
    let dram_base = super::layout::DRAM_BASE;
    let root = root_pt_phys();

    // Allocate ONE L1 page
    let l1_page =
        super::buddy::alloc_page().expect("failed to allocate L1 page for kernel mapping");
    let l1 = page_table_page(l1_page);
    for pte in l1.iter_mut() {
        *pte = PTE::empty();
    }

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
    for (i, pte) in l1.iter_mut().enumerate().take(64) {
        let pa = dram_base + i * 0x20_0000; // 2MB-aligned
        pte.set_ppn(pa >> 12);
        pte.set_flags(true, true, true, false); // R+W+X, kernel
        pte.set_accessed(true);
        pte.set_dirty(true);
    }

    // Identity-map the CLINT MMIO region at 0x02000000 so that
    // clint_set_next_timer() (which reads/writes CLINT_MTIME/MTIMECMP
    // via physical addresses) does not fault after MMU is enabled.
    // CLINT_BASE = 0x02000000 -> vpn2 = (0x02000000 >> 30) & 0x1FF = 0.
    let l1_mmio_page =
        super::buddy::alloc_page().expect("failed to allocate L1 page for CLINT mapping");
    let l1_mmio = page_table_page(l1_mmio_page);
    for pte in l1_mmio.iter_mut() {
        *pte = PTE::empty();
    }

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

    // Identity-map the MMIO region [0x10000000, 0x10200000) for
    // kernel proxy syscalls (UART at 0x10000000, VirtIO at 0x10001000).
    // Shares the same L2[0] → L1 page as CLINT.
    // VPN1(0x10000000) = 128 in this 1GB L2[0] window.
    let mmio_l1_idx = vpn1(0x10000000);
    let mut mmio_pte = PTE::empty();
    mmio_pte.set_ppn(0x10000000 >> 12);
    mmio_pte.set_flags(true, true, false, false); // R+W, kernel-only
    mmio_pte.set_accessed(true);
    mmio_pte.set_dirty(true);
    l1_mmio[mmio_l1_idx] = mmio_pte;

    // Identity-map the PCI ECAM region at [0x30000000, 0x30200000) for
    // V7.0D PCI bus enumeration via kernel proxy syscalls.
    // 2MB superpage, L1 index = vpn1(0x30000000) = 384.
    let ecam_l1_idx = vpn1(0x30000000);
    let mut ecam_pte = PTE::empty();
    ecam_pte.set_ppn(0x30000000 >> 12);
    ecam_pte.set_flags(true, true, false, false); // R+W, kernel-only
    ecam_pte.set_accessed(true);
    ecam_pte.set_dirty(true);
    l1_mmio[ecam_l1_idx] = ecam_pte;
}

// ── V31: One-Level Memory Management ──────────────────────────────────────

/// Map a page with full flag control (R, W, X, U).
///
/// Unlike `map_user_page` (which forces X=false, U=true), this function
/// accepts arbitrary flags and is suitable for use by the PteManager and TxMMU.
pub unsafe fn map_page(
    root_phys: usize,
    va: usize,
    pa: usize,
    r: bool,
    w: bool,
    x: bool,
    u: bool,
) -> Result<(), &'static str> {
    let (l0_phys, idx) =
        walk_process_pt(root_phys, va, true).ok_or("map_page: walk failed")?;
    let l0 = &mut *(pa_to_kva(l0_phys) as *mut [PTE; 512]);
    let mut pte = PTE::empty();
    pte.set_ppn(pa >> 12);
    pte.set_flags(r, w, x, u);
    pte.set_accessed(true);
    pte.set_dirty(true);
    l0[idx] = pte;
    Ok(())
}

/// Unmap a user page from a process-specific page table and return its
/// physical address.  Returns `None` if the VA was not mapped.
pub unsafe fn unmap_user_page_phys(root_phys: usize, va: usize) -> Option<usize> {
    let (l0_phys, idx) = walk_process_pt(root_phys, va, false)?;
    let l0 = &mut *(pa_to_kva(l0_phys) as *mut [PTE; 512]);
    let pte = l0[idx];
    if pte.is_valid() && pte.is_leaf() {
        l0[idx] = PTE::empty();
        Some(pte.phys_addr())
    } else {
        None
    }
}

// ── V35: Multi-Size Transparent Huge Pages (mTHP) ───────────────────────────────

/// mTHP configuration — controls which large-page sizes are enabled and
/// the minimum mapping size (in 4K pages) before trying a larger page.
#[derive(Debug, Clone, Copy)]
pub struct ThpConfig {
    pub enable_2m: bool,
    pub enable_1g: bool,
    pub thp_2m_threshold: usize,   // min 4K pages to try 2MB mapping
    pub thp_1g_threshold: usize,   // min 4K pages to try 1GB mapping
}

pub static THP_CONFIG: Mutex<ThpConfig> = Mutex::new(ThpConfig {
    enable_2m: true,
    enable_1g: false,          // buddy allocator max order is 12 (16MB), not enough
    thp_2m_threshold: 512,     // at least 2MB to try 2MB superpage
    thp_1g_threshold: 262144,  // at least 1GB
});

/// mTHP performance counters.
pub struct ThpStats {
    pub promotions: u64,
    pub splits: u64,
}

static THP_PROMOTIONS: AtomicU64 = AtomicU64::new(0);
static THP_SPLITS: AtomicU64 = AtomicU64::new(0);

/// Try to map a range as 2MB superpages (L1 leaf entries).
///
/// `root_phys` — physical address of the L2 page table.
/// `va` — must be 2MB-aligned.
/// `count_4k` — number of 4K pages (must be >= 512).
/// `flags` — bitwise OR of R(1), W(2), X(4), U(8).
///
/// Returns `true` if at least one 2MB superpage was created.
pub fn try_map_2m(root_phys: usize, va: usize, count_4k: usize, flags: u8) -> bool {
    if count_4k < 512 {
        return false;
    }
    if va & (0x200000 - 1) != 0 {
        return false; // not 2MB-aligned
    }

    let r = flags & 1 != 0;
    let w = flags & 2 != 0;
    let x = flags & 4 != 0;
    let u = flags & 8 != 0;

    unsafe {
        // Allocate 2MB contiguous from buddy (order 9 = 2^9 * 4K)
        let pa = match super::buddy::alloc_pages(9) {
            Some(p) => p,
            None => return false,
        };
        // Zero the whole 2MB range
        core::ptr::write_bytes(pa_to_kva(pa) as *mut u8, 0, 0x200000);

        let vpn2_idx = vpn2(va);
        let vpn1_idx = vpn1(va);

        let l2 = &mut *(pa_to_kva(root_phys) as *mut [PTE; 512]);

        // Ensure the L2 → L1 page table page exists
        if !l2[vpn2_idx].is_valid() {
            let l1_page = match super::buddy::alloc_page() {
                Some(p) => p,
                None => {
                    super::buddy::free_page(pa, 9);
                    return false;
                }
            };
            core::ptr::write_bytes(pa_to_kva(l1_page) as *mut u8, 0, PAGE_SIZE);
            let mut entry = PTE::empty();
            entry.set_ppn(l1_page >> 12);
            entry.set_flags(false, false, false, false);
            l2[vpn2_idx] = entry;
        } else if l2[vpn2_idx].is_leaf() {
            // Already a 1GB superpage — can't sub-map with 2MB
            super::buddy::free_page(pa, 9);
            return false;
        }

        // Set the L1 entry as a 2MB leaf superpage
        let l1 = &mut *(pa_to_kva(l2[vpn2_idx].phys_addr()) as *mut [PTE; 512]);
        if l1[vpn1_idx].is_valid() {
            super::buddy::free_page(pa, 9);
            return false; // already mapped
        }

        let mut pte = PTE::empty();
        pte.set_ppn(pa >> 12);
        pte.set_flags(r, w, x, u);
        pte.set_accessed(true);
        pte.set_dirty(true);
        l1[vpn1_idx] = pte;

        THP_PROMOTIONS.fetch_add(1, Ordering::Relaxed);
    }
    true
}

/// Try to map a range as a 1GB superpage (L2 leaf entry).
///
/// NOTE: The buddy allocator's max order is 12 (16MB), so 1GB
/// (order 18) will always fail.  This function exists for future
/// use with a larger allocator or physically-contiguous reserved regions.
pub fn try_map_1g(_root_phys: usize, _va: usize, _count_4k: usize, _flags: u8) -> bool {
    false // not yet supported (buddy max order = 12)
}

/// Split a 2MB superpage at `va` into 512 individual 4K pages.
///
/// Allocates an L0 page table page and fills it with PTEs pointing to
/// the sub-pages of the existing 2MB physical allocation.
/// Used for COW and mprotect on individual 4K pages.
pub fn split_large_page(root_phys: usize, va: usize) -> Result<(), &'static str> {
    unsafe {
        let vpn2_idx = vpn2(va);
        let vpn1_idx = vpn1(va);

        let l2 = &*(pa_to_kva(root_phys) as *const [PTE; 512]);
        if !l2[vpn2_idx].is_valid() {
            return Err("split: L2 not valid");
        }
        if l2[vpn2_idx].is_leaf() {
            return Err("split: L2 leaf (1GB) not supported");
        }

        let l1_phys = l2[vpn2_idx].phys_addr();
        let l1 = &mut *(pa_to_kva(l1_phys) as *mut [PTE; 512]);

        if !l1[vpn1_idx].is_leaf() {
            return Err("split: not a 2MB superpage");
        }

        let super_phys = l1[vpn1_idx].phys_addr();
        let r = l1[vpn1_idx].is_readable();
        let w = l1[vpn1_idx].is_writable();
        let x = l1[vpn1_idx].is_executable();
        let u = l1[vpn1_idx].is_user();
        let cow = l1[vpn1_idx].is_cow();
        let shared = l1[vpn1_idx].is_shared();

        // Allocate an L0 page table page
        let l0_page = super::buddy::alloc_page().ok_or("split: OOM allocating L0 page")?;
        let l0 = &mut *(pa_to_kva(l0_page) as *mut [PTE; 512]);

        // Fill with 512× 4K PTEs into the existing 2MB physical region
        for i in 0..512 {
            let page_pa = super_phys + i * PAGE_SIZE;
            let mut pte = PTE::empty();
            pte.set_ppn(page_pa >> 12);
            pte.set_flags(r, w, x, u);
            pte.set_accessed(true);
            pte.set_dirty(true);
            if cow {
                pte.set_cow(true);
            }
            if shared {
                pte.set_shared(true);
            }
            l0[i] = pte;
        }

        // Replace L1 leaf entry with a branch to the L0 table
        let mut entry = PTE::empty();
        entry.set_ppn(l0_page >> 12);
        entry.set_flags(false, false, false, false);
        l1[vpn1_idx] = entry;

        // Flush TLB for the 2MB range
        core::arch::asm!("sfence.vma {}", in(reg) va);

        THP_SPLITS.fetch_add(1, Ordering::Relaxed);
    }
    Ok(())
}

/// Attempt to promote a run of contiguous 4K pages into a 2MB superpage.
///
/// Checks if the 512 consecutive 4K pages starting at `va` are all mapped,
/// physically contiguous, and 2MB-aligned.  If so, replaces them with a
/// single L1 leaf entry and frees the L0 page table page.
///
/// Returns `true` if promotion succeeded.
pub fn try_promote(root_phys: usize, va: usize) -> bool {
    if va & (0x200000 - 1) != 0 {
        return false;
    }
    unsafe {
        let vpn2_idx = vpn2(va);
        let vpn1_idx = vpn1(va);

        let l2 = &mut *(pa_to_kva(root_phys) as *mut [PTE; 512]);
        if !l2[vpn2_idx].is_valid() || l2[vpn2_idx].is_leaf() {
            return false;
        }

        let l1 = &mut *(pa_to_kva(l2[vpn2_idx].phys_addr()) as *mut [PTE; 512]);
        if l1[vpn1_idx].is_leaf() {
            return false; // already a superpage
        }
        let l0_phys = l1[vpn1_idx].phys_addr();
        let l0 = &*(pa_to_kva(l0_phys) as *const [PTE; 512]);

        // Read the first PTE to get flags and base physical address
        let first = l0[0];
        if !first.is_valid() || !first.is_leaf() {
            return false;
        }
        let base_phys = first.phys_addr();

        // Check 2MB alignment
        if base_phys & (0x200000 - 1) != 0 {
            return false;
        }

        // Verify all 512 PTEs are present, contiguous, and share the same flags
        for i in 0..512 {
            let pte = l0[i];
            if !pte.is_valid() || !pte.is_leaf() {
                return false;
            }
            if pte.phys_addr() != base_phys + i * PAGE_SIZE {
                return false; // not physically contiguous
            }
            if pte.is_readable() != first.is_readable()
                || pte.is_writable() != first.is_writable()
                || pte.is_executable() != first.is_executable()
                || pte.is_user() != first.is_user()
            {
                return false; // flags differ
            }
        }

        // Promote: set L1 entry as 2MB superpage, free L0 page table page
        let r = first.is_readable();
        let w = first.is_writable();
        let x = first.is_executable();
        let u = first.is_user();

        let mut super_pte = PTE::empty();
        super_pte.set_ppn(base_phys >> 12);
        super_pte.set_flags(r, w, x, u);
        super_pte.set_accessed(true);
        super_pte.set_dirty(true);
        l1[vpn1_idx] = super_pte;

        // Free the L0 page table page
        super::buddy::free_page(l0_phys, 0);

        core::arch::asm!("sfence.vma {}", in(reg) va);

        THP_PROMOTIONS.fetch_add(1, Ordering::Relaxed);
    }
    true
}

/// Get current mTHP performance stats.
pub fn thp_stats() -> ThpStats {
    ThpStats {
        promotions: THP_PROMOTIONS.load(Ordering::Relaxed),
        splits: THP_SPLITS.load(Ordering::Relaxed),
    }
}

/// Page table statistics: counts of pages at each granularity, total bytes,
/// sealed pages, mTHP promotion/split counters, and per-CPU cache efficiency.
#[derive(Debug, Clone, Copy)]
pub struct PteStats {
    pub mapped_4k: usize,
    pub mapped_2m: usize,
    pub mapped_1g: usize,
    pub total_bytes: usize,
    pub sealed_pages: usize,
    pub thp_promotions: u64,
    pub thp_splits: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

/// Direct page table manager — operates directly on page tables without
/// intermediate software abstractions (VMA, region, etc.).
///
/// All ranges are expressed as byte-ranges [va, va+len); they are
/// automatically page-aligned internally.
pub struct PteManager {
    root_phys: usize,
}

impl PteManager {
    /// Create a new manager for the page table rooted at `root_phys`.
    pub fn new(root_phys: usize) -> Self {
        PteManager { root_phys }
    }

    /// Map a range [va, va+len) to physical pages allocated on-demand.
    ///
    /// `flags` is a bitwise OR of R(1), W(2), X(4), U(8) matching the
    /// TxMMU `FLAG_*` constants.  Each allocated page is zeroed before
    /// mapping.
    ///
    /// When mTHP is enabled and the range is large enough, 2MB superpages
    /// are used automatically for aligned sub-ranges (V35).
    pub unsafe fn map_range(
        &mut self,
        va: usize,
        len: usize,
        flags: u8,
    ) -> Result<(), &'static str> {
        let r = flags & 1 != 0;
        let w = flags & 2 != 0;
        let x = flags & 4 != 0;
        let u = flags & 8 != 0;

        let start = page_align_down(va);
        let end = page_align_up(va + len);
        let total_pages = (end - start) / PAGE_SIZE;

        // Try 2MB superpages for large aligned ranges (mTHP)
        let thp_cfg = THP_CONFIG.lock();
        let use_2m = thp_cfg.enable_2m && total_pages >= thp_cfg.thp_2m_threshold;
        drop(thp_cfg);

        let mut page_va = start;
        while page_va < end {
            if use_2m
                && (page_va & (0x200000 - 1)) == 0
                && (end - page_va) >= 0x200000
            {
                if try_map_2m(self.root_phys, page_va, 512, flags) {
                    page_va += 0x200000;
                    continue;
                }
                // Fall through to 4K mapping if try_map_2m fails
            }
            // 4K fallback
            let pa = super::buddy::alloc_page().ok_or("PteManager map_range: OOM")?;
            core::ptr::write_bytes(pa_to_kva(pa) as *mut u8, 0, PAGE_SIZE);
            map_page(self.root_phys, page_va, pa, r, w, x, u)?;
            page_va += PAGE_SIZE;
        }
        Ok(())
    }

    /// Unmap a range [va, va+len) and free the underlying physical pages.
    pub unsafe fn unmap_range(
        &mut self,
        va: usize,
        len: usize,
    ) -> Result<(), &'static str> {
        let start = page_align_down(va);
        let end = page_align_up(va + len);
        let mut page_va = start;
        while page_va < end {
            if let Some(pa) = unmap_user_page_phys(self.root_phys, page_va) {
                super::buddy::free_page(pa, 0);
            }
            page_va += PAGE_SIZE;
        }
        Ok(())
    }

    /// Change protection flags on a range of mapped pages.
    ///
    /// `flags` uses the same bit encoding as `map_range`.
    pub unsafe fn protect_range(
        &mut self,
        va: usize,
        len: usize,
        flags: u8,
    ) -> Result<(), &'static str> {
        let r = flags & 1 != 0;
        let w = flags & 2 != 0;
        let x = flags & 4 != 0;
        let u = flags & 8 != 0;

        let start = page_align_down(va);
        let end = page_align_up(va + len);
        let mut page_va = start;
        while page_va < end {
            if let Some((l0_phys, idx)) = walk_process_pt(self.root_phys, page_va, false) {
                let l0 = &mut *(pa_to_kva(l0_phys) as *mut [PTE; 512]);
                if l0[idx].is_valid() {
                    l0[idx].set_flags(r, w, x, u);
                }
            }
            page_va += PAGE_SIZE;
        }
        Ok(())
    }

    /// Count the number of allocated (leaf) pages mapped in this page table.
    pub fn count_allocated(&self) -> usize {
        let mut count = 0usize;
        unsafe {
            let l2 = &*(pa_to_kva(self.root_phys) as *const [PTE; 512]);
            for vpn2 in 0..256 {
                let l2e = l2[vpn2];
                if !l2e.is_valid() || l2e.is_leaf() {
                    continue;
                }
                let l1 = &*(pa_to_kva(l2e.phys_addr()) as *const [PTE; 512]);
                for vpn1 in 0..512 {
                    let l1e = l1[vpn1];
                    if !l1e.is_valid() {
                        continue;
                    }
                    if l1e.is_leaf() {
                        count += 1; // 2 MiB superpage
                        continue;
                    }
                    let l0 = &*(pa_to_kva(l1e.phys_addr()) as *const [PTE; 512]);
                    for vpn0 in 0..512 {
                        if l0[vpn0].is_valid() && l0[vpn0].is_leaf() {
                            count += 1; // 4 KiB page
                        }
                    }
                }
            }
        }
        count
    }

    /// Find a free virtual address range of the given size (in bytes).
    ///
    /// Scans the user address space (L2 indices 0..255) linearly, starting
    /// from a safe offset, and returns the first gap large enough to hold
    /// `size` bytes.
    pub fn find_free_va(&self, size: usize) -> Option<usize> {
        let pages_needed = (size + PAGE_SIZE - 1) / PAGE_SIZE;
        if pages_needed == 0 {
            return None;
        }

        // Start scanning at 64 KiB to avoid low-zero-page trickery.
        let mut candidate = 0x1_0000usize;
        let end_limit = 0x0000_003F_FFFF_F000usize;

        while candidate < end_limit {
            let mut free_count = 0usize;
            let mut page_va = candidate;

            // Count consecutive free pages.
            while free_count < pages_needed && page_va < end_limit {
                unsafe {
                    if self.is_mapped(page_va) {
                        break;
                    }
                }
                free_count += 1;
                page_va += PAGE_SIZE;
            }

            if free_count >= pages_needed {
                return Some(candidate);
            }

            // Advance past the occupied page.
            candidate = page_align_up(page_va + 1);
        }

        None
    }

    /// Return `true` if the given VA is mapped and leaf-valid.
    unsafe fn is_mapped(&self, va: usize) -> bool {
        if let Some((l0_phys, idx)) = walk_process_pt(self.root_phys, va, false) {
            let l0 = &*(pa_to_kva(l0_phys) as *const [PTE; 512]);
            l0[idx].is_valid() && l0[idx].is_leaf()
        } else {
            false
        }
    }

    /// Gather page table statistics.
    pub fn stats(&self) -> PteStats {
        let mut s = PteStats {
            mapped_4k: 0,
            mapped_2m: 0,
            mapped_1g: 0,
            total_bytes: 0,
            sealed_pages: super::mseal::total_sealed_pages(),
            thp_promotions: THP_PROMOTIONS.load(Ordering::Relaxed),
            thp_splits: THP_SPLITS.load(Ordering::Relaxed),
            cache_hits: super::buddy::cache_hits(),
            cache_misses: super::buddy::cache_misses(),
        };
        unsafe {
            let l2 = &*(pa_to_kva(self.root_phys) as *const [PTE; 512]);
            for vpn2 in 0..256 {
                let l2e = l2[vpn2];
                if !l2e.is_valid() {
                    continue;
                }
                if l2e.is_leaf() {
                    // 1 GiB superpage
                    s.mapped_1g += 1;
                    s.total_bytes += 1 << 30;
                    continue;
                }
                let l1 = &*(pa_to_kva(l2e.phys_addr()) as *const [PTE; 512]);
                for vpn1 in 0..512 {
                    let l1e = l1[vpn1];
                    if !l1e.is_valid() {
                        continue;
                    }
                    if l1e.is_leaf() {
                        // 2 MiB superpage
                        s.mapped_2m += 1;
                        s.total_bytes += 2 * 1024 * 1024;
                        continue;
                    }
                    let l0 = &*(pa_to_kva(l1e.phys_addr()) as *const [PTE; 512]);
                    for vpn0 in 0..512 {
                        if l0[vpn0].is_valid() && l0[vpn0].is_leaf() {
                            // 4 KiB page
                            s.mapped_4k += 1;
                            s.total_bytes += PAGE_SIZE;
                        }
                    }
                }
            }
        }
        s
    }
}

/// Share a physical page from one process with another.
///
/// Looks up the physical page mapped at `src_va` in the source process's
/// page table (identified by `src_pid`), then maps the same physical page
/// into the destination process's page table at `dst_va`.
///
/// Returns Ok(()) on success, or an error string on failure.
pub unsafe fn share_page(
    src_pid: u32,
    dst_pid: u32,
    src_va: usize,
    dst_va: usize,
) -> Result<(), &'static str> {
    // Look up page table roots for both processes
    let (src_root, dst_root) = {
        let procs = crate::proc::PROCESSES.lock();
        let src = procs.iter().find(|p| p.pid == src_pid).ok_or("src process not found")?;
        let dst = procs.iter().find(|p| p.pid == dst_pid).ok_or("dst process not found")?;
        (src.page_table_root, dst.page_table_root)
    };

    // Walk the source page table to find the physical page at src_va
    let (l0_phys, idx) = walk_process_pt(src_root, src_va, false)
        .ok_or("src_va not mapped")?;
    let l0 = &*(pa_to_kva(l0_phys) as *const [PTE; 512]);
    let pte = l0[idx];
    if !pte.is_valid() || !pte.is_leaf() {
        return Err("src_va not a valid mapped page");
    }
    let phys = pte.phys_addr();

    // Map the same physical page into the destination process
    map_user_page(dst_root, dst_va, phys, true, true)?;

    Ok(())
}

/// Splice (transfer) a range of pages from one process to another.
///
/// For each page in the range [src_va + offset, src_va + offset + len),
/// looks up the physical page in the source process and maps it into the
/// destination process at [dst_va + offset, dst_va + offset + len).
///
/// All addresses and lengths should be page-aligned for proper mapping.
pub unsafe fn splice_pages(
    src_pid: u32,
    dst_pid: u32,
    src_va: usize,
    offset: usize,
    dst_va: usize,
    len: usize,
) -> Result<(), &'static str> {
    // Look up page table roots
    let (src_root, dst_root) = {
        let procs = crate::proc::PROCESSES.lock();
        let src = procs.iter().find(|p| p.pid == src_pid).ok_or("src process not found")?;
        let dst = procs.iter().find(|p| p.pid == dst_pid).ok_or("dst process not found")?;
        (src.page_table_root, dst.page_table_root)
    };

    let mut off = offset;
    while off < offset + len {
        let src_page_va = page_align_down(src_va + off);
        let dst_page_va = page_align_down(dst_va + off);

        // Walk the source page table to get the physical page
        let (l0_phys, idx) = walk_process_pt(src_root, src_page_va, false)
            .ok_or("src page not mapped")?;
        let l0 = &*(pa_to_kva(l0_phys) as *const [PTE; 512]);
        let pte = l0[idx];
        if !pte.is_valid() || !pte.is_leaf() {
            return Err("src page not a valid leaf");
        }
        let phys = pte.phys_addr();

        // Map into destination
        map_user_page(dst_root, dst_page_va, phys, true, true)?;

        off += PAGE_SIZE;
    }

    Ok(())
}
