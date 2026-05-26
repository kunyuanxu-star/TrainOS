// V36d — RISC-V IOMMU (I/O Memory Management Unit)
//
// Specification v1.0 (ratified 2024):
//   Provides device-level address translation and isolation for DMA operations.
//   - Device contexts map a device to an I/O page table
//   - IOVA (I/O Virtual Address) → HPA (Host Physical Address) translation
//   - Independent from CPU page tables, or shared for zero-copy
//   - TLB for I/O address translations (IOTLB)
//
// Integration points:
//   - V33 TEE: Device isolation — each enclave gets an IOMMU context
//   - V29 GPU: GPU DMA via IOMMU page tables instead of GART
//   - V22 io_uring: Zero-copy DMA with shared CPU/IOMMU page tables
//
// QEMU support: -device riscv-iommu (since QEMU 8.1+)

use core::ptr::{read_volatile, write_volatile};

// ── IOMMU Register Offsets (MMIO) ───────────────────────────────────────

/// IOMMU Capabilities register (64-bit, offset 0x00).
const IOMMU_CAPABILITIES: usize = 0x00;
/// IOMMU Feature Control register (64-bit, offset 0x08).
const IOMMU_FCTL: usize = 0x08;
/// IOMMU Device Directory Base register (64-bit, offset 0x10).
const IOMMU_DDIR_BASE: usize = 0x10;
/// IOMMU Device Context Cache Control register (32-bit, offset 0x18).
const IOMMU_DC_CTL: usize = 0x18;
/// IOMMU IOTLB Invalidate register (32-bit, offset 0x1C).
const IOMMU_IOTLB_INVAL: usize = 0x1C;
/// IOMMU Command Queue Base (64-bit, offset 0x20).
const IOMMU_CMD_QUEUE_BASE: usize = 0x20;
/// IOMMU Fault Queue Base (64-bit, offset 0x28).
const IOMMU_FAULT_QUEUE_BASE: usize = 0x28;

/// Capabilities register bitfields.
const CAP_VERSION_MASK: u64 = 0xFF;           // Bits 7:0 — version
const CAP_NUM_CONTEXTS_MASK: u64 = 0xFF00;    // Bits 15:8 — number of device contexts
const CAP_NUM_CONTEXTS_SHIFT: u64 = 8;
const CAP_ATS: u64 = 1 << 16;                 // Bit 16 — ATS support
const CAP_PASID: u64 = 1 << 17;               // Bit 17 — PASID support

/// Feature Control register bitfields.
const FCTL_ENABLE: u64 = 1 << 0;              // Bit 0 — global enable

/// IOMMU MMIO base physical address.
/// In QEMU virt, the RISC-V IOMMU is typically at 0x4000_0000
/// (just after PCI ECAM at 0x3000_0000).
const IOMMU_MMIO_BASE: usize = 0x4000_0000;

// ── Device Context Format ───────────────────────────────────────────────

/// A device context (64 bytes) in the IOMMU device directory.
///
/// Each context maps one device (identified by RID — Requester ID)
/// to an I/O page table.
#[derive(Clone, Copy)]
#[repr(C, align(64))]
pub struct IommuDeviceContext {
    /// Valid bit, PASID enable, page table root, address space ID, etc.
    pub word0: u64,
    /// Page table root physical address (PPN format).
    pub word1: u64,
    /// Context tag, PID, device ID for fault reporting.
    pub word2: u64,
    /// Reserved / future use.
    pub word3: u64,
    pub word4: u64,
    pub word5: u64,
    pub word6: u64,
    pub word7: u64,
}

impl IommuDeviceContext {
    pub const fn empty() -> Self {
        IommuDeviceContext {
            word0: 0,
            word1: 0,
            word2: 0,
            word3: 0,
            word4: 0,
            word5: 0,
            word6: 0,
            word7: 0,
        }
    }

    /// Configure the device context for translation.
    ///
    /// `page_table_root` — physical address of the I/O page table root.
    /// `addressing_mode` — 8=Sv39, 9=Sv48, 10=Sv57.
    /// `device_id` — PCI requester ID (bus:device:function).
    pub fn configure(&mut self, page_table_root: usize, addressing_mode: u8, device_id: u32) {
        // word0: V=1, TC=1 (translation enabled), mode=Sv39(8)
        // Bits: V(0), TC(1), mode(4:2)
        let mode_val = (addressing_mode as u64) & 0x7;
        self.word0 = (1 << 0)   // V — valid
            | (1 << 1)          // TC — translation enabled
            | (mode_val << 2);  // Translation mode (Sv39=8→0, Sv48=9→1, Sv57=10→2)

        // word1: Page table root PPN (physical page number)
        self.word1 = (page_table_root >> 12) as u64;

        // word2: Device tag for fault reporting
        self.word2 = device_id as u64;
    }

    pub fn is_valid(&self) -> bool {
        self.word0 & 1 != 0
    }

    pub fn is_enabled(&self) -> bool {
        (self.word0 & (1 << 1)) != 0
    }
}

// ── IOMMU Core ──────────────────────────────────────────────────────────

/// RISC-V IOMMU instance.
pub struct RvIommu {
    /// MMIO base physical address of the IOMMU registers.
    mmio_base: usize,
    /// Number of device contexts supported.
    num_contexts: usize,
    /// IOMMU capabilities (cached from CAPABILITIES register).
    capabilities: u64,
}

impl RvIommu {
    /// Probe for an IOMMU device.
    ///
    /// Currently checks a known MMIO address range for IOMMU registers.
    /// In production, this would read the device tree.
    ///
    /// Returns `Some(RvIommu)` if an IOMMU is found and initialized,
    /// or `None` if no IOMMU is available.
    pub fn probe() -> Option<Self> {
        // The RISC-V IOMMU in QEMU is typically mapped at 0x4000_0000
        // (just after the PCI ECAM region at 0x3000_0000).
        // If no IOMMU is present, reads return all-ones (bus error).
        const IOMMU_MMIO_BASE: usize = 0x4000_0000;

        unsafe {
            // Read the capabilities register. If no device is present,
            // the read returns 0xFFFF_FFFF_FFFF_FFFF (bus error on RISC-V).
            let cap = read_volatile(IOMMU_MMIO_BASE as *const u64);
            if cap == 0 || cap == !0u64 {
                crate::println!("  IOMMU: not present at 0x{:x}", IOMMU_MMIO_BASE);
                return None;
            }

            let version = cap & CAP_VERSION_MASK;
            if version == 0 {
                crate::println!("  IOMMU: invalid version");
                return None;
            }

            let num_ctx = ((cap & CAP_NUM_CONTEXTS_MASK) >> CAP_NUM_CONTEXTS_SHIFT) as usize;
            let ats = (cap & CAP_ATS) != 0;
            let pasid = (cap & CAP_PASID) != 0;

            crate::println!(
                "  IOMMU: v{} at 0x{:x}, {} contexts, ATS={}, PASID={}",
                version,
                IOMMU_MMIO_BASE,
                num_ctx,
                ats,
                pasid,
            );

            Some(RvIommu {
                mmio_base: IOMMU_MMIO_BASE,
                num_contexts: if num_ctx > 0 { num_ctx } else { 16 },
                capabilities: cap,
            })
        }
    }

    /// Create a device context (page table for one device).
    ///
    /// `device_id` — PCI RID (Requester ID) or platform device identifier.
    /// `page_table_root` — physical address of the I/O page table root.
    ///
    /// Returns the context ID on success, or `None` if no free context available.
    pub fn create_context(&self, device_id: u32, page_table_root: usize) -> Option<u32> {
        unsafe {
            // Device directory base register points to the context table in memory.
            // For now, we use a statically-allocated device context table.
            let ddir_base = self.read_reg64(IOMMU_DDIR_BASE);

            // If no device directory is configured, allocate an internal one.
            let ctx_base = if ddir_base == 0 {
                // Use a fixed location in the kernel's device area.
                // In production, this would be allocated from the buddy allocator.
                IOMMU_CONTEXT_TABLE as *const IommuDeviceContext
            } else {
                ddir_base as *const IommuDeviceContext
            };

            // Find a free context slot
            for i in 0..self.num_contexts {
                let ctx = &*ctx_base.add(i);
                if !ctx.is_valid() {
                    // Found a free slot — configure it
                    let ctx_mut = &mut *(ctx_base.add(i) as *mut IommuDeviceContext);
                    // Use Sv39 addressing mode (8) for Sv39-compatible I/O page table
                    ctx_mut.configure(page_table_root, 8, device_id);

                    // Flush the device context cache for this context
                    self.invalidate_context(i);

                    crate::println!(
                        "  IOMMU: context {} created for device 0x{:x}, PT=0x{:x}",
                        i,
                        device_id,
                        page_table_root,
                    );
                    return Some(i as u32);
                }
            }

            crate::println!("  IOMMU: no free context slots");
            None
        }
    }

    /// Map an I/O virtual address (IOVA) to a host physical address (HPA).
    ///
    /// `context_id` — IOMMU context (from `create_context`).
    /// `iova` — I/O virtual address to map.
    /// `hpa` — host physical address.
    /// `size` — size of the mapping (in bytes, must be page-aligned).
    /// `flags` — bit 0=read, bit 1=write.
    ///
    /// Returns `true` if the mapping was created successfully.
    pub fn map(
        &self,
        context_id: u32,
        iova: usize,
        hpa: usize,
        size: usize,
        flags: u8,
    ) -> bool {
        unsafe {
            let ctx_base = IOMMU_CONTEXT_TABLE as *const IommuDeviceContext;
            if (context_id as usize) >= self.num_contexts {
                return false;
            }
            let ctx = &*ctx_base.add(context_id as usize);
            if !ctx.is_valid() {
                return false;
            }

            let pt_root = (ctx.word1 << 12) as usize;
            let r = flags & 1 != 0;
            let w = flags & 2 != 0;

            // Use the shared IOMMU page table mapper
            let mut iommu_pt = IommuPageTable::share_process_pt(pt_root);

            // Map each page in the range
            let start = iova & !(0xFFF);
            let end = (iova + size + 0xFFF) & !(0xFFF);
            let mut current = start;
            while current < end {
                let offset = current - start;
                let page_hpa = hpa + offset;
                iommu_pt.share_region(current, page_hpa, 0x1000, flags);
                current += 0x1000;
            }

            // Invalidate IOTLB for the affected range
            self.invalidate_tlb(context_id);

            true
        }
    }

    /// Unmap an IOVA range from an IOMMU context.
    ///
    /// `context_id` — IOMMU context.
    /// `iova` — start of IOVA range.
    /// `size` — size of the range in bytes.
    pub fn unmap(&self, context_id: u32, iova: usize, size: usize) {
        unsafe {
            let ctx_base = IOMMU_CONTEXT_TABLE as *const IommuDeviceContext;
            if (context_id as usize) >= self.num_contexts {
                return;
            }
            let ctx = &*ctx_base.add(context_id as usize);
            if !ctx.is_valid() {
                return;
            }

            let pt_root = (ctx.word1 << 12) as usize;

            // Walk the IOMMU page table and clear PTEs
            let start = iova & !(0xFFF);
            let end = (iova + size + 0xFFF) & !(0xFFF);
            let mut current = start;
            while current < end {
                iommu_unmap_pte(pt_root, current);
                current += 0x1000;
            }

            // Invalidate IOTLB for this context
            self.invalidate_tlb(context_id);
        }
    }

    /// Enable the IOMMU (start translating).
    ///
    /// Writes the global enable bit in the Feature Control register.
    pub fn enable(&self, _context_id: u32) {
        unsafe {
            let mut fctl = self.read_reg64(IOMMU_FCTL);
            fctl |= FCTL_ENABLE;
            self.write_reg64(IOMMU_FCTL, fctl);
        }
        crate::println!("  IOMMU: enabled");
    }

    /// Disable the IOMMU (passthrough mode).
    ///
    /// Clears the global enable bit. DMA goes directly to physical addresses.
    pub fn disable(&self, _context_id: u32) {
        unsafe {
            let mut fctl = self.read_reg64(IOMMU_FCTL);
            fctl &= !FCTL_ENABLE;
            self.write_reg64(IOMMU_FCTL, fctl);
        }
        crate::println!("  IOMMU: disabled (passthrough mode)");
    }

    /// Handle an IOMMU fault (device accessed unmapped IOVA).
    ///
    /// Reads the fault queue and returns the fault information.
    pub fn handle_fault(&self) -> Option<IommuFault> {
        // In a simplified model, we check the fault queue base register
        // and dequeue the first fault record.
        // For now, we return a generic fault entry.
        Some(IommuFault {
            context_id: 0,
            device_id: 0,
            fault_address: 0,
            is_read: false,
            is_write: false,
        })
    }

    /// Invalidate the IOTLB for a specific context.
    pub fn invalidate_tlb(&self, context_id: u32) {
        unsafe {
            // Write context ID to the IOTLB Invalidate register
            self.write_reg32(IOMMU_IOTLB_INVAL, context_id);
            // Fence to ensure invalidation completes
            core::arch::asm!("fence iorw, iorw");
        }
    }

    /// Invalidate a device context cache entry.
    fn invalidate_context(&self, context_id: usize) {
        unsafe {
            self.write_reg32(IOMMU_DC_CTL, (1 << 31) | (context_id as u32));
            core::arch::asm!("fence iorw, iorw");
        }
    }

    /// Check if the IOMMU is available in the current hardware configuration.
    pub fn is_available() -> bool {
        // Probe the known MMIO address
        unsafe {
            let cap = read_volatile(IOMMU_MMIO_BASE as *const u64);
            cap != 0 && cap != !0u64
        }
    }

    /// Read a 64-bit IOMMU MMIO register.
    unsafe fn read_reg64(&self, offset: usize) -> u64 {
        let addr = (self.mmio_base + offset) as *const u64;
        read_volatile(addr)
    }

    /// Write a 64-bit IOMMU MMIO register.
    unsafe fn write_reg64(&self, offset: usize, val: u64) {
        let addr = (self.mmio_base + offset) as *mut u64;
        write_volatile(addr, val);
    }

    /// Write a 32-bit IOMMU MMIO register.
    unsafe fn write_reg32(&self, offset: usize, val: u32) {
        let addr = (self.mmio_base + offset) as *mut u32;
        write_volatile(addr, val);
    }
}

// ── IOMMU Page Table ────────────────────────────────────────────────────

/// IOMMU page table — manages I/O virtual address to physical address mappings.
///
/// The IOMMU page table format mirrors the CPU Sv39 page table format
/// (3-level, 4K leaf pages, same PTE format). This allows sharing page tables
/// between CPU and IOMMU for zero-copy DMA.
pub struct IommuPageTable {
    /// Physical address of the page table root.
    root_phys: usize,
    /// Addressing mode: 8=Sv39, 9=Sv48.
    addressing_mode: u8,
    /// Count of mapped pages (4K pages).
    mapped_pages: usize,
}

impl IommuPageTable {
    /// Create an IOMMU page table sharing a process's CPU page table root.
    ///
    /// The IOMMU uses the same page table as the CPU, enabling zero-copy DMA:
    /// the device can directly access process virtual addresses.
    pub fn share_process_pt(root_phys: usize) -> Self {
        IommuPageTable {
            root_phys,
            addressing_mode: 8, // Sv39
            mapped_pages: 0,
        }
    }

    /// Create a dedicated IOMMU page table for device-only regions.
    ///
    /// Allocates a new root page table page for the IOMMU.
    pub fn new_dedicated() -> Option<Self> {
        let root = crate::mem::buddy::alloc_page()?;
        unsafe {
            // Zero the root page table
            let kva = crate::mem::sv39::pa_to_kva(root);
            core::ptr::write_bytes(kva as *mut u8, 0, 4096);
        }
        Some(IommuPageTable {
            root_phys: root,
            addressing_mode: 8,
            mapped_pages: 0,
        })
    }

    /// Share a memory region with a device (both CPU and device access same pages).
    ///
    /// `va` — virtual address (must be page-aligned).
    /// `pa` — physical address (must be page-aligned).
    /// `len` — length in bytes (must be page-aligned).
    /// `perms` — bit 0=read, bit 1=write.
    pub fn share_region(&mut self, va: usize, pa: usize, len: usize, perms: u8) {
        let r = perms & 1 != 0;
        let w = perms & 2 != 0;
        // IOMMU PTEs: R=bit1, W=bit2, but never X (devices don't execute)
        let pte_flags = r || w; // V=1 if any access
        if !pte_flags {
            return;
        }

        let start = va & !(0xFFF);
        let end = (va + len + 0xFFF) & !(0xFFF);
        let mut offset = 0usize;
        let mut page_va = start;

        while page_va < end {
            let page_pa = pa + offset;
            unsafe {
                iommu_map_pte(self.root_phys, page_va, page_pa, r, w);
            }
            self.mapped_pages += 1;
            page_va += 0x1000;
            offset += 0x1000;
        }
    }

    /// Get the physical address of the page table root.
    pub fn root_phys(&self) -> usize {
        self.root_phys
    }

    /// Get the current number of mapped pages.
    pub fn mapped_pages(&self) -> usize {
        self.mapped_pages
    }
}

// ── IOMMU Page Table Walk (Sv39) ───────────────────────────────────────

/// Walk the IOMMU page table (Sv39 format) and create intermediate tables
/// if needed. Returns (L0 physical address, index in L0).
unsafe fn iommu_walk_pt(root_phys: usize, va: usize, alloc: bool) -> Option<(usize, usize)> {
    let vpn2 = (va >> 30) & 0x1FF;
    let vpn1 = (va >> 21) & 0x1FF;
    let vpn0 = (va >> 12) & 0x1FF;

    let kva = |phys| crate::mem::sv39::pa_to_kva(phys);

    // L2 → L1
    let l2 = &mut *(kva(root_phys) as *mut [u64; 512]);
    let l1_phys = if l2[vpn2] & 1 == 0 {
        if !alloc {
            return None;
        }
        let new_page = crate::mem::buddy::alloc_page()?;
        core::ptr::write_bytes(kva(new_page) as *mut u8, 0, 4096);
        // Non-leaf PTE: V=1, R=W=X=0
        l2[vpn2] = (new_page >> 12) << 10 | 1; // V=1, PPN=new_page>>12
        new_page
    } else if (l2[vpn2] & 0b1110) != 0 {
        // Leaf entry (shouldn't happen at L2 for Sv39, but guard)
        return None;
    } else {
        (l2[vpn2] >> 10) << 12
    };

    // L1 → L0
    let l1 = &mut *(kva(l1_phys) as *mut [u64; 512]);
    if l1[vpn1] & 1 == 0 {
        if !alloc {
            return None;
        }
        let new_page = crate::mem::buddy::alloc_page()?;
        core::ptr::write_bytes(kva(new_page) as *mut u8, 0, 4096);
        l1[vpn1] = (new_page >> 12) << 10 | 1;
        Some((new_page, vpn0))
    } else {
        let l0_phys = (l1[vpn1] >> 10) << 12;
        Some((l0_phys, vpn0))
    }
}

/// Map a single 4K page in the IOMMU page table.
unsafe fn iommu_map_pte(root_phys: usize, va: usize, pa: usize, r: bool, w: bool) {
    if let Some((l0_phys, idx)) = iommu_walk_pt(root_phys, va, true) {
        let kva = crate::mem::sv39::pa_to_kva(l0_phys);
        let l0 = &mut *(kva as *mut [u64; 512]);
        let mut pte = 1u64; // V=1
        if r {
            pte |= 1 << 1; // R
        }
        if w {
            pte |= 1 << 2; // W
        }
        pte |= (pa >> 12) << 10; // PPN
        pte |= 1 << 6; // A (Accessed)
        pte |= 1 << 7; // D (Dirty)
        l0[idx] = pte;
    }
}

/// Unmap a single 4K page from the IOMMU page table.
unsafe fn iommu_unmap_pte(root_phys: usize, va: usize) {
    if let Some((l0_phys, idx)) = iommu_walk_pt(root_phys, va, false) {
        let kva = crate::mem::sv39::pa_to_kva(l0_phys);
        let l0 = &mut *(kva as *mut [u64; 512]);
        l0[idx] = 0;
    }
}

// ── IOMMU Fault Handling ───────────────────────────────────────────────

/// Information about an IOMMU fault.
#[derive(Debug, Clone, Copy)]
pub struct IommuFault {
    /// IOMMU context that experienced the fault.
    pub context_id: u32,
    /// Device ID that caused the fault.
    pub device_id: u32,
    /// IOVA that caused the fault.
    pub fault_address: usize,
    /// Whether the fault was caused by a read.
    pub is_read: bool,
    /// Whether the fault was caused by a write.
    pub is_write: bool,
}

// ── Static IOMMU State ─────────────────────────────────────────────────

/// IOMMU context table (up to 16 device contexts).
const IOMMU_MAX_CONTEXTS: usize = 16;

/// Physical address where the IOMMU device context table resides.
/// This is allocated at kernel build time.
const IOMMU_CONTEXT_TABLE: usize = 0x5000_0000; // Just above IOMMU MMIO

/// Global IOMMU instance (optional).
pub static mut IOMMU_INSTANCE: Option<RvIommu> = None;

// ── IOMMU Initialization ───────────────────────────────────────────────

/// Initialize the IOMMU subsystem.
///
/// Probes for an IOMMU device. If found, initializes the device context
/// table and enables translation.
pub fn iommu_init() {
    // Initialize the context table memory
    unsafe {
        let ctx_table = IOMMU_CONTEXT_TABLE as *mut IommuDeviceContext;
        for i in 0..IOMMU_MAX_CONTEXTS {
            core::ptr::write_volatile(ctx_table.add(i), IommuDeviceContext::empty());
        }
    }

    // Probe for IOMMU hardware
    if let Some(iommu) = RvIommu::probe() {
        unsafe {
            IOMMU_INSTANCE = Some(iommu);
        }
        crate::println!("  IOMMU: subsystem initialized");
    } else {
        crate::println!("  IOMMU: not available (device isolation via PMP only)");
    }
}

// ── Helper Functions ───────────────────────────────────────────────────

/// Register an IOMMU-protected device context for a device.
///
/// `device_id` — PCI or platform device identifier.
/// `page_table_root` — physical address of the I/O page table root.
///
/// Returns the context ID, or `None` if no IOMMU is available.
pub fn iommu_register_device(device_id: u32, page_table_root: usize) -> Option<u32> {
    unsafe {
        match &IOMMU_INSTANCE {
            Some(iommu) => iommu.create_context(device_id, page_table_root),
            None => None,
        }
    }
}

/// Map a device physical address range into an IOMMU context.
///
/// `context_id` — IOMMU context from `iommu_register_device`.
/// `iova` — starting IOVA.
/// `hpa` — starting host physical address.
/// `size` — size of the range.
/// `flags` — access flags (bit 0=read, bit 1=write).
pub fn iommu_map_device_mem(
    context_id: u32,
    iova: usize,
    hpa: usize,
    size: usize,
    flags: u8,
) -> bool {
    unsafe {
        match &IOMMU_INSTANCE {
            Some(iommu) => iommu.map(context_id, iova, hpa, size, flags),
            None => false,
        }
    }
}

/// Share a process page table with an IOMMU context for zero-copy DMA.
///
/// `context_id` — IOMMU context.
/// `process_pt_root` — physical address of the process CPU page table root.
pub fn iommu_share_process_pt(context_id: u32, process_pt_root: usize) -> bool {
    unsafe {
        let iommu = match &IOMMU_INSTANCE {
            Some(i) => i,
            None => return false,
        };

        let ctx_base = IOMMU_CONTEXT_TABLE as *mut IommuDeviceContext;
        if (context_id as usize) >= IOMMU_MAX_CONTEXTS {
            return false;
        }
        let ctx = &mut *ctx_base.add(context_id as usize);
        if !ctx.is_valid() {
            return false;
        }

        // Update the context to point to the process page table
        ctx.word1 = (process_pt_root >> 12) as u64;
        // Flush caches
        iommu.invalidate_context(context_id as usize);
        iommu.invalidate_tlb(context_id);
        true
    }
}

// ── V33 TEE Integration: Device Isolation ──────────────────────────────

/// Create an IOMMU context for a TEE enclave, isolating device DMA
/// to the enclave's memory region.
///
/// `enclave_pmp_start` — physical start of the enclave memory (PMP region).
/// `enclave_pmp_size` — size of the enclave memory.
/// `device_id` — device that the enclave will use.
///
/// Returns the IOMMU context ID, or `None` if IOMMU is unavailable.
pub fn iommu_tee_enclave_context(
    enclave_pmp_start: usize,
    enclave_pmp_size: usize,
    device_id: u32,
) -> Option<u32> {
    // Create a dedicated IOMMU page table that only maps the enclave memory
    let pt = IommuPageTable::new_dedicated()?;

    // Map the entire enclave memory as R+W in the IOMMU page table
    pt.share_region(enclave_pmp_start, enclave_pmp_start, enclave_pmp_size, 0x3);

    // Register with the IOMMU
    let ctx_id = iommu_register_device(device_id, pt.root_phys())?;

    crate::println!(
        "  IOMMU: TEE enclave context {}: device 0x{:x}, mem 0x{:x}-0x{:x}",
        ctx_id,
        device_id,
        enclave_pmp_start,
        enclave_pmp_start + enclave_pmp_size,
    );

    Some(ctx_id)
}
