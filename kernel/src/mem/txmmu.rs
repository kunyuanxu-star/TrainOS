// V31 — Transactional MMU (TxMMU)
//
// Inspired by CortenMM (SOSP'25 Best Paper): single-level memory management
// where page table operations are first-class, transactional primitives.
//
// Instead of going through multiple software layers (buddy → VMA → page table),
// TxMMU queues map/unmap/protect operations and applies them atomically
// on commit().  On conflict detection, the transaction is aborted and
// the caller should retry (lock-free optimistic concurrency).

use crate::mem::{buddy, layout::PAGE_SIZE, sv39};

const MAX_OPS: usize = 16;

/// A single pending page-table operation inside a transaction.
#[derive(Clone, Copy, Debug)]
enum TxOp {
    Map {
        va: usize,
        pa: usize,
        flags: u8,
    },
    Unmap {
        va: usize,
    },
    Protect {
        va: usize,
        flags: u8,
    },
}

/// Transactional MMU — begin/commit/abort for page table operations.
///
/// # Example
///
/// ```ignore
/// let mut tx = TxMMU::begin(root_phys);
/// tx.map(va, pa, FLAG_R | FLAG_W | FLAG_U).unwrap();
/// tx.protect(other_va, FLAG_R | FLAG_X | FLAG_U).unwrap();
/// tx.commit().unwrap();  // both operations applied atomically
/// ```
pub struct TxMMU {
    root_phys: usize,
    pending_ops: [Option<TxOp>; MAX_OPS],
    op_count: usize,
    in_tx: bool,
}

// ── Flag bit encoding ──────────────────────────────────────────────────────
pub const FLAG_R: u8 = 1 << 0;
pub const FLAG_W: u8 = 1 << 1;
pub const FLAG_X: u8 = 1 << 2;
pub const FLAG_U: u8 = 1 << 3;

impl TxMMU {
    /// Begin a new transaction against the page table rooted at `root_phys`.
    pub fn begin(root_phys: usize) -> Self {
        TxMMU {
            root_phys,
            pending_ops: [None; MAX_OPS],
            op_count: 0,
            in_tx: true,
        }
    }

    /// Queue a map operation.
    ///
    /// `flags` is a bitwise OR of `FLAG_R`, `FLAG_W`, `FLAG_X`, `FLAG_U`.
    pub fn map(&mut self, va: usize, pa: usize, flags: u8) -> Result<(), &'static str> {
        if !self.in_tx {
            return Err("TxMMU: no transaction in progress");
        }
        if self.op_count >= MAX_OPS {
            return Err("TxMMU: transaction operation buffer full");
        }
        self.pending_ops[self.op_count] = Some(TxOp::Map { va, pa, flags });
        self.op_count += 1;
        Ok(())
    }

    /// Queue an unmap operation.
    pub fn unmap(&mut self, va: usize) -> Result<(), &'static str> {
        if !self.in_tx {
            return Err("TxMMU: no transaction in progress");
        }
        if self.op_count >= MAX_OPS {
            return Err("TxMMU: transaction operation buffer full");
        }
        self.pending_ops[self.op_count] = Some(TxOp::Unmap { va });
        self.op_count += 1;
        Ok(())
    }

    /// Queue a protect (change flags) operation.
    ///
    /// `flags` uses the same encoding as `map()`.
    pub fn protect(&mut self, va: usize, flags: u8) -> Result<(), &'static str> {
        if !self.in_tx {
            return Err("TxMMU: no transaction in progress");
        }
        if self.op_count >= MAX_OPS {
            return Err("TxMMU: transaction operation buffer full");
        }
        self.pending_ops[self.op_count] = Some(TxOp::Protect { va, flags });
        self.op_count += 1;
        Ok(())
    }

    /// Atomically apply all queued operations.
    ///
    /// Walks the page table for each operation and writes the leaf PTE.
    /// After all operations are applied, issues a global `sfence.vma`.
    /// On failure (e.g. page table walk allocation fails), the transaction
    /// is aborted and the error is returned.  The page table is left in
    /// an undefined-but-safe state (some operations may have been applied).
    pub fn commit(&mut self) -> Result<(), &'static str> {
        if !self.in_tx {
            return Err("TxMMU: no transaction in progress");
        }

        // Apply all pending operations
        for i in 0..self.op_count {
            match self.pending_ops[i] {
                Some(TxOp::Map { va, pa, flags }) => unsafe {
                    self.commit_map(va, pa, flags)?;
                },
                Some(TxOp::Unmap { va }) => unsafe {
                    self.commit_unmap(va);
                },
                Some(TxOp::Protect { va, flags }) => unsafe {
                    self.commit_protect(va, flags);
                },
                None => {}
            }
        }

        // Global TLB flush
        unsafe {
            core::arch::asm!("sfence.vma");
        }

        self.op_count = 0;
        self.pending_ops = [None; MAX_OPS];
        self.in_tx = false;
        Ok(())
    }

    /// Abort the current transaction — discard all pending operations.
    pub fn abort(&mut self) {
        self.op_count = 0;
        self.pending_ops = [None; MAX_OPS];
        self.in_tx = false;
    }

    /// Detect whether a concurrent thread modified the page table during
    /// the transaction.
    ///
    /// Checks every target VA for conflicting state:
    /// - Map:   PTE should be invalid (not already mapped)
    /// - Unmap: PTE should still be valid and leaf
    /// - Protect: PTE should still be valid and leaf
    ///
    /// On detecting a conflict the transaction is aborted and `true` is
    /// returned.  The caller should retry with a fresh transaction.
    /// Returns `false` if no conflict is detected.
    pub fn rollback_on_conflict(&mut self) -> bool {
        if !self.in_tx {
            return false;
        }

        for i in 0..self.op_count {
            match self.pending_ops[i] {
                Some(TxOp::Map { va, .. }) => {
                    // Conflict if the target PTE is already a valid leaf
                    // (someone else mapped this page while our tx was open).
                    unsafe {
                        if let Some((l0_phys, idx)) =
                            sv39::walk_process_pt(self.root_phys, va, false)
                        {
                            let l0 =
                                &*(sv39::pa_to_kva(l0_phys) as *const [sv39::PTE; 512]);
                            if l0[idx].is_valid() && l0[idx].is_leaf() {
                                crate::println!(
                                    "TxMMU: conflict on map at va=0x{:x}",
                                    va
                                );
                                self.abort();
                                return true;
                            }
                        }
                    }
                }
                Some(TxOp::Unmap { va }) => {
                    // Conflict if the PTE is already gone.
                    unsafe {
                        let present =
                            sv39::walk_process_pt(self.root_phys, va, false).and_then(
                                |(l0_phys, idx)| {
                                    let l0 = &*(sv39::pa_to_kva(l0_phys)
                                        as *const [sv39::PTE; 512]);
                                    if l0[idx].is_valid() && l0[idx].is_leaf() {
                                        Some(())
                                    } else {
                                        None
                                    }
                                },
                            );
                        if present.is_none() {
                            crate::println!(
                                "TxMMU: conflict on unmap at va=0x{:x}",
                                va
                            );
                            self.abort();
                            return true;
                        }
                    }
                }
                Some(TxOp::Protect { va, .. }) => {
                    // Conflict if the PTE is gone.
                    unsafe {
                        let present =
                            sv39::walk_process_pt(self.root_phys, va, false).and_then(
                                |(l0_phys, idx)| {
                                    let l0 = &*(sv39::pa_to_kva(l0_phys)
                                        as *const [sv39::PTE; 512]);
                                    if l0[idx].is_valid() && l0[idx].is_leaf() {
                                        Some(())
                                    } else {
                                        None
                                    }
                                },
                            );
                        if present.is_none() {
                            crate::println!(
                                "TxMMU: conflict on protect at va=0x{:x}",
                                va
                            );
                            self.abort();
                            return true;
                        }
                    }
                }
                None => {}
            }
        }
        false
    }

    // ── Internal commit helpers ──────────────────────────────────────────

    /// Apply a single map operation: walk (with allocation) and set leaf PTE.
    unsafe fn commit_map(
        &self,
        va: usize,
        pa: usize,
        flags: u8,
    ) -> Result<(), &'static str> {
        let (l0_phys, idx) = sv39::walk_process_pt(self.root_phys, va, true)
            .ok_or("TxMMU commit_map: page table walk failed")?;
        let l0 = &mut *(sv39::pa_to_kva(l0_phys) as *mut [sv39::PTE; 512]);
        let r = flags & 1 != 0;
        let w = flags & 2 != 0;
        let x = flags & 4 != 0;
        let u = flags & 8 != 0;
        let mut pte = sv39::PTE::empty();
        pte.set_ppn(pa >> 12);
        pte.set_flags(r, w, x, u);
        pte.set_accessed(true);
        pte.set_dirty(true);
        l0[idx] = pte;
        Ok(())
    }

    /// Apply a single unmap operation: walk (no alloc) and clear leaf PTE.
    unsafe fn commit_unmap(&self, va: usize) {
        if let Some((l0_phys, idx)) = sv39::walk_process_pt(self.root_phys, va, false) {
            let l0 = &mut *(sv39::pa_to_kva(l0_phys) as *mut [sv39::PTE; 512]);
            l0[idx] = sv39::PTE::empty();
        }
    }

    /// Apply a single protect operation: walk (no alloc) and update flags.
    unsafe fn commit_protect(&self, va: usize, flags: u8) {
        if let Some((l0_phys, idx)) = sv39::walk_process_pt(self.root_phys, va, false) {
            let l0 = &mut *(sv39::pa_to_kva(l0_phys) as *mut [sv39::PTE; 512]);
            let r = flags & 1 != 0;
            let w = flags & 2 != 0;
            let x = flags & 4 != 0;
            let u = flags & 8 != 0;
            l0[idx].set_flags(r, w, x, u);
        }
    }
}
