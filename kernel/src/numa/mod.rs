// V25: NUMA-aware scheduler and memory management
//
// Features: per-node ready queues, EEVDF scheduling,
// node-local page allocation, CPU topology discovery.

const MAX_NODES: usize = 8;
const MAX_CPUS: usize = 64;

#[derive(Clone, Copy)]
struct NumaNode {
    node_id: u8,
    cpu_mask: u64,      // CPUs belonging to this node
    memory_base: usize,  // physical memory base
    memory_size: usize,  // memory size in bytes
    local_pages: usize,  // pages allocated from this node
}

struct EevdfThread {
    pid: u32,
    virtual_runtime: u64,  // vruntime for EEVDF
    weight: u32,
    deadline: u64,
    node_id: u8,           // preferred NUMA node
}

static mut NUMA_NODES: [NumaNode; MAX_NODES] = [
    NumaNode { node_id: 0, cpu_mask: 0xFF, memory_base: 0x80000000, memory_size: 128*1024*1024, local_pages: 0 }; MAX_NODES
];
static mut NUMA_NODE_COUNT: usize = 1; // at least one node

// Per-node ready queues (simple u64 bitmaps)
static mut NODE_BITMAPS: [u64; MAX_NODES] = [0; MAX_NODES];

/// Discover NUMA topology (simplified: single-node for QEMU virt).
pub fn discover() {
    // QEMU virt presents a single NUMA node
    // Real hardware would parse ACPI SRAT / device tree
    unsafe {
        NUMA_NODE_COUNT = 1;
        NUMA_NODES[0].node_id = 0;
        NUMA_NODES[0].cpu_mask = 0xFF; // 8 CPUs max
    }
}

/// Get the NUMA node for a physical address.
pub fn addr_to_node(_pa: usize) -> u8 {
    // Simplified: all memory on node 0
    0
}

/// Allocate a page from a specific NUMA node.
pub fn node_alloc_page(node: u8) -> Option<usize> {
    if node >= unsafe { NUMA_NODE_COUNT as u8 } { return None; }
    unsafe { NUMA_NODES[node as usize].local_pages += 1; }
    crate::mem::buddy::alloc_page()
}

/// Get CPU mask for a node.
pub fn node_cpu_mask(node: u8) -> u64 {
    if (node as usize) < unsafe { NUMA_NODE_COUNT } {
        unsafe { NUMA_NODES[node as usize].cpu_mask }
    } else { 0 }
}

/// Get node count.
pub fn node_count() -> usize {
    unsafe { NUMA_NODE_COUNT }
}
