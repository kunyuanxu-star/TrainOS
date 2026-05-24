// V22.6 I/O Scheduler Framework
//
// Provides a trait-based I/O scheduling abstraction for block devices.
// Implementations:
//   NoopScheduler       — simple FIFO queue
//   DeadlineScheduler   — read-biased with per-request deadlines

use crate::device::merge::BlockRequest;

const SCHED_MAX: usize = 256;

// Timer tick rate is ~10 ms in the default kernel configuration.
const READ_DEADLINE_TICKS: u64 = 50;    // 500 ms
const WRITE_DEADLINE_TICKS: u64 = 500;  // 5 s

/// I/O Scheduler trait.
///
/// All implementations must work without heap allocation.
pub trait IoScheduler {
    /// Enqueue a block request. `now` is the current timer tick.
    fn enqueue(&mut self, req: BlockRequest, now: u64);

    /// Dequeue the next request to dispatch. `now` is the current timer tick.
    /// Returns `None` when the queue is empty.
    fn dequeue(&mut self, now: u64) -> Option<BlockRequest>;

    /// Return the number of pending requests.
    fn count(&self) -> usize;
}

// ─── Noop (FIFO) Scheduler ───────────────────────────────────────────────────

pub struct NoopScheduler {
    queue: [BlockRequest; SCHED_MAX],
    head: usize,
    tail: usize,
}

impl NoopScheduler {
    pub const fn new() -> Self {
        NoopScheduler {
            queue: [BlockRequest::empty(); SCHED_MAX],
            head: 0,
            tail: 0,
        }
    }
}

impl IoScheduler for NoopScheduler {
    fn enqueue(&mut self, req: BlockRequest, _now: u64) {
        if self.tail - self.head < SCHED_MAX {
            self.queue[self.tail % SCHED_MAX] = req;
            self.tail += 1;
        }
    }

    fn dequeue(&mut self, _now: u64) -> Option<BlockRequest> {
        if self.head < self.tail {
            let idx = self.head % SCHED_MAX;
            let req = self.queue[idx];
            self.head += 1;
            Some(req)
        } else {
            None
        }
    }

    fn count(&self) -> usize {
        self.tail - self.head
    }
}

// ─── Deadline Scheduler ──────────────────────────────────────────────────────

pub struct DeadlineScheduler {
    read_queue: [BlockRequest; SCHED_MAX],
    read_deadlines: [u64; SCHED_MAX],
    read_head: usize,
    read_tail: usize,

    write_queue: [BlockRequest; SCHED_MAX],
    write_deadlines: [u64; SCHED_MAX],
    write_head: usize,
    write_tail: usize,

    /// Number of consecutive reads dispatched since the last write.
    /// When this exceeds STARVE_LIMIT, a write is forced.
    starve_counter: usize,
}

const STARVE_LIMIT: usize = 16;

impl DeadlineScheduler {
    pub const fn new() -> Self {
        DeadlineScheduler {
            read_queue: [BlockRequest::empty(); SCHED_MAX],
            read_deadlines: [0; SCHED_MAX],
            read_head: 0,
            read_tail: 0,

            write_queue: [BlockRequest::empty(); SCHED_MAX],
            write_deadlines: [0; SCHED_MAX],
            write_head: 0,
            write_tail: 0,

            starve_counter: 0,
        }
    }
}

impl IoScheduler for DeadlineScheduler {
    fn enqueue(&mut self, req: BlockRequest, now: u64) {
        if req.write {
            if self.write_tail - self.write_head < SCHED_MAX {
                let idx = self.write_tail % SCHED_MAX;
                self.write_queue[idx] = req;
                self.write_deadlines[idx] = now + WRITE_DEADLINE_TICKS;
                self.write_tail += 1;
            }
        } else {
            if self.read_tail - self.read_head < SCHED_MAX {
                let idx = self.read_tail % SCHED_MAX;
                self.read_queue[idx] = req;
                self.read_deadlines[idx] = now + READ_DEADLINE_TICKS;
                self.read_tail += 1;
            }
        }
    }

    fn dequeue(&mut self, now: u64) -> Option<BlockRequest> {
        // 1. Check for expired deadlines (both read and write)
        let mut i = self.read_head;
        while i < self.read_tail {
            let idx = i % SCHED_MAX;
            if now >= self.read_deadlines[idx] {
                let req = self.read_queue[idx];
                // Compact the read queue by shifting remaining entries
                self.read_head = i + 1;
                self.starve_counter = 0;
                return Some(req);
            }
            i += 1;
        }

        let mut i = self.write_head;
        while i < self.write_tail {
            let idx = i % SCHED_MAX;
            if now >= self.write_deadlines[idx] {
                let req = self.write_queue[idx];
                self.write_head = i + 1;
                self.starve_counter = 0;
                return Some(req);
            }
            i += 1;
        }

        // 2. Prefer reads over writes, but avoid write starvation
        if self.read_head < self.read_tail {
            if self.starve_counter >= STARVE_LIMIT && self.write_head < self.write_tail {
                // Force a write to prevent starvation
                self.starve_counter = 0;
                let idx = self.write_head % SCHED_MAX;
                let req = self.write_queue[idx];
                self.write_head += 1;
                return Some(req);
            }
            let idx = self.read_head % SCHED_MAX;
            let req = self.read_queue[idx];
            self.read_head += 1;
            self.starve_counter += 1;
            Some(req)
        } else if self.write_head < self.write_tail {
            self.starve_counter = 0;
            let idx = self.write_head % SCHED_MAX;
            let req = self.write_queue[idx];
            self.write_head += 1;
            Some(req)
        } else {
            None
        }
    }

    fn count(&self) -> usize {
        (self.read_tail - self.read_head) + (self.write_tail - self.write_head)
    }
}
