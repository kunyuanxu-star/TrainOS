// V29: AI-Native OS Subsystem
//
// Features:
//   - GPU device model with command ring, fence synchronization, MSI-X interrupts
//   - GPU command submission and MMIO fence polling
//   - GPU memory allocation (GART-like) and deallocation
//   - GPU utilization tracking
//   - Priority-based AI workload scheduling (4 levels: LOW/NORMAL/HIGH/REALTIME)
//   - Time-slicing with preemption for GPU workloads
//   - Multi-process GPU sharing (MPS) — up to 4 concurrent workloads per GPU
//   - Tensor accelerator operations and model inference pipeline

pub(crate) mod gpu_mem;
pub(crate) mod tensor;
pub(crate) mod pd_sched;
pub(crate) mod kvcache;

use gpu_mem::{alloc_region_pages, register_region, gpu_free as gpu_free_region};
use tensor::{TensorOp, TENSOR_MATMUL};
use pd_sched::{PdWorkload, PdRole, PdWorkloadState};

// ── Constants ─────────────────────────────────────────────────────────────

pub(crate) const MAX_GPU_DEVS: usize = 4;
pub(crate) const MAX_AI_QUEUES: usize = 16;  // increased for inference workloads
const MAX_CONCURRENT_PER_GPU: usize = 4;     // MPS limit
const TIME_QUANTUM_OPS: usize = 1000;        // GPU operations per time slice
const MSIX_MAX_VECTORS: usize = 4;

// Priority levels
pub(crate) const PRIO_LOW: u8 = 0;
pub(crate) const PRIO_NORMAL: u8 = 1;
pub(crate) const PRIO_HIGH: u8 = 2;
pub(crate) const PRIO_REALTIME: u8 = 3;

// Workload states
pub(crate) const WL_QUEUED: u8 = 0;
pub(crate) const WL_RUNNING: u8 = 1;
pub(crate) const WL_COMPLETED: u8 = 2;
pub(crate) const WL_FAILED: u8 = 3;
pub(crate) const WL_PREEMPTED: u8 = 4;

// ── GPU Device ────────────────────────────────────────────────────────────

/// A GPU device with full command submission, interrupt, and memory management.
#[derive(Clone, Copy)]
pub(crate) struct GpuDevice {
    pub dev_id: u32,
    pub mmio_base: usize,         // MMIO base address
    pub memory_base: usize,       // GPU memory base (GART aperture)
    pub memory_size: usize,       // GPU memory size in bytes
    pub active: bool,

    // ── Command ring (29.1) ──────────────────────────────────────────────
    pub command_ring_phys: usize, // physical address of command ring buffer
    pub command_ring_size: usize, // size of command ring buffer (bytes)
    pub fence_value: u64,         // current fence value (incremented per submission)

    // ── Interrupt (29.1 MSI-X) ───────────────────────────────────────────
    pub msix_vectors: [bool; MSIX_MAX_VECTORS], // allocated MSI-X vectors
    pub interrupt_pending: bool,  // true if interrupt is being processed
    pub gart_next_va: usize,      // next free GPU VA for GART allocation

    // ── MPS tracking (29.2) ──────────────────────────────────────────────
    pub active_workloads: [i16; MAX_CONCURRENT_PER_GPU], // workload IDs (-1 if none)

    // ── Utilization ──────────────────────────────────────────────────────
    pub total_ops_submitted: u64,   // lifetime ops submitted
    pub total_ops_completed: u64,   // lifetime ops completed
    pub start_tick: u64,            // tick when device became active
    pub idle_ticks: u64,            // cumulative idle ticks
}

impl GpuDevice {
    const fn empty() -> Self {
        GpuDevice {
            dev_id: 0,
            mmio_base: 0,
            memory_base: 0,
            memory_size: 0,
            active: false,
            command_ring_phys: 0,
            command_ring_size: 0,
            fence_value: 0,
            msix_vectors: [false; MSIX_MAX_VECTORS],
            interrupt_pending: false,
            gart_next_va: 0,
            active_workloads: [-1i16; MAX_CONCURRENT_PER_GPU],
            total_ops_submitted: 0,
            total_ops_completed: 0,
            start_tick: 0,
            idle_ticks: 0,
        }
    }
}

// ── AI Workload ───────────────────────────────────────────────────────────

/// An AI workload with priority, state, time-slicing, and tensor operation data.
#[derive(Clone, Copy)]
pub(crate) struct AiWorkload {
    pub pid: u32,
    pub gpu_id: u32,
    pub priority: u8,          // 0=low, 1=normal, 2=high, 3=realtime
    pub state: u8,             // 0=queued, 1=running, 2=completed, 3=failed, 4=preempted
    pub batch_size: usize,     // total number of GPU operations
    pub ops_done: usize,       // completed operations (for preemption tracking)
    pub submit_tick: u64,      // tick at submission time
    pub model_id: u32,         // associated model (-1 if none)
    pub tensor_op_data: [u8; 32], // serialized tensor operation data
    pub tensor_op_len: usize,  // valid bytes in tensor_op_data
}

// ── Static Tables ─────────────────────────────────────────────────────────

pub(crate) static mut GPU_DEVICES: [GpuDevice; MAX_GPU_DEVS] = [
    GpuDevice::empty(); MAX_GPU_DEVS
];
pub(crate) static mut GPU_COUNT: usize = 0;

pub(crate) static mut AI_QUEUES: [AiWorkload; MAX_AI_QUEUES] = [
    AiWorkload {
        pid: 0,
        gpu_id: 0,
        priority: 0,
        state: 0,
        batch_size: 0,
        ops_done: 0,
        submit_tick: 0,
        model_id: 0,
        tensor_op_data: [0u8; 32],
        tensor_op_len: 0,
    }; MAX_AI_QUEUES
];
pub(crate) static mut AI_QUEUE_COUNT: usize = 0;

// ── 29.1 GPU Driver Framework ─────────────────────────────────────────────

/// Register a GPU device discovered during PCI enumeration.
/// Sets up command ring, initializes GART aperture, and prepares MSI-X slots.
pub fn gpu_register(mmio_base: usize, memory_base: usize, memory_size: usize) -> Option<u32> {
    unsafe {
        if GPU_COUNT >= MAX_GPU_DEVS {
            return None;
        }
        let id = GPU_COUNT as u32;
        GPU_DEVICES[GPU_COUNT] = GpuDevice {
            dev_id: id,
            mmio_base,
            memory_base,
            memory_size,
            active: true,
            command_ring_phys: mmio_base + 0x1000,  // ring buffer right after MMIO region
            command_ring_size: 4096,                 // 4KB ring buffer
            fence_value: 0,
            msix_vectors: [false; MSIX_MAX_VECTORS],
            interrupt_pending: false,
            gart_next_va: memory_base,               // start GPU VA at memory base
            active_workloads: [-1i16; MAX_CONCURRENT_PER_GPU],
            total_ops_submitted: 0,
            total_ops_completed: 0,
            start_tick: crate::trap::TICK_COUNT as u64,
            idle_ticks: 0,
        };
        GPU_COUNT += 1;
        Some(id)
    }
}

/// Submit a command buffer to a GPU device via the command ring.
/// Writes command data to the ring buffer and updates the fence.
pub fn gpu_submit_command(gpu_id: usize, command_buffer: &[u8], command_len: usize) -> Result<(), &'static str> {
    unsafe {
        if gpu_id >= GPU_COUNT || !GPU_DEVICES[gpu_id].active {
            return Err("invalid gpu");
        }
        let dev = &mut GPU_DEVICES[gpu_id];
        if command_len > dev.command_ring_size {
            return Err("command too large");
        }

        // Write command to ring buffer via MMIO
        let ring_ptr = dev.command_ring_phys as *mut u8;
        for i in 0..command_len {
            ring_ptr.add(i).write_volatile(command_buffer[i]);
        }

        // Increment fence value — signals GPU to start processing
        dev.fence_value += 1;
        dev.total_ops_submitted += command_len as u64;

        // Write fence to MMIO register (offset 0x20 from MMIO base)
        let fence_reg = (dev.mmio_base + 0x20) as *mut u64;
        fence_reg.write_volatile(dev.fence_value);

        Ok(())
    }
}

/// Wait for a fence value to be completed by the GPU.
/// Polls an MMIO register until the fence is signaled.
pub fn gpu_wait_fence(gpu_id: usize, fence: u64) -> Result<(), &'static str> {
    unsafe {
        if gpu_id >= GPU_COUNT || !GPU_DEVICES[gpu_id].active {
            return Err("invalid gpu");
        }
        let dev = &GPU_DEVICES[gpu_id];

        // Poll the fence status register at MMIO offset 0x28
        let fence_status = (dev.mmio_base + 0x28) as *const u64;
        let mut poll_count: u32 = 0;
        loop {
            let status = fence_status.read_volatile();
            if status >= fence {
                return Ok(());
            }
            poll_count += 1;
            if poll_count > 100_000_000 {
                return Err("fence timeout");
            }
            core::hint::spin_loop();
        }
    }
}

/// Allocate GPU memory: returns a GPU virtual address.
/// Uses the GART-like allocator to assign GPU VA space and buddy allocator for physical pages.
pub fn gpu_alloc(gpu_id: u32, size: usize) -> Option<usize> {
    unsafe {
        if gpu_id as usize >= GPU_COUNT || !GPU_DEVICES[gpu_id as usize].active {
            return None;
        }
        let aligned_size = (size + 0xFFF) & !0xFFF;
        if aligned_size > 16 * 4096 {
            return None;
        }
        let num_pages = aligned_size >> 12;
        if num_pages > 16 {
            return None;
        }

        // Allocate physical pages and find a region slot
        let (slot, phys_pages) = alloc_region_pages(num_pages)?;

        // Assign GPU VA from this device's GART window
        let dev = &mut GPU_DEVICES[gpu_id as usize];
        let gpu_va = dev.gart_next_va;
        if gpu_va + aligned_size > dev.memory_base + dev.memory_size {
            return None;
        }
        dev.gart_next_va = gpu_va + aligned_size;

        // Register the region
        register_region(slot, gpu_va, aligned_size, phys_pages, num_pages);

        Some(gpu_va)
    }
}

/// Free GPU memory by GPU virtual address.
pub fn gpu_free(gpu_id: u32, gpu_va: usize) -> bool {
    let _ = gpu_id; // device check done in gpu_free_region
    gpu_free_region(gpu_va)
}

/// Handle a GPU interrupt (MSI-X style).
/// Processes the completion, advances fence, updates utilization.
pub fn gpu_handle_interrupt(gpu_id: usize) {
    unsafe {
        if gpu_id >= GPU_COUNT || !GPU_DEVICES[gpu_id].active {
            return;
        }
        let dev = &mut GPU_DEVICES[gpu_id];
        dev.interrupt_pending = false;

        // Read completed fence from MMIO status register
        let fence_status = (dev.mmio_base + 0x28) as *const u64;
        let completed_fence = fence_status.read_volatile();
        dev.fence_value = completed_fence;
        dev.total_ops_completed = completed_fence;

        // Acknowledge interrupt at MMIO offset 0x30
        let ack_reg = (dev.mmio_base + 0x30) as *mut u32;
        ack_reg.write_volatile(1);
    }
}

/// Get GPU utilization as a fixed-point value (0-1000, where 1000 = 100.0%).
pub fn gpu_utilization(gpu_id: u32) -> u32 {
    unsafe {
        if gpu_id as usize >= GPU_COUNT || !GPU_DEVICES[gpu_id as usize].active {
            return 0;
        }
        let dev = &GPU_DEVICES[gpu_id as usize];
        let current_tick = crate::trap::TICK_COUNT as u64;
        let elapsed = if current_tick > dev.start_tick {
            current_tick - dev.start_tick
        } else {
            1
        };
        if elapsed == 0 {
            return 0;
        }
        // Utilization = (total_ops_completed / max_possible_ops) scaled to 0-1000
        // Estimate max ops per tick as ~1000
        let max_ops = elapsed * 1000;
        if max_ops == 0 {
            return 0;
        }
        let util = (dev.total_ops_completed * 1000) / max_ops;
        core::cmp::min(util as u32, 1000)
    }
}

// ── 29.2 AI Workload Scheduling ───────────────────────────────────────────

/// Submit an AI workload to the GPU queue (without tensor data).
pub fn ai_submit(pid: u32, gpu_id: u32, priority: u8, batch_size: usize) -> Option<usize> {
    unsafe {
        if AI_QUEUE_COUNT >= MAX_AI_QUEUES {
            return None;
        }
        let id = AI_QUEUE_COUNT;
        AI_QUEUES[id] = AiWorkload {
            pid,
            gpu_id,
            priority: core::cmp::min(priority, PRIO_REALTIME),
            state: WL_QUEUED,
            batch_size,
            ops_done: 0,
            submit_tick: crate::trap::TICK_COUNT as u64,
            model_id: u32::MAX,
            tensor_op_data: [0u8; 32],
            tensor_op_len: 0,
        };
        AI_QUEUE_COUNT += 1;
        Some(id)
    }
}

/// Submit an AI workload with embedded tensor operation data.
/// This is used by the inference pipeline to attach TensorOp data to workloads.
pub fn ai_submit_with_data(pid: u32, gpu_id: u32, priority: u8, batch_size: usize,
    tensor_op_bytes: &[u8]) -> Option<usize> {
    unsafe {
        if AI_QUEUE_COUNT >= MAX_AI_QUEUES {
            return None;
        }
        let copy_len = core::cmp::min(tensor_op_bytes.len(), 32);
        let mut data = [0u8; 32];
        data[..copy_len].copy_from_slice(&tensor_op_bytes[..copy_len]);

        let id = AI_QUEUE_COUNT;
        AI_QUEUES[id] = AiWorkload {
            pid,
            gpu_id,
            priority: core::cmp::min(priority, PRIO_REALTIME),
            state: WL_QUEUED,
            batch_size,
            ops_done: 0,
            submit_tick: crate::trap::TICK_COUNT as u64,
            model_id: u32::MAX,
            tensor_op_data: data,
            tensor_op_len: copy_len,
        };
        AI_QUEUE_COUNT += 1;
        Some(id)
    }
}

/// Schedule: pick the next workload to run.
/// Strategy: highest priority first, FIFO within same priority.
/// Also checks MPS limits — up to MAX_CONCURRENT_PER_GPU concurrent workloads per GPU.
pub fn ai_schedule() -> Option<usize> {
    unsafe {
        // Check MPS limits: count running workloads per GPU
        let mut gpu_running: [usize; MAX_GPU_DEVS] = [0; MAX_GPU_DEVS];
        for i in 0..AI_QUEUE_COUNT {
            if AI_QUEUES[i].state == WL_RUNNING {
                let gid = AI_QUEUES[i].gpu_id as usize;
                if gid < MAX_GPU_DEVS {
                    gpu_running[gid] += 1;
                }
            }
        }

        // Find the best candidate: highest priority, oldest queued, respecting MPS
        let mut best: Option<(usize, u8, u64)> = None; // (idx, priority, submit_tick)
        for i in 0..AI_QUEUE_COUNT {
            if AI_QUEUES[i].state != WL_QUEUED && AI_QUEUES[i].state != WL_PREEMPTED {
                continue;
            }
            let gid = AI_QUEUES[i].gpu_id as usize;
            // Check MPS limit: is this GPU at capacity?
            if gid < MAX_GPU_DEVS && gpu_running[gid] >= MAX_CONCURRENT_PER_GPU {
                continue;
            }

            let prio = AI_QUEUES[i].priority;
            let tick = AI_QUEUES[i].submit_tick;
            match best {
                None => best = Some((i, prio, tick)),
                Some((_, best_prio, best_tick)) => {
                    if prio > best_prio || (prio == best_prio && tick < best_tick) {
                        best = Some((i, prio, tick));
                    }
                }
            }
        }

        if let Some((idx, _, _)) = best {
            AI_QUEUES[idx].state = WL_RUNNING;
            // Update GPU active_workloads
            let gid = AI_QUEUES[idx].gpu_id as usize;
            if gid < MAX_GPU_DEVS {
                for slot in &mut GPU_DEVICES[gid].active_workloads {
                    if *slot == -1 {
                        *slot = idx as i16;
                        break;
                    }
                }
            }
            Some(idx)
        } else {
            None
        }
    }
}

/// Mark a workload as completed with a result.
pub fn ai_complete(workload_id: usize, result: bool) -> bool {
    unsafe {
        if workload_id >= AI_QUEUE_COUNT {
            return false;
        }
        AI_QUEUES[workload_id].state = if result { WL_COMPLETED } else { WL_FAILED };

        // Remove from GPU active_workloads tracking
        let gid = AI_QUEUES[workload_id].gpu_id as usize;
        if gid < MAX_GPU_DEVS {
            for slot in &mut GPU_DEVICES[gid].active_workloads {
                if *slot == workload_id as i16 {
                    *slot = -1;
                    break;
                }
            }
        }

        // Update GPU completion stats
        if gid < MAX_GPU_DEVS {
            GPU_DEVICES[gid].total_ops_completed += AI_QUEUES[workload_id].ops_done as u64;
        }

        true
    }
}

/// Preempt a running workload: save progress and mark as preempted.
pub fn ai_preempt(workload_id: usize) -> bool {
    unsafe {
        if workload_id >= AI_QUEUE_COUNT || AI_QUEUES[workload_id].state != WL_RUNNING {
            return false;
        }
        AI_QUEUES[workload_id].state = WL_PREEMPTED;

        // Remove from GPU active_workloads tracking
        let gid = AI_QUEUES[workload_id].gpu_id as usize;
        if gid < MAX_GPU_DEVS {
            for slot in &mut GPU_DEVICES[gid].active_workloads {
                if *slot == workload_id as i16 {
                    *slot = -1;
                    break;
                }
            }
        }

        true
    }
}

/// Check for time-slice expiry and preempt if needed.
/// Called periodically (e.g., from the scheduler tick).
pub fn ai_check_preemption() {
    unsafe {
        for i in 0..AI_QUEUE_COUNT {
            if AI_QUEUES[i].state == WL_RUNNING {
                if AI_QUEUES[i].ops_done >= TIME_QUANTUM_OPS {
                    // Check if there are other queued workloads on the same GPU
                    let gid = AI_QUEUES[i].gpu_id as usize;
                    let has_pending = (0..AI_QUEUE_COUNT).any(|j| {
                        j != i && AI_QUEUES[j].state == WL_QUEUED && AI_QUEUES[j].gpu_id as usize == gid
                    });
                    if has_pending {
                        let _ = ai_preempt(i);
                    }
                }
            }
        }
    }
}

/// Get the number of active (running) workloads on a GPU.
pub fn gpu_active_workloads(gpu_id: u32) -> usize {
    unsafe {
        if gpu_id as usize >= GPU_COUNT || !GPU_DEVICES[gpu_id as usize].active {
            return 0;
        }
        let mut count = 0;
        for i in 0..AI_QUEUE_COUNT {
            if AI_QUEUES[i].state == WL_RUNNING && AI_QUEUES[i].gpu_id == gpu_id {
                count += 1;
            }
        }
        count
    }
}

// ── Tensor / Inference Integration (29.3) ─────────────────────────────────

/// Load a model into GPU memory and register it.
pub fn model_load(gpu_id: u32, model_data: *const u8, model_len: usize) -> Option<u32> {
    tensor::model_load(gpu_id, model_data, model_len)
}

/// Unload a model and free its GPU memory.
pub fn model_unload(model_id: u32) -> bool {
    tensor::model_unload(model_id)
}

/// List models into a buffer.
pub fn model_list(buf: &mut [u8]) -> usize {
    tensor::model_list(buf)
}

/// Submit an inference job: wraps tensor ops into an AI workload.
pub fn inference_submit(model_id: u32, input_tensor: u64, output_tensor: u64) -> Option<usize> {
    tensor::inference_submit(model_id, input_tensor, output_tensor)
}

/// Get inference statistics for a model.
pub fn inference_stats(model_id: u32) -> Option<(u64, u64, u64)> {
    tensor::inference_stats(model_id)
}

/// Execute a single tensor operation (simulated).
pub fn tensor_op_execute(op: &TensorOp) {
    tensor::tensor_op_execute(op);
}

/// Advance the ops_done counter for a running workload (called during execution).
pub fn ai_advance_ops(workload_id: usize, ops: usize) {
    unsafe {
        if workload_id < AI_QUEUE_COUNT && AI_QUEUES[workload_id].state == WL_RUNNING {
            AI_QUEUES[workload_id].ops_done += ops;
        }
    }
}

// ── V34.3 GPU-CPU Heterogeneous Scheduling ─────────────────────────────────

/// GPU-CPU heterogeneous scheduler.
/// Extends V25 NUMA-aware scheduling for GPU devices.
pub(crate) struct HeteroScheduler {
    gpu_numa_map: [(u32, u8); 4],
    gpu_usage: [u32; 4],
    cpu_usage: [u32; 8],
}

impl HeteroScheduler {
    pub const fn new() -> Self {
        HeteroScheduler {
            gpu_numa_map: [(u32::MAX, 0); 4],
            gpu_usage: [0; 4],
            cpu_usage: [0; 8],
        }
    }

    pub fn set_gpu_numa(&mut self, gpu_id: u32, numa_node: u8) {
        for entry in self.gpu_numa_map.iter_mut() {
            if entry.0 == u32::MAX || entry.0 == gpu_id {
                *entry = (gpu_id, numa_node);
                return;
            }
        }
    }

    pub fn schedule(&self, workload: &PdWorkload) -> Option<(u32, u8)> {
        let mut best: Option<(u32, u8, u32)> = None;
        for entry in self.gpu_numa_map.iter() {
            let (gpu_id, numa_node) = *entry;
            if gpu_id == u32::MAX { continue; }
            let gpu_idx = gpu_id as usize;
            if gpu_idx >= 4 { continue; }
            let usage = self.gpu_usage[gpu_idx];
            match best {
                None => best = Some((gpu_id, numa_node, usage)),
                Some((_, _, best_usage)) => {
                    if usage < best_usage {
                        best = Some((gpu_id, numa_node, usage));
                    }
                }
            }
        }
        best.map(|(gpu, node, _)| (gpu, node))
    }

    pub fn migrate_workload(&mut self, workload_id: usize, to_gpu: u32) -> Result<(), &'static str> {
        if (to_gpu as usize) >= 4 { return Err("invalid gpu"); }
        if let Some(wl) = unsafe { pd_sched::PD_SCHEDULER.get_workload_mut(workload_id) } {
            crate::println!("HETERO: migrating workload {} from GPU {} to GPU {}",
                workload_id, wl.gpu_id, to_gpu);
            wl.gpu_id = to_gpu;
            Ok(())
        } else {
            Err("workload not found")
        }
    }

    pub fn balance_gpu_load(&mut self) {
        let mut hottest = 0usize;
        let mut coldest = 0usize;
        let mut max_usage = u32::MIN;
        let mut min_usage = u32::MAX;
        for (i, &usage) in self.gpu_usage.iter().enumerate() {
            if usage > max_usage { max_usage = usage; hottest = i; }
            if usage < min_usage { min_usage = usage; coldest = i; }
        }
        if max_usage <= min_usage + 300 { return; }
        unsafe {
            for i in 0..pd_sched::MAX_PD_WORKLOADS {
                if let Some(ref wl) = pd_sched::PD_SCHEDULER.workloads[i] {
                    if wl.gpu_id as usize == hottest
                        && matches!(wl.role, PdRole::Decode)
                        && matches!(wl.state, PdWorkloadState::Decoding)
                    {
                        let wid = wl.workload_id;
                        let _ = self.migrate_workload(wid, coldest as u32);
                        crate::println!("HETERO: balanced workload {}: GPU {} -> GPU {}",
                            wid, hottest, coldest);
                        break;
                    }
                }
            }
        }
        unsafe {
            crate::ai::AI_SCHED_STATS.gpu_balance_operations =
                crate::ai::AI_SCHED_STATS.gpu_balance_operations.wrapping_add(1);
        }
    }

    pub fn optimal_memory_placement(&self, gpu_id: u32) -> u8 {
        for entry in self.gpu_numa_map.iter() {
            if entry.0 == gpu_id { return entry.1; }
        }
        0
    }

    pub fn update_gpu_usage(&mut self, gpu_id: u32, usage: u32) {
        let idx = gpu_id as usize;
        if idx < 4 { self.gpu_usage[idx] = core::cmp::min(usage, 1000); }
    }

    pub fn update_cpu_usage(&mut self, node: u8, usage: u32) {
        let idx = node as usize;
        if idx < 8 { self.cpu_usage[idx] = core::cmp::min(usage, 1000); }
    }
}

pub(crate) static mut HETERO_SCHEDULER: HeteroScheduler = HeteroScheduler::new();

// ── V34.4 AI Scheduling Statistics ──────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct AiSchedStats {
    pub prefill_workloads: u64,
    pub decode_steps: u64,
    pub kv_cache_hits: u64,
    pub kv_cache_misses: u64,
    pub kv_cache_evictions: u64,
    pub page_migrations: u64,
    pub gpu_balance_operations: u64,
    pub avg_prefill_latency_us: u64,
    pub avg_decode_latency_us: u64,
    pub p99_decode_latency_us: u64,
}

impl AiSchedStats {
    pub const fn new() -> Self {
        AiSchedStats {
            prefill_workloads: 0,
            decode_steps: 0,
            kv_cache_hits: 0,
            kv_cache_misses: 0,
            kv_cache_evictions: 0,
            page_migrations: 0,
            gpu_balance_operations: 0,
            avg_prefill_latency_us: 0,
            avg_decode_latency_us: 0,
            p99_decode_latency_us: 0,
        }
    }
}

pub(crate) static mut AI_SCHED_STATS: AiSchedStats = AiSchedStats::new();

pub fn ai_sched_stats() -> AiSchedStats {
    unsafe {
        let mut stats = AI_SCHED_STATS;
        stats.prefill_workloads = pd_sched::PD_SCHEDULER.prefill_count;
        stats.decode_steps = pd_sched::PD_SCHEDULER.decode_count;
        stats
    }
}

pub fn ai_sched_reset_stats() {
    unsafe { AI_SCHED_STATS = AiSchedStats::new(); }
}

// ── Utility Functions ─────────────────────────────────────────────────────

/// List GPU devices into a buffer.
/// Format per device: [dev_id:4][memory_size:4][active:1][utilization:4] = 13 bytes each
pub fn gpu_list(buf: &mut [u8]) -> usize {
    unsafe {
        let mut pos = 0;
        for i in 0..GPU_COUNT {
            if pos + 13 > buf.len() {
                break;
            }
            let dev = &GPU_DEVICES[i];
            buf[pos..pos+4].copy_from_slice(&dev.dev_id.to_le_bytes());
            buf[pos+4..pos+8].copy_from_slice(&(dev.memory_size as u32).to_le_bytes());
            buf[pos+8] = dev.active as u8;
            let util = gpu_utilization(dev.dev_id);
            buf[pos+9..pos+13].copy_from_slice(&util.to_le_bytes());
            pos += 13;
        }
        pos
    }
}

/// Get detailed GPU device info into a buffer.
/// Format: [dev_id:4][fence_value:8][active_wl:4][mem_used:4][util:4] = 24 bytes.
pub fn gpu_info(gpu_id: u32, buf: &mut [u8]) -> Option<usize> {
    unsafe {
        if (gpu_id as usize) >= GPU_COUNT || buf.len() < 24 {
            return None;
        }
        let dev = &GPU_DEVICES[gpu_id as usize];
        buf[0..4].copy_from_slice(&dev.dev_id.to_le_bytes());
        buf[4..12].copy_from_slice(&dev.fence_value.to_le_bytes());
        let active_wl = gpu_active_workloads(gpu_id) as u32;
        buf[12..16].copy_from_slice(&active_wl.to_le_bytes());
        let mem_used = gpu_mem::gpu_mem_used() as u32;
        buf[16..20].copy_from_slice(&mem_used.to_le_bytes());
        let util = gpu_utilization(gpu_id);
        buf[20..24].copy_from_slice(&util.to_le_bytes());
        Some(24)
    }
}

/// Get next queued workload ID (for scheduler polling).
pub fn ai_next_workload() -> Option<usize> {
    ai_schedule()
}

/// Get workload status information into a buffer.
/// Format: [wl_id:4][pid:4][state:1][priority:1][gpu_id:1][ops_done:4][batch_size:4] = 19 bytes
pub fn ai_workload_info(workload_id: usize, buf: &mut [u8]) -> Option<usize> {
    unsafe {
        if workload_id >= AI_QUEUE_COUNT || buf.len() < 19 {
            return None;
        }
        let wl = &AI_QUEUES[workload_id];
        buf[0..4].copy_from_slice(&(workload_id as u32).to_le_bytes());
        buf[4..8].copy_from_slice(&wl.pid.to_le_bytes());
        buf[8] = wl.state;
        buf[9] = wl.priority;
        buf[10] = wl.gpu_id as u8;
        buf[11..15].copy_from_slice(&(wl.ops_done as u32).to_le_bytes());
        buf[15..19].copy_from_slice(&(wl.batch_size as u32).to_le_bytes());
        Some(19)
    }
}
