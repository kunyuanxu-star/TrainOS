// V22.4 Block Request Merging
//
// Coalesces adjacent block I/O requests into larger requests
// to reduce dispatch overhead and improve throughput.

use core::cmp::Ordering;

/// A single block I/O request.
#[derive(Copy, Clone, Debug)]
pub struct BlockRequest {
    pub sector: u64,
    pub count: u32,
    pub buf: u64,    // physical address of data buffer
    pub write: bool,
}

impl BlockRequest {
    pub const fn empty() -> Self {
        BlockRequest { sector: 0, count: 0, buf: 0, write: false }
    }
}

/// Merge adjacent block requests.
///
/// Sorts the input slice by sector number, then walks the sorted list
/// merging requests that are contiguous (i.e. `prev.sector + prev.count
/// == next.sector`) and have the same direction (read/write).
///
/// Returns a fixed-size array of up to 128 merged requests. Unused
/// entries are zeroed (`BlockRequest::empty()`).
pub fn merge_requests(reqs: &[BlockRequest]) -> [BlockRequest; 128] {
    let n = reqs.len();
    if n == 0 {
        return [BlockRequest::empty(); 128];
    }

    // Clamp to 128 and copy into local fixed-size array for sorting
    let count = n.min(128);
    let mut sorted = [BlockRequest::empty(); 128];
    let mut i = 0;
    while i < count {
        sorted[i] = reqs[i];
        i += 1;
    }

    // Bubble sort by sector (small N, no heap)
    let mut i = 0;
    while i < count {
        let mut j = 0;
        while j + 1 < count - i {
            if sorted[j].sector > sorted[j + 1].sector {
                let tmp = sorted[j];
                sorted[j] = sorted[j + 1];
                sorted[j + 1] = tmp;
            }
            j += 1;
        }
        i += 1;
    }

    // Merge adjacent requests with same direction
    let mut write_idx = 0;
    let mut i = 1;
    while i < count {
        let prev = sorted[write_idx];
        let cur = sorted[i];
        let prev_end = prev.sector + prev.count as u64;

        if prev.write == cur.write && prev_end == cur.sector {
            // Coalesce: extend previous request, keep its buffer start
            // (the caller is responsible for scatter/gather if needed)
            sorted[write_idx].count = prev.count + cur.count;
        } else {
            write_idx += 1;
            sorted[write_idx] = cur;
        }
        i += 1;
    }

    let merged = write_idx + 1;

    // Zero out remaining entries beyond merged count
    let mut i = merged;
    while i < 128 {
        sorted[i] = BlockRequest::empty();
        i += 1;
    }

    sorted
}
