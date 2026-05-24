// V34: KV-Cache Paged Management
// Inspired by vLLM's PagedAttention approach.

use alloc::vec::Vec;

const KV_PAGE_SIZE: usize = 4096;
const MAX_KV_PAGES: usize = 1024;
const PAGES_PER_WORD: usize = 64;

fn bitmap_alloc(free_map: &mut [u64; 16]) -> Option<usize> {
    for (word_idx, word) in free_map.iter_mut().enumerate() {
        if *word != 0 {
            let bit = word.trailing_zeros() as usize;
            *word &= !(1u64 << bit);
            return Some(word_idx * PAGES_PER_WORD + bit);
        }
    }
    None
}

fn bitmap_free(free_map: &mut [u64; 16], page: usize) {
    let idx = page / PAGES_PER_WORD;
    let bit = page % PAGES_PER_WORD;
    if idx < 16 { free_map[idx] |= 1u64 << bit; }
}

fn bitmap_free_count(free_map: &[u64; 16]) -> usize {
    let mut count = 0;
    for word in free_map { count += word.count_ones() as usize; }
    count
}

#[derive(Clone, Copy)]
struct KvPageEntry {
    token_start: usize,
    token_count: usize,
    gpu_phys_addr: usize,
    dirty: bool,
    ref_count: u8,
    allocated: bool,
    last_access_tick: u64,
}

const EMPTY_PAGE_ENTRY: KvPageEntry = KvPageEntry {
    token_start: 0, token_count: 0, gpu_phys_addr: 0,
    dirty: false, ref_count: 0, allocated: false, last_access_tick: 0,
};

pub(crate) struct KvPageTable {
    entries: [KvPageEntry; MAX_KV_PAGES],
    free_pages: [u64; 16],
    total_pages: usize,
}

impl KvPageTable {
    pub const fn new(total_pages: usize) -> Self {
        let total = if total_pages > MAX_KV_PAGES { MAX_KV_PAGES } else { total_pages };
        let mut free = [0u64; 16];
        let mut i = 0;
        while i < total {
            let idx = i / PAGES_PER_WORD;
            let bit = i % PAGES_PER_WORD;
            free[idx] |= 1u64 << bit;
            i += 1;
        }
        KvPageTable {
            entries: [EMPTY_PAGE_ENTRY; MAX_KV_PAGES],
            free_pages: free,
            total_pages: total,
        }
    }

    pub fn alloc_pages(&mut self, token_count: usize) -> Option<Vec<usize>> {
        let pages_needed = core::cmp::max(1, (token_count + 63) / 64);
        let mut pages = Vec::with_capacity(pages_needed);
        for _ in 0..pages_needed {
            let page = bitmap_alloc(&mut self.free_pages)?;
            let tick = unsafe { crate::trap::TICK_COUNT as u64 };
            self.entries[page] = KvPageEntry {
                token_start: 0, token_count: 0, gpu_phys_addr: 0,
                dirty: true, ref_count: 1, allocated: true, last_access_tick: tick,
            };
            pages.push(page);
        }
        Some(pages)
    }

    pub fn free_pages(&mut self, pages: &[usize]) {
        for &page in pages {
            if page >= MAX_KV_PAGES || !self.entries[page].allocated { continue; }
            let entry = &mut self.entries[page];
            entry.ref_count = entry.ref_count.saturating_sub(1);
            if entry.ref_count == 0 {
                entry.allocated = false; entry.dirty = false;
                entry.token_start = 0; entry.token_count = 0;
                entry.gpu_phys_addr = 0; entry.last_access_tick = 0;
                bitmap_free(&mut self.free_pages, page);
            }
        }
    }

    pub fn share_pages(&mut self, pages: &[usize]) -> Result<(), &'static str> {
        for &page in pages {
            if page >= MAX_KV_PAGES || !self.entries[page].allocated {
                return Err("invalid or unallocated page");
            }
            let entry = &mut self.entries[page];
            entry.ref_count = entry.ref_count.saturating_add(1);
            if entry.ref_count > 10 { entry.ref_count = 10; }
        }
        Ok(())
    }

    pub fn evict_lru(&mut self, target_count: usize) -> usize {
        let mut evicted = 0;
        for _ in 0..target_count {
            let oldest = self.find_lru_evictable();
            match oldest {
                Some(page) => {
                    if self.entries[page].dirty { self.entries[page].dirty = false; }
                    self.entries[page].allocated = false;
                    self.entries[page].ref_count = 0;
                    self.entries[page].gpu_phys_addr = 0;
                    bitmap_free(&mut self.free_pages, page);
                    evicted += 1;
                }
                None => break,
            }
        }
        evicted
    }

    fn find_lru_evictable(&self) -> Option<usize> {
        let mut oldest_page = None;
        let mut oldest_tick = u64::MAX;
        for i in 0..self.total_pages {
            let entry = &self.entries[i];
            if entry.allocated && entry.ref_count == 1 && entry.last_access_tick < oldest_tick {
                oldest_tick = entry.last_access_tick;
                oldest_page = Some(i);
            }
        }
        oldest_page
    }

    pub fn writeback_dirty(&mut self, _gpu_id: u32) -> usize {
        let mut written = 0;
        for i in 0..self.total_pages {
            if self.entries[i].allocated && self.entries[i].dirty {
                self.entries[i].dirty = false;
                written += 1;
            }
        }
        written
    }

    pub fn page_in(&mut self, _gpu_id: u32, page: usize, _cpu_addr: usize) {
        if page >= MAX_KV_PAGES || !self.entries[page].allocated { return; }
        self.entries[page].dirty = false;
        self.entries[page].last_access_tick = unsafe { crate::trap::TICK_COUNT as u64 };
    }

    pub fn utilization(&self) -> f32 {
        let in_use = self.total_pages.saturating_sub(bitmap_free_count(&self.free_pages));
        if self.total_pages == 0 { return 0.0; }
        (in_use as f32) / (self.total_pages as f32)
    }

    pub fn dirty_page_count(&self) -> usize {
        let mut count = 0;
        for i in 0..self.total_pages {
            if self.entries[i].allocated && self.entries[i].dirty { count += 1; }
        }
        count
    }

    pub fn allocated_page_count(&self) -> usize {
        self.total_pages.saturating_sub(bitmap_free_count(&self.free_pages))
    }

    pub fn total_page_count(&self) -> usize { self.total_pages }
}

pub(crate) static mut KV_PAGE_TABLE: KvPageTable = KvPageTable::new(256);

pub fn kv_alloc_pages(token_count: usize) -> Option<Vec<usize>> {
    unsafe { KV_PAGE_TABLE.alloc_pages(token_count) }
}

pub fn kv_free_pages(pages: &[usize]) {
    unsafe { KV_PAGE_TABLE.free_pages(pages) }
}

pub fn kv_share_pages(pages: &[usize]) -> Result<(), &'static str> {
    unsafe { KV_PAGE_TABLE.share_pages(pages) }
}

pub fn kv_evict_lru(target_count: usize) -> usize {
    unsafe { KV_PAGE_TABLE.evict_lru(target_count) }
}

pub fn kv_writeback_dirty(gpu_id: u32) -> usize {
    unsafe { KV_PAGE_TABLE.writeback_dirty(gpu_id) }
}

pub fn kv_page_in(gpu_id: u32, page: usize, cpu_addr: usize) {
    unsafe { KV_PAGE_TABLE.page_in(gpu_id, page, cpu_addr) }
}

pub fn kv_utilization() -> f32 {
    unsafe { KV_PAGE_TABLE.utilization() }
}

pub fn kv_dirty_page_count() -> usize {
    unsafe { KV_PAGE_TABLE.dirty_page_count() }
}

pub fn kv_allocated_page_count() -> usize {
    unsafe { KV_PAGE_TABLE.allocated_page_count() }
}
