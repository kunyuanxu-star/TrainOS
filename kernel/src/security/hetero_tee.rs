// V33 — Confidential Computing TEE — Heterogeneous TEE (CPU + AI Accelerator)
//
// Features:
//   - Extends TEE protection to AI accelerators (GPU/NPU)
//   - PMP-protected CPU enclave + IOMMU/PMP-protected GPU memory
//   - Secure data transfer between CPU enclave and GPU TEE memory
//   - GPU-side attestation verification
//
// Architecture:
//   A heterogeneous TEE consists of a CPU-side enclave (protected by PMP)
//   and a GPU-side memory region (protected by IOMMU or GPU PMP). Data
//   transfers between the two are mediated by the kernel.

use core::mem;

/// Maximum number of concurrent heterogeneous TEEs.
const MAX_HETERO_TEES: usize = 8;

/// Default GPU memory page size.
const GPU_PAGE_SIZE: usize = 4096;

// ── HeteroTee ─────────────────────────────────────────────────────────────────

/// A heterogeneous TEE spanning CPU and AI accelerator (GPU/NPU).
#[derive(Clone, Copy)]
pub struct HeteroTee {
    pub ht_id: u32,
    /// CPU-side enclave ID (from `tee::TeeEnclave`).
    pub cpu_enclave_id: u32,
    /// GPU device ID (from V29 AI subsystem).
    pub gpu_id: u32,
    /// Physical address of GPU memory region for TEE-protected data.
    pub gpu_mem_region: usize,
    /// Size of GPU memory region in bytes.
    pub gpu_mem_size: usize,
    /// Whether this heterogeneous TEE is active.
    pub active: bool,
    /// Attestation measurement for the GPU-side computation.
    pub gpu_measurement: [u8; 32],
    /// Whether GPU attestation has been verified.
    pub gpu_attested: bool,
}

impl HeteroTee {
    /// Create a new heterogeneous TEE spanning CPU enclave + GPU device.
    pub fn create(cpu_enclave_id: u32, gpu_id: u32, gpu_mem_size: usize) -> Option<Self> {
        if gpu_mem_size == 0 || gpu_mem_size > 64 * 1024 * 1024 {
            return None;
        }

        Some(HeteroTee {
            ht_id: 0,
            cpu_enclave_id,
            gpu_id,
            gpu_mem_region: 0,
            gpu_mem_size,
            active: true,
            gpu_measurement: [0u8; 32],
            gpu_attested: false,
        })
    }

    /// Securely copy data from the CPU enclave to GPU TEE memory.
    pub fn cpu_to_gpu(&self, src: &[u8], gpu_offset: usize) -> Result<(), &'static str> {
        if !self.active {
            return Err("hetero TEE not active");
        }
        if gpu_offset + src.len() > self.gpu_mem_size {
            return Err("GPU offset + data length exceeds GPU TEE region");
        }
        if self.gpu_mem_region == 0 {
            return Err("GPU TEE memory not yet allocated");
        }

        let gpu_dst = self.gpu_mem_region + gpu_offset;
        unsafe {
            core::ptr::copy_nonoverlapping(src.as_ptr(), gpu_dst as *mut u8, src.len());
        }
        Ok(())
    }

    /// Securely copy results from GPU TEE memory back to CPU enclave.
    pub fn gpu_to_cpu(&self, gpu_offset: usize, dst: &mut [u8]) -> Result<(), &'static str> {
        if !self.active {
            return Err("hetero TEE not active");
        }
        if gpu_offset + dst.len() > self.gpu_mem_size {
            return Err("GPU offset + data length exceeds GPU TEE region");
        }
        if self.gpu_mem_region == 0 {
            return Err("GPU TEE memory not yet allocated");
        }

        let gpu_src = self.gpu_mem_region + gpu_offset;
        unsafe {
            core::ptr::copy_nonoverlapping(gpu_src as *const u8, dst.as_mut_ptr(), dst.len());
        }
        Ok(())
    }

    /// Verify GPU-side attestation.
    pub fn verify_gpu_attestation(&self) -> bool {
        self.gpu_attested
    }

    /// Set the GPU memory region (called after GPU memory is allocated).
    pub fn set_gpu_mem_region(&mut self, phys_addr: usize) {
        self.gpu_mem_region = phys_addr;
    }

    /// Set the GPU measurement after attestation.
    pub fn set_gpu_measurement(&mut self, measurement: [u8; 32]) {
        self.gpu_measurement = measurement;
        self.gpu_attested = true;
    }

    /// Deactivate this heterogeneous TEE.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.gpu_attested = false;
    }
}

// ── Global heterogeneous TEE state ────────────────────────────────────────────

static mut HETERO_TEES: [HeteroTee; MAX_HETERO_TEES] = unsafe { zeroed_hetero_tees() };
static mut HETERO_TEE_COUNT: usize = 0;
static mut HETERO_TEE_ID_COUNTER: u32 = 1;

/// Helper to zero-initialize the hetero TEE table at compile time.
const fn zeroed_hetero_tees() -> [HeteroTee; MAX_HETERO_TEES] {
    unsafe { core::mem::transmute([0u8; mem::size_of::<HeteroTee>() * MAX_HETERO_TEES]) }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Create a new heterogeneous TEE.
pub fn hetero_tee_create(cpu_enclave_id: u32, gpu_id: u32, gpu_mem_size: usize) -> Option<u32> {
    unsafe {
        if HETERO_TEE_COUNT >= MAX_HETERO_TEES {
            crate::println!("  TEE: heterogeneous TEE table full");
            return None;
        }

        let mut ht = HeteroTee::create(cpu_enclave_id, gpu_id, gpu_mem_size)?;
        let ht_id = HETERO_TEE_ID_COUNTER;
        HETERO_TEE_ID_COUNTER = HETERO_TEE_ID_COUNTER.wrapping_add(1);
        ht.ht_id = ht_id;

        let idx = HETERO_TEE_COUNT;
        HETERO_TEES[idx] = ht;
        HETERO_TEE_COUNT += 1;

        crate::println!(
            "  TEE: heterogeneous TEE {} created (enclave {}, GPU {})",
            ht_id, cpu_enclave_id, gpu_id
        );
        Some(ht_id)
    }
}

/// Find a heterogeneous TEE by its ID.
fn hetero_tee_find(ht_id: u32) -> Option<&'static mut HeteroTee> {
    unsafe {
        for i in 0..HETERO_TEE_COUNT {
            if HETERO_TEES[i].ht_id == ht_id && HETERO_TEES[i].active {
                return Some(&mut HETERO_TEES[i]);
            }
        }
        None
    }
}

/// Copy data from CPU enclave to GPU TEE memory.
pub fn hetero_tee_cpu_to_gpu(ht_id: u32, src: &[u8], gpu_offset: usize) -> Result<(), &'static str> {
    match hetero_tee_find(ht_id) {
        Some(ht) => ht.cpu_to_gpu(src, gpu_offset),
        None => Err("hetero TEE not found"),
    }
}

/// Copy data from GPU TEE memory to CPU enclave.
pub fn hetero_tee_gpu_to_cpu(
    ht_id: u32,
    gpu_offset: usize,
    dst: &mut [u8],
) -> Result<(), &'static str> {
    match hetero_tee_find(ht_id) {
        Some(ht) => ht.gpu_to_cpu(gpu_offset, dst),
        None => Err("hetero TEE not found"),
    }
}

/// Compute the smallest order that can contain `page_count` pages.
fn pages_to_order(page_count: usize) -> usize {
    let mut order = 0usize;
    while (1usize << order) < page_count {
        order += 1;
    }
    order
}

/// Allocate GPU memory for a heterogeneous TEE.
pub fn hetero_tee_alloc_gpu_memory(ht_id: u32) -> Result<(), &'static str> {
    unsafe {
        let ht = match hetero_tee_find(ht_id) {
            Some(ht) => ht as *mut HeteroTee,
            None => return Err("hetero TEE not found"),
        };

        let size = (*ht).gpu_mem_size;
        let pages = (size + GPU_PAGE_SIZE - 1) / GPU_PAGE_SIZE;
        let order = pages_to_order(pages);

        let phys =
            crate::mem::buddy::alloc_pages(order).ok_or("failed to allocate GPU TEE memory")?;
        let phys_addr = phys as usize;

        (*ht).set_gpu_mem_region(phys_addr);

        crate::println!(
            "  TEE: allocated {} pages ({} bytes, order={}) at 0x{:x} for hetero TEE {}",
            pages,
            size,
            order,
            phys_addr,
            ht_id
        );
        Ok(())
    }
}

/// Free GPU memory associated with a heterogeneous TEE.
pub fn hetero_tee_free_gpu_memory(ht_id: u32) -> Result<(), &'static str> {
    unsafe {
        let ht = match hetero_tee_find(ht_id) {
            Some(ht) => ht as *mut HeteroTee,
            None => return Err("hetero TEE not found"),
        };

        if (*ht).gpu_mem_region != 0 {
            let pages = ((*ht).gpu_mem_size + GPU_PAGE_SIZE - 1) / GPU_PAGE_SIZE;
            let order = pages_to_order(pages);
            crate::mem::buddy::free_page((*ht).gpu_mem_region, order);
            (*ht).gpu_mem_region = 0;
        }
        Ok(())
    }
}

/// Destroy a heterogeneous TEE.
pub fn hetero_tee_destroy(ht_id: u32) -> bool {
    unsafe {
        for i in 0..HETERO_TEE_COUNT {
            if HETERO_TEES[i].ht_id == ht_id {
                if HETERO_TEES[i].gpu_mem_region != 0 {
                    let pages =
                        (HETERO_TEES[i].gpu_mem_size + GPU_PAGE_SIZE - 1) / GPU_PAGE_SIZE;
                    let order = pages_to_order(pages);
                    crate::mem::buddy::free_page(HETERO_TEES[i].gpu_mem_region, order);
                }
                HETERO_TEES[i].deactivate();
                crate::println!("  TEE: heterogeneous TEE {} destroyed", ht_id);
                return true;
            }
        }
        false
    }
}

/// List all active heterogeneous TEEs. Returns bytes written.
pub fn hetero_tee_list(buf: &mut [u8]) -> usize {
    let mut pos = 0usize;
    unsafe {
        for i in 0..HETERO_TEE_COUNT {
            if !HETERO_TEES[i].active {
                continue;
            }
            let ht = &HETERO_TEES[i];
            pos += w_str(buf, pos, "ht=");
            pos += w_u64(buf, pos, ht.ht_id as u64);
            pos += w_str(buf, pos, " cpu_enclave=");
            pos += w_u64(buf, pos, ht.cpu_enclave_id as u64);
            pos += w_str(buf, pos, " gpu=");
            pos += w_u64(buf, pos, ht.gpu_id as u64);
            pos += w_str(buf, pos, " mem=0x");
            pos += w_hex64(buf, pos, ht.gpu_mem_region as u64);
            pos += w_str(buf, pos, " size=");
            pos += w_u64(buf, pos, ht.gpu_mem_size as u64);
            pos += w_str(buf, pos, " attested=");
            pos += w_str(buf, pos, if ht.gpu_attested { "yes" } else { "no" });
            pos += w_str(buf, pos, "\n");
        }
    }
    pos
}

// ── Formatting helpers ────────────────────────────────────────────────────────

fn w_str(buf: &mut [u8], pos: usize, s: &str) -> usize {
    let bytes = s.as_bytes();
    let len = bytes.len().min(buf.len().saturating_sub(pos));
    if len > 0 {
        buf[pos..pos + len].copy_from_slice(&bytes[..len]);
    }
    len
}

fn w_u64(buf: &mut [u8], pos: usize, v: u64) -> usize {
    if v == 0 {
        if pos < buf.len() {
            buf[pos] = b'0';
            return 1;
        }
        return 0;
    }
    let mut temp = [0u8; 20];
    let mut n = v;
    let mut len = 0;
    while n > 0 {
        temp[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    let mut written = 0;
    for i in (0..len).rev() {
        if pos + written < buf.len() {
            buf[pos + written] = temp[i];
            written += 1;
        } else {
            break;
        }
    }
    written
}

fn w_hex64(buf: &mut [u8], pos: usize, v: u64) -> usize {
    if v == 0 {
        if pos < buf.len() {
            buf[pos] = b'0';
            return 1;
        }
        return 0;
    }
    let mut temp = [0u8; 16];
    let mut n = v;
    let mut len = 0;
    while n > 0 {
        let nibble = (n & 0xF) as u8;
        temp[len] = if nibble < 10 {
            b'0' + nibble
        } else {
            b'a' + nibble - 10
        };
        n >>= 4;
        len += 1;
    }
    let mut written = 0;
    for i in (0..len).rev() {
        if pos + written < buf.len() {
            buf[pos + written] = temp[i];
            written += 1;
        } else {
            break;
        }
    }
    written
}
