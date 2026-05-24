// V25: MCS (Mellor-Crummey Scott) lock — avoids cache-line bouncing
// by having each spinner spin on its own local node.
//
// Usage:
//   let lock = McsLock::new();
//   let mut node = McsNode::new();
//   lock.lock(&mut node);
//   // critical section ...
//   lock.unlock(&mut node);

#![allow(dead_code)]

use core::sync::atomic::{AtomicPtr, Ordering};

/// A node in the MCS lock queue — one per lock user.
/// Each thread must provide its own McsNode on the stack (or in thread-local
/// storage) so that spinning does not contend a shared cache line.
#[repr(C)]
pub struct McsNode {
    next: *mut McsNode,
    locked: bool,
}

impl McsNode {
    pub const fn new() -> Self {
        McsNode {
            next: core::ptr::null_mut(),
            locked: false,
        }
    }
}

/// MCS lock — scalable spinlock that avoids cache-line bouncing.
/// The `tail` pointer tracks the last node in the queue.
pub struct McsLock {
    tail: AtomicPtr<McsNode>,
}

impl McsLock {
    pub const fn new() -> Self {
        McsLock {
            tail: AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    /// Acquire the lock.
    /// `node` must be a mut reference owned exclusively by the caller for the
    /// duration of the critical section.
    pub fn lock(&self, node: &mut McsNode) {
        node.next = core::ptr::null_mut();
        node.locked = true;
        let prev = self.tail.swap(node as *mut McsNode, Ordering::AcqRel);
        if !prev.is_null() {
            // There is a predecessor — chain ourselves and spin.
            unsafe {
                (*prev).next = node as *mut McsNode;
            }
            while node.locked {
                core::hint::spin_loop();
            }
        }
        // No predecessor — we hold the lock immediately.
    }

    /// Release the lock.
    /// `node` must be the same node passed to `lock()`.
    pub fn unlock(&self, node: &mut McsNode) {
        if node.next.is_null() {
            // No known successor. Try to atomically null the tail.
            if self
                .tail
                .compare_exchange(
                    node as *mut McsNode,
                    core::ptr::null_mut(),
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                // We removed ourselves; no successor to wake.
                return;
            }
            // A successor is about to appear — spin until it links in.
            while node.next.is_null() {
                core::hint::spin_loop();
            }
        }
        // Pass the lock to the successor.
        unsafe {
            (*node.next).locked = false;
        }
    }
}
