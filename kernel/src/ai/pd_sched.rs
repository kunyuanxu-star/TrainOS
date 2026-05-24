// V34: P/D (Prefill/Decode) separated scheduling for LLM inference
//
// Prefill: compute-bound, high throughput, batch-process prompts
// Decode: memory-bound, low latency, auto-regressive token generation

use alloc::vec::Vec;

pub(crate) const MAX_PD_WORKLOADS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PdRole { Prefill, Decode }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PdWorkloadState { Queued, Prefilling, Decoding, Preempted, Completed }

#[derive(Clone, Copy)]
pub(crate) struct PdWorkload {
    pub workload_id: usize,
    pub role: PdRole,
    pub gpu_id: u32,
    pub model_id: u32,
    pub kv_cache_pages: [usize; 64],
    pub kv_cache_page_count: usize,
    pub prefill_workload_id: Option<usize>,
    pub priority: u8,
    pub state: PdWorkloadState,
    pub prefill_ctx: [u8; 64],
    pub prefill_ctx_len: usize,
    pub submit_tick: u64,
}

pub(crate) struct PdScheduler {
    pub workloads: [Option<PdWorkload>; MAX_PD_WORKLOADS],
    next_id: usize,
    pub prefill_count: u64,
    pub decode_count: u64,
}

impl PdScheduler {
    pub const fn new() -> Self {
        PdScheduler {
            workloads: [None; MAX_PD_WORKLOADS],
            next_id: 1,
            prefill_count: 0,
            decode_count: 0,
        }
    }

    pub fn submit_pd(&mut self, prefill_ctx: &[u8], model_id: u32, gpu_id: u32) -> Option<(usize, usize)> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let mut p_slot = None;
        for i in (0..MAX_PD_WORKLOADS).step_by(2) {
            if self.workloads[i].is_none() && self.workloads[i + 1].is_none() {
                p_slot = Some(i);
                break;
            }
        }
        let p_slot = p_slot?;
        let d_slot = p_slot + 1;
        let ctx_len = core::cmp::min(prefill_ctx.len(), 64);
        let mut ctx_buf = [0u8; 64];
        if ctx_len > 0 { ctx_buf[..ctx_len].copy_from_slice(&prefill_ctx[..ctx_len]); }
        let tick = unsafe { crate::trap::TICK_COUNT as u64 };
        let prefill_id = id;
        let decode_id = id.wrapping_add(1);

        // Use split_at_mut to get non-overlapping mutable references
        let (left, right) = self.workloads.split_at_mut(d_slot);
        left[p_slot] = Some(PdWorkload {
            workload_id: prefill_id, role: PdRole::Prefill, gpu_id, model_id,
            kv_cache_pages: [0usize; 64], kv_cache_page_count: 0,
            prefill_workload_id: None, priority: 1, state: PdWorkloadState::Queued,
            prefill_ctx: ctx_buf, prefill_ctx_len: ctx_len, submit_tick: tick,
        });
        right[0] = Some(PdWorkload {
            workload_id: decode_id, role: PdRole::Decode, gpu_id, model_id,
            kv_cache_pages: [0usize; 64], kv_cache_page_count: 0,
            prefill_workload_id: Some(prefill_id), priority: 3,
            state: PdWorkloadState::Queued,
            prefill_ctx: [0u8; 64], prefill_ctx_len: 0, submit_tick: tick,
        });
        self.prefill_count = self.prefill_count.wrapping_add(1);
        self.decode_count = self.decode_count.wrapping_add(1);
        Some((prefill_id, decode_id))
    }

    pub fn next_decode_step(&mut self) -> Option<usize> {
        let mut best: Option<(usize, u8, u64)> = None;
        for i in 0..MAX_PD_WORKLOADS {
            if let Some(wl) = self.workloads[i] {
                if !matches!(wl.role, PdRole::Decode) { continue; }
                if !matches!(wl.state, PdWorkloadState::Queued) { continue; }
                match best {
                    None => best = Some((i, wl.priority, wl.submit_tick)),
                    Some((_, bp, bt)) => {
                        if wl.priority > bp || (wl.priority == bp && wl.submit_tick < bt) {
                            best = Some((i, wl.priority, wl.submit_tick));
                        }
                    }
                }
            }
        }
        if let Some((idx, _, _)) = best {
            if let Some(ref mut wl) = self.workloads[idx] {
                wl.state = PdWorkloadState::Decoding;
                Some(wl.workload_id)
            } else { None }
        } else { None }
    }

    pub fn next_prefill_batch(&mut self) -> Vec<usize> {
        let mut batch = Vec::new();
        for i in 0..MAX_PD_WORKLOADS {
            if self.workloads[i].is_none() { continue; }
            let (is_prefill, is_queued) = match self.workloads[i] {
                Some(wl) => (matches!(wl.role, PdRole::Prefill), matches!(wl.state, PdWorkloadState::Queued)),
                None => (false, false),
            };
            if is_prefill && is_queued {
                batch.push(self.workloads[i].unwrap().workload_id);
                self.workloads[i].as_mut().unwrap().state = PdWorkloadState::Prefilling;
            }
        }
        batch
    }

    pub fn preempt_decode(&mut self, workload_id: usize) {
        for i in 0..MAX_PD_WORKLOADS {
            if let Some(ref mut wl) = self.workloads[i] {
                if wl.workload_id == workload_id && matches!(wl.role, PdRole::Decode)
                    && matches!(wl.state, PdWorkloadState::Decoding)
                { wl.state = PdWorkloadState::Preempted; return; }
            }
        }
    }

    pub fn resume_decode(&mut self, workload_id: usize) {
        for i in 0..MAX_PD_WORKLOADS {
            if let Some(ref mut wl) = self.workloads[i] {
                if wl.workload_id == workload_id && matches!(wl.role, PdRole::Decode)
                    && matches!(wl.state, PdWorkloadState::Preempted)
                { wl.state = PdWorkloadState::Decoding; return; }
            }
        }
    }

    pub fn get_workload(&self, workload_id: usize) -> Option<&PdWorkload> {
        for i in 0..MAX_PD_WORKLOADS {
            if let Some(ref wl) = self.workloads[i] {
                if wl.workload_id == workload_id { return Some(wl); }
            }
        }
        None
    }

    pub fn get_workload_mut(&mut self, workload_id: usize) -> Option<&mut PdWorkload> {
        let idx = self.workloads.iter().position(|entry| {
            entry.as_ref().map_or(false, |wl| wl.workload_id == workload_id)
        });
        match idx {
            Some(i) => self.workloads[i].as_mut(),
            None => None,
        }
    }

    pub fn count_by_state(&self, role: PdRole, state: PdWorkloadState) -> usize {
        let mut count = 0;
        for i in 0..MAX_PD_WORKLOADS {
            if let Some(ref wl) = self.workloads[i] {
                if wl.role == role && wl.state == state { count += 1; }
            }
        }
        count
    }
}

pub(crate) static mut PD_SCHEDULER: PdScheduler = PdScheduler::new();

pub fn pd_submit(prefill_ctx: &[u8], model_id: u32, gpu_id: u32) -> Option<(usize, usize)> {
    unsafe { PD_SCHEDULER.submit_pd(prefill_ctx, model_id, gpu_id) }
}

pub fn pd_next_decode_step() -> Option<usize> {
    unsafe { PD_SCHEDULER.next_decode_step() }
}

pub fn pd_next_prefill_batch() -> Vec<usize> {
    unsafe { PD_SCHEDULER.next_prefill_batch() }
}

pub fn pd_preempt_decode(workload_id: usize) {
    unsafe { PD_SCHEDULER.preempt_decode(workload_id) }
}

pub fn pd_resume_decode(workload_id: usize) {
    unsafe { PD_SCHEDULER.resume_decode(workload_id) }
}
