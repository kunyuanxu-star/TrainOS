// V26: Remote memory pool manager
//
// Manages remote memory pages allocated across cluster nodes.
// Each remote page is tracked by a handle and associated metadata.
//
// A "handle" encodes the owning physical address in the bottom bits
// and the owning node in the upper byte: (node_id << 56) | (phys >> 12)
// This allows us to reference a remote page without storing a full map.

use crate::mem::buddy;
use crate::mem::sv39;
use spin::Mutex;

// ── Constants ───────────────────────────────────────────────────────────────────

/// Maximum remote pages tracked per node.
const MAX_REMOTE_PAGES: usize = 256;

/// How many remote nodes we can pool memory from.
const MAX_POOL_NODES: usize = 8;

// ── Remote page handle encoding ─────────────────────────────────────────────────

/// Encode a remote page handle from node_id and physical address.
/// Format: [node_id:8][phys_page_index:56]
pub fn encode_handle(node_id: u8, phys: usize) -> u64 {
    let page_idx = (phys >> 12) as u64;
    (node_id as u64) << 56 | (page_idx & 0x00FF_FFFF_FFFF_FFFF)
}

/// Decode a remote page handle into (node_id, physical_address).
pub fn decode_handle(handle: u64) -> (u8, usize) {
    let node_id = (handle >> 56) as u8;
    let phys = ((handle & 0x00FF_FFFF_FFFF_FFFF) as usize) << 12;
    (node_id, phys)
}

// ── Remote page metadata ────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
struct RemotePageMeta {
    handle: u64,
    node_id: u8,
    size: u32,       // number of 4K pages in this allocation
    in_use: bool,
}

// ── Per-node memory pool ────────────────────────────────────────────────────────

/// Tracks pages allocated from a specific remote node.
pub(crate) struct NodeMemoryPool {
    pub node_id: u8,
    pub available_pages: usize,
    pages: [RemotePageMeta; MAX_REMOTE_PAGES],
    page_count: usize,
}

impl NodeMemoryPool {
    pub const fn new(node_id: u8) -> Self {
        NodeMemoryPool {
            node_id,
            available_pages: 4096, // assume 4096 pages (16MB) initially
            pages: [RemotePageMeta {
                handle: 0,
                node_id: 0,
                size: 0,
                in_use: false,
            }; MAX_REMOTE_PAGES],
            page_count: 0,
        }
    }

    /// Track a newly allocated remote page.
    pub fn track_page(&mut self, handle: u64, size: u32) -> bool {
        if self.page_count >= MAX_REMOTE_PAGES {
            return false;
        }
        self.pages[self.page_count] = RemotePageMeta {
            handle,
            node_id: self.node_id,
            size,
            in_use: true,
        };
        self.page_count += 1;
        true
    }

    /// Release a tracked page by handle.
    pub fn release_page(&mut self, handle: u64) -> bool {
        for i in 0..self.page_count {
            if self.pages[i].handle == handle && self.pages[i].in_use {
                self.pages[i].in_use = false;
                self.available_pages += self.pages[i].size as usize;
                return true;
            }
        }
        false
    }

    /// Count pages currently in use.
    pub fn used_pages(&self) -> usize {
        let mut count = 0;
        for i in 0..self.page_count {
            if self.pages[i].in_use {
                count += self.pages[i].size as usize;
            }
        }
        count
    }
}

// ── Global remote memory pool manager ───────────────────────────────────────────

pub(crate) struct RemoteMemPool {
    pub per_node: [NodeMemoryPool; MAX_POOL_NODES],
    pub node_count: usize,
}

impl RemoteMemPool {
    pub const fn new() -> Self {
        RemoteMemPool {
            per_node: [
                NodeMemoryPool::new(0),
                NodeMemoryPool::new(1),
                NodeMemoryPool::new(2),
                NodeMemoryPool::new(3),
                NodeMemoryPool::new(4),
                NodeMemoryPool::new(5),
                NodeMemoryPool::new(6),
                NodeMemoryPool::new(7),
            ],
            node_count: 0,
        }
    }

    /// Register a remote node for memory pooling.
    pub fn register_node(&mut self, node_id: u8) -> bool {
        if self.node_count >= MAX_POOL_NODES {
            return false;
        }
        if (node_id as usize) >= MAX_POOL_NODES {
            return false;
        }
        // Ensure not already registered
        for i in 0..self.node_count {
            if self.per_node[i].node_id == node_id {
                return true; // already registered
            }
        }
        // Move the node into the active slot
        if node_id as usize != self.node_count {
            self.per_node[self.node_count] = NodeMemoryPool::new(node_id);
        }
        self.node_count += 1;
        true
    }

    /// Get the pool for a specific node.
    pub fn get_pool(&mut self, node_id: u8) -> Option<&mut NodeMemoryPool> {
        for i in 0..self.node_count {
            if self.per_node[i].node_id == node_id {
                return Some(&mut self.per_node[i]);
            }
        }
        None
    }

    /// Track a page allocated from a remote node.
    pub fn track_remote_page(&mut self, node_id: u8, handle: u64, size: u32) -> bool {
        if let Some(pool) = self.get_pool(node_id) {
            if pool.available_pages >= size as usize {
                pool.available_pages -= size as usize;
                pool.track_page(handle, size)
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Release a remote page.
    pub fn release_remote_page(&mut self, handle: u64) -> bool {
        let (node_id, _) = decode_handle(handle);
        if let Some(pool) = self.get_pool(node_id) {
            pool.release_page(handle)
        } else {
            false
        }
    }

    /// Get available pages for a node.
    pub fn available_for_node(&self, node_id: u8) -> usize {
        for i in 0..self.node_count {
            if self.per_node[i].node_id == node_id {
                return self.per_node[i].available_pages;
            }
        }
        0
    }

    /// Set available pages for a node (updated via heartbeat or explicit RPC).
    pub fn set_available_pages(&mut self, node_id: u8, count: usize) {
        if let Some(pool) = self.get_pool(node_id) {
            pool.available_pages = count;
        }
    }
}

// ── Global static ───────────────────────────────────────────────────────────────

static REMOTE_MEM_POOL: Mutex<RemoteMemPool> = Mutex::new(RemoteMemPool::new());

/// Initialize the remote memory pool manager.
pub fn init() {
    // Register node 0 (local node) by default
    let mut pool = REMOTE_MEM_POOL.lock();
    pool.register_node(0);
    pool.set_available_pages(0, crate::mem::buddy::total_pages() - crate::mem::buddy::allocated_pages());
}

/// Register a remote node for memory pooling.
pub fn register_mem_node(node_id: u8) -> bool {
    REMOTE_MEM_POOL.lock().register_node(node_id)
}

/// Compute the smallest order (power-of-2) that can hold `n` pages.
fn pages_to_order(n: usize) -> usize {
    let mut order = 0;
    let mut size = 1;
    while size < n {
        size <<= 1;
        order += 1;
    }
    order
}

/// Allocate pages from a remote node.
///
/// This is a local-side operation: it allocates local physical pages and
/// tracks them as "remote" (from the perspective of the caller, they came
/// from node_id's pool). In a real distributed system, this would send
/// an RPC to the remote node.
pub fn remote_alloc_pages(node_id: u8, num_pages: usize) -> Option<usize> {
    if num_pages == 0 || num_pages > 16 {
        return None;
    }

    let order = pages_to_order(num_pages);

    // Allocate local pages to satisfy the request (in a real system,
    // this would be an RPC to the remote node's memory manager).
    let phys = buddy::alloc_pages(order)?;

    // Track in the pool
    let handle = encode_handle(node_id, phys);
    let mut pool = REMOTE_MEM_POOL.lock();
    if pool.track_remote_page(node_id, handle, (1 << order) as u32) {
        crate::println!(
            "DIST: remote_alloc_pages node={} count={} phys=0x{:x} handle=0x{:x}",
            node_id, 1 << order, phys, handle
        );
        Some(phys)
    } else {
        // Pool tracking full — free the page and fail
        buddy::free_page(phys, order);
        None
    }
}

/// Allocate a single page from a remote node.
pub fn remote_alloc_page(node_id: u8) -> Option<usize> {
    remote_alloc_pages(node_id, 1)
}

/// Free a remote page.
///
/// Returns the handle so the remote node can be notified.
pub fn remote_free_page(handle: u64) -> bool {
    let mut pool = REMOTE_MEM_POOL.lock();
    if pool.release_remote_page(handle) {
        let (_, phys) = decode_handle(handle);
        buddy::free_page(phys, 0);
        true
    } else {
        false
    }
}

/// Migrate a page from one node to another.
///
/// 1. Reads page contents from the source node (local read in simulation).
/// 2. Allocates on the target node.
/// 3. Writes data to the target.
/// 4. Updates page tracking.
///
/// Returns the new physical address on success.
pub fn migrate_page_local(phys: usize, from_node: u8, to_node: u8) -> Result<usize, &'static str> {
    if phys == 0 || phys & 0xFFF != 0 {
        return Err("invalid physical address");
    }
    if from_node == to_node {
        return Err("same node");
    }

    // Allocate a page on the target node
    let new_phys = remote_alloc_page(to_node).ok_or("OOM on target")?;

    // Copy data
    let old_kva = sv39::pa_to_kva(phys);
    let new_kva = sv39::pa_to_kva(new_phys);
    unsafe {
        core::ptr::copy_nonoverlapping(old_kva as *const u8, new_kva as *mut u8, 4096);
    }

    // Free the original page
    {
        let mut pool = REMOTE_MEM_POOL.lock();
        let old_handle = encode_handle(from_node, phys);
        pool.release_remote_page(old_handle);
    }
    buddy::free_page(phys, 0);

    crate::println!(
        "DIST: migrated page 0x{:x} from node {} to node {} -> 0x{:x}",
        phys, from_node, to_node, new_phys
    );

    Ok(new_phys)
}

/// Batch allocate from a remote node.
pub fn alloc_remote(node_id: u8, num_pages: usize) -> Option<usize> {
    remote_alloc_pages(node_id, num_pages)
}

/// Free remote pages by handle.
pub fn free_remote(_node_id: u8, handle: u64) -> bool {
    remote_free_page(handle)
}

/// Print memory pool statistics.
pub fn print_pool_stats() {
    let pool = REMOTE_MEM_POOL.lock();
    for i in 0..pool.node_count {
        let n = &pool.per_node[i];
        crate::println!(
            "  DIST pool node={}: avail={} used={}",
            n.node_id,
            n.available_pages,
            n.used_pages(),
        );
    }
}

// ── Accessors for syscalls ──────────────────────────────────────────────────────

/// Get total available pages for a remote node.
pub fn get_available_pages(node_id: u8) -> usize {
    let pool = REMOTE_MEM_POOL.lock();
    pool.available_for_node(node_id)
}

/// Set available pages count for a node.
pub fn set_available_pages(node_id: u8, count: usize) {
    let mut pool = REMOTE_MEM_POOL.lock();
    pool.set_available_pages(node_id, count);
}
