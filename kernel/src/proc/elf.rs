use crate::mem::{buddy, layout::PAGE_SIZE, sv39};

/// ELF64 file header
#[repr(C)]
struct Elf64Header {
    ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

/// ELF64 program header
#[repr(C)]
struct Elf64Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_paddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}

const PT_LOAD: u32 = 1;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

// ── Page table helpers (parameterized by explicit root_pt) ──────────────
// These cannot use sv39::map/walk/virt_to_phys because those operate on
// the *global* kernel page table root.  We need to write into the per-process
// page table whose root is given as an argument.

/// Get mutable reference to a page table page at given phys addr.
unsafe fn pt_page_mut(phys: usize) -> &'static mut [sv39::PTE; 512] {
    let kva = sv39::pa_to_kva(phys);
    &mut *(kva as *mut [sv39::PTE; 512])
}

unsafe fn pt_page_ref(phys: usize) -> &'static [sv39::PTE; 512] {
    let kva = sv39::pa_to_kva(phys);
    &*(kva as *const [sv39::PTE; 512])
}

/// Walk a page table (given its root) and return (l0_phys, l0_index).
/// Creates intermediate page table pages if `alloc` is true.
pub(crate) unsafe fn walk_pt(root_pt: usize, va: usize, alloc: bool) -> Option<(usize, usize)> {
    let vpn2 = sv39::vpn2(va);
    let vpn1 = sv39::vpn1(va);
    let vpn0 = sv39::vpn0(va);

    let l2 = pt_page_mut(root_pt);

    // L2 -> L1
    let l1_phys = if !l2[vpn2].is_valid() {
        if !alloc {
            return None;
        }
        let new_page = buddy::alloc_page()?;
        let new_pt = pt_page_mut(new_page);
        for pte in new_pt.iter_mut() {
            *pte = sv39::PTE::empty();
        }
        let mut entry = sv39::PTE::empty();
        entry.set_ppn(new_page >> 12);
        entry.set_flags(false, false, false, false); // non-leaf: R=W=X=0
        l2[vpn2] = entry;
        new_page
    } else {
        l2[vpn2].phys_addr()
    };

    // L1 -> L0
    let l1 = pt_page_mut(l1_phys);
    let l0_phys = if !l1[vpn1].is_valid() {
        if !alloc {
            return None;
        }
        let new_page = buddy::alloc_page()?;
        let new_pt = pt_page_mut(new_page);
        for pte in new_pt.iter_mut() {
            *pte = sv39::PTE::empty();
        }
        let mut entry = sv39::PTE::empty();
        entry.set_ppn(new_page >> 12);
        entry.set_flags(false, false, false, false); // non-leaf: R=W=X=0
        l1[vpn1] = entry;
        new_page
    } else {
        l1[vpn1].phys_addr()
    };

    Some((l0_phys, vpn0))
}

/// Map a single 4 KiB page into a specific page table.
pub unsafe fn map_into_pt(
    root_pt: usize,
    va: usize,
    pa: usize,
    r: bool,
    w: bool,
    x: bool,
    u: bool,
) {
    let (l0_phys, idx) = walk_pt(root_pt, va, true).expect("map_into_pt: walk failed");
    let l0 = pt_page_mut(l0_phys);
    let mut pte = sv39::PTE::empty();
    pte.set_ppn(pa >> 12);
    pte.set_flags(r, w, x, u);
    pte.set_accessed(true);
    pte.set_dirty(true);
    l0[idx] = pte;
}

/// Translate virtual to physical address in a specific page table.
pub(crate) unsafe fn virt_to_phys_in_pt(root_pt: usize, va: usize) -> Option<usize> {
    let (l0_phys, idx) = walk_pt(root_pt, va, false)?;
    let l0 = pt_page_ref(l0_phys);
    let pte = l0[idx];
    if pte.is_valid() && pte.is_leaf() {
        Some(pte.phys_addr() | (va & (PAGE_SIZE - 1)))
    } else {
        None
    }
}

// ── ELF loader ──────────────────────────────────────────────────────────

/// Load an ELF64 binary into the address space described by `page_table_root`.
///
/// `page_table_root` is the **physical** address of the L2 (root) page table
/// page for the target process.  The page must already be zeroed by the caller.
///
/// Returns `(entry_point, user_stack_top)` on success, `None` on failure.
pub fn load_elf(elf_data: &[u8], page_table_root: usize) -> Option<(usize, usize)> {
    if elf_data.len() < core::mem::size_of::<Elf64Header>() {
        crate::console::puts("elf:fail_header_size\n");
        return None;
    }

    // SAFETY: we have verified elf_data is large enough to hold the header.
    let header = unsafe { &*(elf_data.as_ptr() as *const Elf64Header) };

    // Validate ELF magic and machine type
    if &header.ident[0..4] != b"\x7FELF" {
        crate::console::puts("elf:fail_magic\n");
        return None;
    }
    if header.e_machine != 243 {
        // EM_RISCV
        crate::console::puts("elf:fail_machine\n");
        return None;
    }

    let entry = header.e_entry as usize;
    let phoff = header.e_phoff as usize;
    let phentsize = header.e_phentsize as usize;
    let phnum = header.e_phnum as usize;

    // Bounds check: program headers must fit inside the binary
    if phoff + phnum * phentsize > elf_data.len() {
        crate::console::puts("elf:fail_bounds\n");
        return None;
    }

    // Iterate over all program headers
    for i in 0..phnum {
        // SAFETY: bounds checked above.
        let phdr_ptr = unsafe { elf_data.as_ptr().add(phoff + i * phentsize) };
        let phdr = unsafe { &*(phdr_ptr as *const Elf64Phdr) };

        if phdr.p_type != PT_LOAD {
            continue;
        }

        let vaddr = phdr.p_vaddr as usize;
        let filesz = phdr.p_filesz as usize;
        let memsz = phdr.p_memsz as usize;
        let offset = phdr.p_offset as usize;

        if memsz == 0 {
            continue;
        }

        let r = (phdr.p_flags & PF_R) != 0;
        let w = (phdr.p_flags & PF_W) != 0;
        let x = (phdr.p_flags & PF_X) != 0;

        // Page-align the segment bounds
        let seg_start = sv39::page_align_down(vaddr);
        let seg_end = sv39::page_align_up(vaddr + memsz);

        // ── Allocate and map physical pages ──────────────────────
        for page_va in (seg_start..seg_end).step_by(PAGE_SIZE) {
            let phys = buddy::alloc_page().expect("ELF: out of memory");

            // Zero the page via its kernel virtual address
            let kva = sv39::pa_to_kva(phys);
            unsafe {
                core::ptr::write_bytes(kva as *mut u8, 0, PAGE_SIZE);
            }

            // Map into the process page table
            unsafe {
                map_into_pt(page_table_root, page_va, phys, r, w, x, true);
            }
        }

        // ── Copy file data into the mapped pages ────────────────
        if filesz > 0 && offset + filesz <= elf_data.len() {
            let mut copy_va = vaddr;
            let mut remaining = filesz;
            while remaining > 0 {
                let page_off = copy_va & (PAGE_SIZE - 1);
                let chunk = core::cmp::min(remaining, PAGE_SIZE - page_off);

                // SAFETY: the page has been mapped above, so walk with
                // alloc=false will succeed.
                let phys = unsafe {
                    virt_to_phys_in_pt(page_table_root, copy_va)
                        .expect("ELF: virt_to_phys failed during load")
                };

                let dst = sv39::pa_to_kva(phys) as *mut u8;
                let src = unsafe { elf_data.as_ptr().add(offset + (copy_va - vaddr)) };
                unsafe {
                    core::ptr::copy_nonoverlapping(src, dst, chunk);
                }
                copy_va += chunk;
                remaining -= chunk;
            }
        }
        // BSS (memsz - filesz) is already zeroed from the page-zeroing loop above.
    }

    // ── Allocate user stack ──────────────────────────────────────
    // Place the stack at the very top of the user address space
    // (0x0000_0000 – 0x0000_003F_FFFF_FFFF), growing downward.
    // The last valid user page is at 0x0000_003F_FFFF_F000 (VPN2=255,
    // VPN1=511, VPN0=511).  stack_bottom + PAGE_SIZE would overflow
    // past 2^38 (bit 38 = 1), producing a non-canonical Sv39 address,
    // so we set user_sp 16 bytes below the boundary for safety.
    let stack_bottom = 0x0000_003F_FFFF_F000; // last valid user page
    let stack_phys = buddy::alloc_page()?;
    unsafe {
        let kva = sv39::pa_to_kva(stack_phys);
        core::ptr::write_bytes(kva as *mut u8, 0, PAGE_SIZE);
        map_into_pt(
            page_table_root,
            stack_bottom,
            stack_phys,
            true,  // read
            true,  // write
            false, // no execute
            true,  // user
        );
    }

    // V21.10: Enforce W^X — clear X on any page with both W and X set
    let fixed = crate::mem::sv39::force_wxorx(page_table_root);
    if fixed > 0 {
        crate::println!("W^X: fixed {} pages during ELF load", fixed);
    }

    Some((entry, stack_bottom + PAGE_SIZE - 16))
}

// ── V31: Transactional ELF loading using TxMMU ─────────────────────────────

/// Information about a single mapped page in the transactional loader.
#[derive(Clone, Copy)]
struct MapEntry {
    va: usize,
    pa: usize,
    flags: u8,
}

/// Information about one PT_LOAD segment for data-copy phase.
#[derive(Clone, Copy)]
struct SegInfo {
    data_offset: usize,
    vaddr: usize,
    filesz: usize,
    page_start_idx: usize,
    page_count: usize,
}

/// Load an ELF64 binary using the Transactional MMU.
///
/// All PT_LOAD segments are mapped atomically: either every page in every
/// segment is mapped, or no pages are mapped (if any allocation fails).
///
/// Returns `(entry_point, user_stack_top)` on success, `None` on failure.
pub fn load_elf_transactional(
    elf_data: &[u8],
    page_table_root: usize,
) -> Option<(usize, usize)> {
    use crate::mem::txmmu::{TxMMU, FLAG_R, FLAG_W, FLAG_X, FLAG_U};
    use crate::mem::layout::PAGE_SIZE;

    if elf_data.len() < core::mem::size_of::<Elf64Header>() {
        crate::console::puts("elf_tx:fail_header_size\n");
        return None;
    }

    // SAFETY: bounds checked above.
    let header = unsafe { &*(elf_data.as_ptr() as *const Elf64Header) };

    if &header.ident[0..4] != b"\x7FELF" {
        crate::console::puts("elf_tx:fail_magic\n");
        return None;
    }
    if header.e_machine != 243 {
        crate::console::puts("elf_tx:fail_machine\n");
        return None;
    }

    let entry = header.e_entry as usize;
    let phoff = header.e_phoff as usize;
    let phentsize = header.e_phentsize as usize;
    let phnum = header.e_phnum as usize;

    if phoff + phnum * phentsize > elf_data.len() {
        crate::console::puts("elf_tx:fail_bounds\n");
        return None;
    }

    // ── Fixed-size working area (no_std, no heap) ──────────────────────
    let mut va_buf = [0usize; 512];
    let mut pa_buf = [0usize; 512];
    let mut flags_buf = [0u8; 512];
    let mut num_entries = 0usize;

    let mut segs = [SegInfo {
        data_offset: 0,
        vaddr: 0,
        filesz: 0,
        page_start_idx: 0,
        page_count: 0,
    }; 8];
    let mut num_segs = 0usize;

    // ── Phase 1: Parse headers, allocate physical pages, collect maps ──
    for i in 0..phnum {
        // SAFETY: bounds checked above.
        let phdr_ptr = unsafe { elf_data.as_ptr().add(phoff + i * phentsize) };
        let phdr = unsafe { &*(phdr_ptr as *const Elf64Phdr) };

        if phdr.p_type != PT_LOAD {
            continue;
        }

        let vaddr = phdr.p_vaddr as usize;
        let filesz = phdr.p_filesz as usize;
        let memsz = phdr.p_memsz as usize;
        let offset = phdr.p_offset as usize;

        if memsz == 0 {
            continue;
        }

        // Build flags byte: R=1, W=2, X=4, U=8
        let r = (phdr.p_flags & PF_R) != 0;
        let w = (phdr.p_flags & PF_W) != 0;
        let x = (phdr.p_flags & PF_X) != 0;
        let flags: u8 = (if r { FLAG_R } else { 0 })
            | (if w { FLAG_W } else { 0 })
            | (if x { FLAG_X } else { 0 })
            | FLAG_U; // user-accessible

        let seg_start = crate::mem::sv39::page_align_down(vaddr);
        let seg_end = crate::mem::sv39::page_align_up(vaddr + memsz);

        let page_start_idx = num_entries;
        let page_count = (seg_end - seg_start) / PAGE_SIZE;

        if num_entries + page_count > 512 || num_segs >= 8 {
            crate::console::puts("elf_tx: overflow\n");
            return None;
        }

        // Allocate and zero each physical page.
        for page_va in (seg_start..seg_end).step_by(PAGE_SIZE) {
            let phys = crate::mem::buddy::alloc_page()?;
            unsafe {
                core::ptr::write_bytes(
                    crate::mem::sv39::pa_to_kva(phys) as *mut u8,
                    0,
                    PAGE_SIZE,
                );
            }
            va_buf[num_entries] = page_va;
            pa_buf[num_entries] = phys;
            flags_buf[num_entries] = flags;
            num_entries += 1;
        }

        segs[num_segs] = SegInfo {
            data_offset: offset,
            vaddr,
            filesz,
            page_start_idx,
            page_count,
        };
        num_segs += 1;
    }

    // ── Phase 2: Queue all mappings in a single transaction ────────────
    {
        let mut tx = TxMMU::begin(page_table_root);
        for i in 0..num_entries {
            tx.map(va_buf[i], pa_buf[i], flags_buf[i]).ok()?;
        }

        // Check for conflicts before committing.
        if tx.rollback_on_conflict() {
            // Conflict detected — free allocated pages and retry is up to caller.
            for i in 0..num_entries {
                crate::mem::buddy::free_page(pa_buf[i], 0);
            }
            return None;
        }

        tx.commit().ok()?;
    }

    // ── Phase 3: Copy ELF segment data into the mapped pages ───────────
    for si in 0..num_segs {
        let seg = &segs[si];
        if seg.filesz == 0 {
            continue;
        }
        if seg.data_offset + seg.filesz > elf_data.len() {
            continue;
        }

        let mut remaining = seg.filesz;
        let mut copy_va = seg.vaddr;
        while remaining > 0 {
            let page_off = copy_va & (PAGE_SIZE - 1);
            let chunk = core::cmp::min(remaining, PAGE_SIZE - page_off);

            // Locate the physical page for this VA.
            let page_idx = (copy_va - va_buf[seg.page_start_idx]) / PAGE_SIZE;
            let actual_idx = seg.page_start_idx + page_idx;
            let phys = pa_buf[actual_idx];

            let dst =
                unsafe { crate::mem::sv39::pa_to_kva(phys + page_off) as *mut u8 };
            let src = unsafe {
                elf_data
                    .as_ptr()
                    .add(seg.data_offset + (copy_va - seg.vaddr))
            };
            unsafe {
                core::ptr::copy_nonoverlapping(src, dst, chunk);
            }
            copy_va += chunk;
            remaining -= chunk;
        }
    }

    // ── Phase 4: Allocate and map the user stack ───────────────────────
    let stack_bottom = 0x0000_003F_FFFF_F000usize;
    let stack_phys = crate::mem::buddy::alloc_page()?;
    unsafe {
        core::ptr::write_bytes(
            crate::mem::sv39::pa_to_kva(stack_phys) as *mut u8,
            0,
            PAGE_SIZE,
        );
    }

    {
        let mut tx = TxMMU::begin(page_table_root);
        // Stack: R=1, W=1, X=0, U=1 => flags = 1 | 2 | 8 = 11 (0b1011)
        tx.map(stack_bottom, stack_phys, FLAG_R | FLAG_W | FLAG_U)
            .ok()?;
        tx.commit().ok()?;
    }

    // V21.10: Enforce W^X
    let fixed = crate::mem::sv39::force_wxorx(page_table_root);
    if fixed > 0 {
        crate::println!("W^X: fixed {} pages during transactional ELF load", fixed);
    }

    Some((entry, stack_bottom + PAGE_SIZE - 16))
}

/// Map a physical page (e.g. MMIO region) into a process's page table.
/// Returns the virtual address at which it was mapped.
/// The VA is computed as 0x4000_0000 + offset from the physical address.
pub fn map_phys_to_user(root_pt: usize, phys: usize, _size: usize) -> usize {
    let va = 0x4000_0000 + (phys & 0xFFF);
    unsafe {
        map_into_pt(root_pt, va, phys, true, true, false, true);
    }
    va
}
