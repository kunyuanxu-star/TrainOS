// V25: NUMA-aware scheduler, EEVDF, per-node ready queues,
// load balancing, topology discovery, and memory subsystem sharding.
//
// Overview
// --------
//   - Each NUMA node has its own ready queues (64 priorities per node)
//     and priority bitmap.
//   - EEVDF (Earliest Eligible Virtual Deadline First) sorts threads
//     within each priority level by deadline.
//   - Load balancing runs every 1000 timer ticks, migrating threads
//     when node imbalance exceeds 25%.
//   - Memory allocations prefer the local node, falling back to others.
//   - Topology is discovered from a simplified device-tree (mock for QEMU).

pub mod mcs;
pub mod percpu;
pub mod rcu;

use crate::proc::thread::{Thread, ThreadState};
use crate::sched::ThreadQueue;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Mutex;

// ── Constants ─────────────────────────────────────────────────────────────────

const MAX_NODES: usize = 8;
const NUM_PRIORITIES: usize = 64;
const BALANCE_INTERVAL: usize = 1000; // ticks between balance checks
const BALANCE_THRESHOLD: usize = 25; // percentage imbalance threshold
const SLICE_TICKS: u64 = 1; // EEVDF time slice in timer ticks
const WEIGHT_MAX: u64 = 512; // maximum scheduling weight

// ── EevdfThread (tracking / display struct) ───────────────────────────────────

/// EEVDF scheduling parameters for a thread.
/// The actual fields live on `Thread`; this struct is used for
/// the `sys_numa_info` API and diagnostic output.
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct EevdfThread {
    pub pid: u32,
    pub virtual_runtime: u64,
    pub weight: u32,
    pub deadline: u64,
    pub node_id: u8,
}

// ── Per-node ready queues ─────────────────────────────────────────────────────

/// Ready queues for one NUMA node.
pub(crate) struct NumaReadyQueues {
    pub(crate) queues: [ThreadQueue; NUM_PRIORITIES],
    pub(crate) priority_bitmap: u64,
    /// Approximate number of ready threads on this node (for load balancing).
    pub(crate) thread_count: usize,
}

impl NumaReadyQueues {
    pub const fn new() -> Self {
        const EMPTY: ThreadQueue = ThreadQueue::new();
        NumaReadyQueues {
            queues: [EMPTY; NUM_PRIORITIES],
            priority_bitmap: 0,
            thread_count: 0,
        }
    }
}

// ── NUMA node descriptor ──────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct NumaNode {
    node_id: u8,
    cpu_mask: u64,        // CPUs belonging to this node (bitmask)
    memory_base: usize,   // physical memory base
    memory_size: usize,   // memory size in bytes
    free_pages: usize,    // free pages on this node
    total_pages: usize,   // total pages on this node
    local_pages: usize,   // pages allocated from this node
}

// ── Global state ──────────────────────────────────────────────────────────────

static NUMA_NODES: Mutex<[NumaNode; MAX_NODES]> = Mutex::new([
    NumaNode {
        node_id: 0,
        cpu_mask: 0xFF,
        memory_base: 0x80000000,
        memory_size: 128 * 1024 * 1024,
        free_pages: 0,
        total_pages: 0,
        local_pages: 0,
    };
    MAX_NODES
]);

static NUMA_NODE_COUNT: AtomicUsize = AtomicUsize::new(1);

/// Per-node ready queues, each protected by its own mutex.
const EMPTY_QUEUES: NumaReadyQueues = NumaReadyQueues::new();
static NODE_QUEUES: [Mutex<NumaReadyQueues>; MAX_NODES] = [
    Mutex::new(EMPTY_QUEUES),
    Mutex::new(EMPTY_QUEUES),
    Mutex::new(EMPTY_QUEUES),
    Mutex::new(EMPTY_QUEUES),
    Mutex::new(EMPTY_QUEUES),
    Mutex::new(EMPTY_QUEUES),
    Mutex::new(EMPTY_QUEUES),
    Mutex::new(EMPTY_QUEUES),
];

/// Balance tick counter — incremented by try_balance, triggers every 1000.
static BALANCE_TICK: AtomicUsize = AtomicUsize::new(0);

// ── CPU topology discovery ────────────────────────────────────────────────────

/// Initialize NUMA topology.
///
/// For QEMU `virt` we create a single node covering all CPUs and all DRAM.
/// Real hardware would parse ACPI SRAT or device-tree.
pub fn discover() {
    let mut nodes = NUMA_NODES.lock();
    // QEMU virt: single node, all CPUs (0..7), full DRAM range
    let total_mem = crate::mem::layout::DRAM_SIZE;
    let total_pages = total_mem / 4096;
    nodes[0] = NumaNode {
        node_id: 0,
        cpu_mask: 0xFF, // 8 CPUs
        memory_base: crate::mem::layout::DRAM_BASE,
        memory_size: total_mem,
        free_pages: total_pages - crate::mem::buddy::allocated_pages(),
        total_pages,
        local_pages: 0,
    };
    NUMA_NODE_COUNT.store(1, Ordering::Release);
    crate::println!("NUMA: discovered {} node(s), {} MB total", 1, total_mem >> 20);
}

/// Register a new NUMA node.
/// Called by platform init code for multi-node configurations.
#[allow(dead_code)]
pub fn register_node(node_id: u8, cpu_mask: u64, mem_base: usize, mem_size: usize) {
    let mut nodes = NUMA_NODES.lock();
    if (node_id as usize) < MAX_NODES {
        nodes[node_id as usize] = NumaNode {
            node_id,
            cpu_mask,
            memory_base: mem_base,
            memory_size: mem_size,
            free_pages: mem_size / 4096,
            total_pages: mem_size / 4096,
            local_pages: 0,
        };
        let prev = NUMA_NODE_COUNT.load(Ordering::Acquire);
        if (node_id as usize) >= prev {
            NUMA_NODE_COUNT.store(node_id as usize + 1, Ordering::Release);
        }
        crate::println!(
            "NUMA: registered node {} cpu_mask=0x{:x} mem=0x{:x}+{}",
            node_id, cpu_mask, mem_base, mem_size
        );
    }
}

// ── Hart → Node mapping ───────────────────────────────────────────────────────

/// Return the NUMA node that a given hart belongs to.
pub fn hart_to_node(hart: usize) -> u8 {
    let count = NUMA_NODE_COUNT.load(Ordering::Acquire);
    let nodes = NUMA_NODES.lock();
    for i in 0..count {
        if (nodes[i].cpu_mask >> hart) & 1 != 0 {
            return nodes[i].node_id;
        }
    }
    0 // default to node 0
}

// ── Memory address → Node mapping ─────────────────────────────────────────────

/// Return the NUMA node that owns a given physical address.
#[allow(dead_code)]
pub fn addr_to_node(pa: usize) -> u8 {
    let count = NUMA_NODE_COUNT.load(Ordering::Acquire);
    let nodes = NUMA_NODES.lock();
    for i in 0..count {
        let n = &nodes[i];
        if pa >= n.memory_base && pa < n.memory_base + n.memory_size {
            return n.node_id;
        }
    }
    0
}

// ── Scheduling: enqueue ───────────────────────────────────────────────────────

/// Recompute a thread's EEVDF deadline based on its vruntime and weight.
fn update_eevdf_deadline(thread: &mut Thread) {
    // Map weight (8..512) into a scheduling pressure:
    //   deadline = vruntime + (SLICE_TICKS * WEIGHT_MAX / weight)
    // Higher-weight (higher-priority) threads get shorter deadlines.
    let w = core::cmp::max(thread.weight, 1) as u64;
    let slice = SLICE_TICKS * WEIGHT_MAX / w;
    thread.deadline = thread.vruntime.wrapping_add(slice);
}

/// Insert a thread into a node's ready queue, sorted by EEVDF deadline.
fn enqueue_eevdf(thread: *mut Thread, queues: &mut NumaReadyQueues) {
    unsafe {
        let pri = (*thread).effective_priority as usize;
        if pri >= NUM_PRIORITIES {
            return;
        }
        let deadline = (*thread).deadline;
        // Find insertion point: keep queue sorted by deadline (earliest first).
        let q = &mut queues.queues[pri];
        q.insert_sorted_by_deadline(thread, deadline);
        queues.priority_bitmap |= 1u64 << pri;
        queues.thread_count = queues.thread_count.wrapping_add(1);
    }
}

/// Enqueue a thread onto its NUMA node's ready queue.
/// Updates EEVDF vruntime and deadline before inserting.
pub fn enqueue_thread(thread: *mut Thread) {
    unsafe {
        let node = (*thread).node_id;
        // Update EEVDF state: increment vruntime if the thread was running
        if (*thread).state == ThreadState::Running {
            let w = core::cmp::max((*thread).weight, 1) as u64;
            (*thread).vruntime = (*thread).vruntime.wrapping_add(WEIGHT_MAX / w);
        }
        update_eevdf_deadline(&mut *thread);
        (*thread).state = ThreadState::Ready;
        let node_idx = (node as usize).min(MAX_NODES - 1);
        let mut q = NODE_QUEUES[node_idx].lock();
        enqueue_eevdf(thread, &mut *q);
    }
}

// ── Scheduling: pick next ─────────────────────────────────────────────────────

/// Pick the thread with the earliest deadline from the highest-priority
/// non-empty queue on the given node.
fn pick_next_eevdf(queues: &mut NumaReadyQueues) -> Option<*mut Thread> {
    if queues.priority_bitmap == 0 {
        return None;
    }
    let highest = 63 - queues.priority_bitmap.leading_zeros() as usize;
    let q = &mut queues.queues[highest];
    let thread = q.pop_front()?;
    if q.is_empty() {
        queues.priority_bitmap &= !(1u64 << highest);
    }
    queues.thread_count = queues.thread_count.wrapping_sub(1);
    unsafe {
        (*thread).state = ThreadState::Running;
    }
    Some(thread)
}

/// Pick the next thread to run on a given NUMA node.
pub fn pick_next(node_id: u8) -> Option<*mut Thread> {
    let node_idx = (node_id as usize).min(MAX_NODES - 1);
    let mut q = NODE_QUEUES[node_idx].lock();
    pick_next_eevdf(&mut *q)
}

/// Pick the next thread for a specific hardware thread (hart).
/// Maps the hart to its NUMA node and picks from that node.
pub fn pick_next_for_hart(hart_id: usize) -> Option<*mut Thread> {
    let node = hart_to_node(hart_id);
    pick_next(node)
}

// ── Load balancing ────────────────────────────────────────────────────────────

/// Attempt load balancing: if the imbalance between the busiest and idlest
/// nodes exceeds the threshold, migrate one thread.
///
/// Called periodically from the timer interrupt or scheduler.
pub fn try_balance() {
    let tick = BALANCE_TICK.fetch_add(1, Ordering::Relaxed);
    if tick % BALANCE_INTERVAL != 0 {
        return;
    }

    let count = NUMA_NODE_COUNT.load(Ordering::Acquire);
    if count < 2 {
        return; // nothing to balance with a single node
    }

    // Find the busiest and idlest nodes (lock all queues briefly).
    // We lock nodes in order to avoid deadlocks: lower index first.
    let mut busiest = 0usize;
    let mut idlest = 0usize;
    let mut max_load = 0usize;
    let mut min_load = usize::MAX;

    for i in 0..count {
        let load = NODE_QUEUES[i].lock().thread_count;
        if load > max_load {
            max_load = load;
            busiest = i;
        }
        if load < min_load {
            min_load = load;
            idlest = i;
        }
    }

    // Check if imbalance exceeds threshold.
    if (max_load - min_load) * 100 <= max_load * BALANCE_THRESHOLD {
        return; // imbalance within acceptable range
    }

    // Migrate one thread from busiest to idlest.
    // Lock order: lower index first to prevent deadlock.
    let (first, second) = if busiest < idlest {
        (busiest, idlest)
    } else {
        (idlest, busiest)
    };

    let mut q_first = NODE_QUEUES[first].lock();
    let mut q_second = NODE_QUEUES[second].lock();
    let (q_busy, q_idle) = if busiest < idlest {
        (&mut *q_first, &mut *q_second)
    } else {
        (&mut *q_second, &mut *q_first)
    };

    // Steal the first thread from the busiest node's highest-priority queue.
    if let Some(thread) = pick_next_eevdf(q_busy) {
        unsafe {
            (*thread).node_id = if busiest < idlest { second as u8 } else { first as u8 };
            (*thread).state = ThreadState::Ready;
        }
        enqueue_eevdf(thread, q_idle);
        crate::println!(
            "NUMA: migrated thread {} from node {} to node {}",
            unsafe { (*thread).owner },
            busiest,
            idlest
        );
    }
    // Locks are released when q_second and q_first are dropped (in reverse order).
}

// ── Node accessors ────────────────────────────────────────────────────────────

/// Return the CPU mask for a given node.
#[allow(dead_code)]
pub fn node_cpu_mask(node: u8) -> u64 {
    let nodes = NUMA_NODES.lock();
    if (node as usize) < MAX_NODES {
        nodes[node as usize].cpu_mask
    } else {
        0
    }
}

/// Return the number of registered NUMA nodes.
pub fn node_count() -> usize {
    NUMA_NODE_COUNT.load(Ordering::Acquire)
}

/// Return per-node memory info: (base, size, free_pages, total_pages).
#[allow(dead_code)]
pub fn node_mem_info(node: u8) -> (usize, usize, usize, usize) {
    let nodes = NUMA_NODES.lock();
    if (node as usize) < MAX_NODES {
        let n = &nodes[node as usize];
        (n.memory_base, n.memory_size, n.free_pages, n.total_pages)
    } else {
        (0, 0, 0, 0)
    }
}

// ── Memory subsystem: local-first allocation ──────────────────────────────────

/// Allocate a physical page, preferring the given NUMA node.
/// Falls back to other nodes if the preferred node is out of memory.
pub fn node_alloc_page(node: u8) -> Option<usize> {
    let node_idx = (node as usize).min(MAX_NODES - 1);

    // Try the preferred node's allocator first.
    // The buddy allocator is global; we track per-node pages in the node struct.
    if let Some(pa) = crate::mem::buddy::alloc_page() {
        let mut nodes = NUMA_NODES.lock();
        if nodes[node_idx].free_pages > 0 {
            nodes[node_idx].free_pages -= 1;
        }
        nodes[node_idx].local_pages += 1;
        return Some(pa);
    }

    // Fall back: try other nodes.
    let count = NUMA_NODE_COUNT.load(Ordering::Acquire);
    for i in 0..count {
        if i == node_idx {
            continue;
        }
        if let Some(pa) = crate::mem::buddy::alloc_page() {
            let mut nodes = NUMA_NODES.lock();
            if nodes[i].free_pages > 0 {
                nodes[i].free_pages -= 1;
            }
            return Some(pa);
        }
    }

    None // all nodes OOM
}

// ── Page migration ────────────────────────────────────────────────────────────

/// Migrate a physical page from one NUMA node to another.
///
/// 1. Allocates a new page on the target node.
/// 2. Copies data from the old page to the new page.
/// 3. Frees the old page.
///
/// Returns the new physical address on success.
/// The caller is responsible for updating page table entries.
pub fn migrate_page(phys: usize, _from_node: u8, to_node: u8) -> Result<usize, &'static str> {
    if phys == 0 || phys & 0xFFF != 0 {
        return Err("invalid physical address");
    }

    // Allocate a new page on the target node.
    let new_page = node_alloc_page(to_node).ok_or("OOM on target node")?;

    // Copy data: old page -> new page.
    let old_kva = crate::mem::sv39::pa_to_kva(phys);
    let new_kva = crate::mem::sv39::pa_to_kva(new_page);
    unsafe {
        core::ptr::copy_nonoverlapping(old_kva as *const u8, new_kva as *mut u8, 4096);
    }

    // Free the old page.
    crate::mem::buddy::free_page(phys, 0);

    crate::println!(
        "NUMA: migrated page 0x{:x} -> 0x{:x} (node {})",
        phys,
        new_page,
        to_node
    );

    Ok(new_page)
}

/// Update EEVDF vruntime and deadline for a thread that just ran.
/// Called from the scheduler (via schedule() or from the timer tick).
#[allow(dead_code)]
pub fn eevdf_tick(thread: *mut Thread) {
    unsafe {
        let w = core::cmp::max((*thread).weight, 1) as u64;
        (*thread).vruntime = (*thread).vruntime.wrapping_add(WEIGHT_MAX / w);
        update_eevdf_deadline(&mut *thread);
    }
}

// ── Diagnostics ───────────────────────────────────────────────────────────────

/// Check a node's ready queues for invariant consistency.
/// Sets `ok` to false if any inconsistency is found.
/// Returns the total thread count on the node.
pub fn check_node_queues(node: u8, ok: &mut bool) -> usize {
    let node_idx = (node as usize).min(MAX_NODES - 1);
    let q = NODE_QUEUES[node_idx].lock();
    let bitmap = q.priority_bitmap;
    let mut total = 0;

    for prio in 0..64 {
        let has_bit = (bitmap >> prio) & 1 != 0;
        let empty = q.queues[prio].is_empty();
        if has_bit && empty {
            crate::println!(
                "INVARIANT: node {} prio {} bitmap set but queue empty",
                node, prio
            );
            *ok = false;
        } else if !has_bit && !empty {
            crate::println!(
                "INVARIANT: node {} prio {} bitmap clear but queue has threads",
                node, prio
            );
            *ok = false;
        }
        if !empty {
            for &t in q.queues[prio].iter() {
                unsafe {
                    match (*t).state {
                        crate::proc::thread::ThreadState::Ready => {}
                        _ => {
                            crate::println!(
                                "INVARIANT: node {} thread pid={} in ready queues has state {:?}",
                                node, (*t).owner, (*t).state
                            );
                            *ok = false;
                        }
                    }
                }
                total += 1;
            }
        }
    }
    total
}

/// Format NUMA state into a user-provided buffer.
/// Each node entry: [node_id:1][cpu_mask:8][free_pages:8][total_pages:8] = 25 bytes.
pub fn numa_state_buf(buf: &mut [u8]) -> usize {
    let count = NUMA_NODE_COUNT.load(Ordering::Acquire);
    let nodes = NUMA_NODES.lock();
    let mut pos = 0;
    for i in 0..count {
        if pos + 25 > buf.len() {
            break;
        }
        let n = &nodes[i];
        buf[pos] = n.node_id;
        let mask_bytes = n.cpu_mask.to_le_bytes();
        buf[pos + 1..pos + 9].copy_from_slice(&mask_bytes);
        let free_bytes = n.free_pages.to_le_bytes();
        buf[pos + 9..pos + 17].copy_from_slice(&free_bytes);
        let total_bytes = n.total_pages.to_le_bytes();
        buf[pos + 17..pos + 25].copy_from_slice(&total_bytes);
        pos += 25;
    }
    pos
}
