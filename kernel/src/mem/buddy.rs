use super::layout::PAGE_SIZE;
use spin::Mutex;

const MAX_ORDER: usize = 12; // 2^12 * 4KB = 16MB max block
const FREE_LIST_COUNT: usize = MAX_ORDER + 1;

struct BuddyInner {
    free_lists: [Option<usize>; FREE_LIST_COUNT],
    base: usize,
    end: usize,
    total_pages: usize,
    allocated_pages: usize,
}

pub struct BuddyAllocator {
    inner: Mutex<BuddyInner>,
}

static ALLOCATOR: BuddyAllocator = BuddyAllocator {
    inner: Mutex::new(BuddyInner {
        free_lists: [None; FREE_LIST_COUNT],
        base: 0,
        end: 0,
        total_pages: 0,
        allocated_pages: 0,
    }),
};

/// node_next/set_node_next take a PAGE INDEX and a BASE address, converting to
/// a physical address via base + page * PAGE_SIZE before reading/writing.
unsafe fn node_next(page: usize, base: usize) -> Option<usize> {
    let ptr = (base + page * PAGE_SIZE) as *const usize;
    let val = ptr.read_volatile();
    if val == 0 { None } else { Some(val) }
}

unsafe fn set_node_next(page: usize, next: Option<usize>, base: usize) {
    let ptr = (base + page * PAGE_SIZE) as *mut usize;
    ptr.write_volatile(next.unwrap_or(0));
}

impl BuddyInner {
    fn addr_to_page(&self, addr: usize) -> usize {
        (addr - self.base) / PAGE_SIZE
    }

    fn page_to_addr(&self, page: usize) -> usize {
        self.base + page * PAGE_SIZE
    }

    fn has_buddy(&self, page: usize, order: usize) -> bool {
        let block_size = 1 << order;
        let buddy_page = page ^ block_size;
        let buddy_addr = self.page_to_addr(buddy_page);
        let buddy_end = buddy_addr + (block_size * PAGE_SIZE);
        buddy_addr >= self.base && buddy_end <= self.end
    }

    unsafe fn push_free(&mut self, page: usize, order: usize) {
        set_node_next(page, self.free_lists[order], self.base);
        self.free_lists[order] = Some(page);
    }

    unsafe fn pop_free(&mut self, order: usize) -> Option<usize> {
        let page = self.free_lists[order];
        if let Some(p) = page {
            self.free_lists[order] = node_next(p, self.base);
        }
        page
    }

    unsafe fn split(&mut self, mut order: usize, target_order: usize) -> usize {
        while order > target_order {
            order -= 1;
            let buddy_offset = 1 << order;
            let block_start = self.pop_free(order + 1).unwrap();
            self.push_free(block_start + buddy_offset, order);
            if order > target_order {
                self.push_free(block_start, order);
            } else {
                return block_start;
            }
        }
        unreachable!()
    }

    unsafe fn try_merge(&mut self, mut page: usize, mut order: usize) {
        while order < MAX_ORDER {
            let buddy_page = page ^ (1 << order);
            if !self.has_buddy(page, order) {
                break;
            }
            let mut prev: Option<usize> = None;
            let mut curr = self.free_lists[order];
            let mut found = false;
            while let Some(p) = curr {
                if p == buddy_page {
                    found = true;
                    break;
                }
                prev = curr;
                curr = node_next(p, self.base);
            }
            if !found {
                break;
            }
            let buddy_next = node_next(buddy_page, self.base);
            if let Some(prev_p) = prev {
                set_node_next(prev_p, buddy_next, self.base);
            } else {
                self.free_lists[order] = buddy_next;
            }
            page = page & !(1 << order);
            order += 1;
        }
        self.push_free(page, order);
    }

    pub fn alloc_pages(&mut self, order: usize) -> Option<usize> {
        if order > MAX_ORDER {
            return None;
        }
        unsafe {
            for o in order..=MAX_ORDER {
                if self.free_lists[o].is_some() {
                    if o > order {
                        // split() will pop from free_lists[o] itself
                        let lower = self.split(o, order);
                        self.allocated_pages += 1 << order;
                        return Some(lower);
                    } else {
                        let page = self.pop_free(o).unwrap();
                        self.allocated_pages += 1 << o;
                        return Some(page);
                    }
                }
            }
            None
        }
    }

    pub fn free_pages(&mut self, page: usize, order: usize) {
        if order > MAX_ORDER {
            return;
        }
        unsafe {
            self.allocated_pages -= 1 << order;
            self.try_merge(page, order);
        }
    }

    pub fn allocated_count(&self) -> usize {
        self.allocated_pages
    }

    pub fn total_pages(&self) -> usize {
        self.total_pages
    }
}

pub fn init(base: usize, end: usize) {
    let mut inner = ALLOCATOR.inner.lock();
    inner.base = base;
    inner.end = end;
    let total_size = end - base;
    inner.total_pages = total_size / PAGE_SIZE;
    inner.allocated_pages = 0;
    inner.free_lists = [None; FREE_LIST_COUNT];

    let mut remaining = inner.total_pages;
    let mut offset = 0usize;
    let mut order = MAX_ORDER;
    loop {
        let block_pages = 1 << order;
        if remaining >= block_pages {
            unsafe { inner.push_free(offset, order); }
            offset += block_pages;
            remaining -= block_pages;
        } else if order > 0 {
            order -= 1;
        } else {
            break;
        }
    }
}

/// Allocate a single 4KB physical page. Returns physical address.
pub fn alloc_page() -> Option<usize> {
    let mut inner = ALLOCATOR.inner.lock();
    inner.alloc_pages(0).map(|p| p * PAGE_SIZE + inner.base)
}

/// Allocate 2^order contiguous pages. Returns physical address.
pub fn alloc_pages(order: usize) -> Option<usize> {
    let mut inner = ALLOCATOR.inner.lock();
    inner.alloc_pages(order).map(|p| p * PAGE_SIZE + inner.base)
}

/// Free a physical page allocated at `addr` with the given `order`.
pub fn free_page(addr: usize, order: usize) {
    let mut inner = ALLOCATOR.inner.lock();
    let page = (addr - inner.base) / PAGE_SIZE;
    inner.free_pages(page, order);
}

/// Number of allocated pages (for debugging)
pub fn allocated_pages() -> usize {
    ALLOCATOR.inner.lock().allocated_count()
}

pub fn total_pages() -> usize {
    ALLOCATOR.inner.lock().total_pages()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(align(16384))]
    struct AlignedBuf([u8; 1024 * 1024]);

    static mut TEST_MEM: AlignedBuf = AlignedBuf([0; 1024 * 1024]);

    /// Helper to re-init the allocator with fresh test memory before each test.
    /// Note: because ALLOCATOR is a global static, tests that run in the same
    /// process see the same instance. We work around that by re-initializing
    /// with a fresh region and relying on the mutex for ordering.
    unsafe fn test_init() -> (usize, usize) {
        let base = core::ptr::addr_of!(TEST_MEM) as usize;
        let end = base + 1024 * 1024;
        init(base, end);
        (base, end)
    }

    #[test]
    fn test_alloc_free_single_page() {
        let (base, end) = unsafe { test_init() };
        let page = alloc_page().expect("should allocate one page");
        assert!(page >= base && page < end);
        assert_eq!(allocated_pages(), 1);
        free_page(page, 0);
        assert_eq!(allocated_pages(), 0);
    }

    #[test]
    fn test_alloc_multiple_pages() {
        unsafe { test_init(); }
        let p1 = alloc_page().unwrap();
        let p2 = alloc_page().unwrap();
        assert_ne!(p1, p2);
        assert_eq!(allocated_pages(), 2);
        free_page(p1, 0);
        free_page(p2, 0);
        assert_eq!(allocated_pages(), 0);
    }

    #[test]
    fn test_alloc_order_2() {
        unsafe { test_init(); }
        let block = alloc_pages(2).expect("should allocate 4 pages");
        assert_eq!(block % (PAGE_SIZE * 4), 0);
        free_page(block, 2);
        assert_eq!(allocated_pages(), 0);
    }

    #[test]
    fn test_exhaust_then_free() {
        unsafe { test_init(); }
        let total = total_pages();
        // total pages for 1MB test memory = 256, use a fixed-size array
        let mut pages = [0usize; 256];
        let mut count = 0;
        for _ in 0..total {
            pages[count] = alloc_page().expect("should allocate");
            count += 1;
        }
        assert_eq!(allocated_pages(), total);
        assert!(alloc_page().is_none());
        for i in 0..count {
            free_page(pages[i], 0);
        }
        assert_eq!(allocated_pages(), 0);
    }

    #[test]
    fn test_merge_after_free() {
        unsafe { test_init(); }
        let p1 = alloc_page().unwrap();
        let p2 = alloc_page().unwrap();
        free_page(p1, 0);
        free_page(p2, 0);
        // After merging, we should be able to allocate an order-1 block
        let block = alloc_pages(1);
        assert!(block.is_some());
        free_page(block.unwrap(), 1);
        assert_eq!(allocated_pages(), 0);
    }
}
