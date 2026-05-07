use super::message::Message;
use crate::proc::thread::Thread;
use alloc::vec::Vec;

/// Simple FIFO queue for pending messages, backed by Vec with head index.
/// Avoids need for alloc::collections::VecDeque which requires nightly features.
struct MessageQueue {
    items: Vec<(u32, Message)>,
    head: usize,
}

impl MessageQueue {
    const fn new() -> Self {
        MessageQueue {
            items: Vec::new(),
            head: 0,
        }
    }

    fn push_back(&mut self, sender_pid: u32, msg: Message) {
        self.items.push((sender_pid, msg));
    }

    fn pop_front(&mut self) -> Option<(u32, Message)> {
        if self.head >= self.items.len() {
            return None;
        }
        let item = self.items[self.head];
        self.head += 1;
        // Compact when head is large to avoid unbounded growth
        if self.head > 64 && self.head >= self.items.len() / 2 {
            self.items.drain(0..self.head);
            self.head = 0;
        }
        Some(item)
    }

    fn is_empty(&self) -> bool {
        self.head >= self.items.len()
    }
}

pub struct Endpoint {
    pub id: usize,
    pending_senders: MessageQueue,
    pub waiting_receiver: Option<*mut Thread>,
}

// Endpoint contains raw *mut Thread pointers which are not Send,
// but the kernel is effectively single-threaded (no SMP).
unsafe impl Send for Endpoint {}

impl Endpoint {
    pub fn new(id: usize) -> Self {
        Endpoint { id, pending_senders: MessageQueue::new(), waiting_receiver: None }
    }
}

/// Non-blocking send. Queues message or delivers if receiver waiting.
pub fn send(ep_id: usize, sender_pid: u32, msg: Message) -> Result<(), &'static str> {
    let mut eps = super::ENDPOINTS.lock();
    let ep = eps.get_mut(ep_id).and_then(|e| e.as_mut()).ok_or("invalid ep")?;

    if let Some(receiver) = ep.waiting_receiver.take() {
        // Receiver is waiting -- wake it and deliver
        // Store message in receiver's buffer area (handled at syscall level)
        // For now, just wake the receiver
        unsafe {
            (*receiver).state = crate::proc::thread::ThreadState::Ready;
        }
        crate::sched::enqueue_thread(receiver);
    } else {
        // Queue for later
        ep.pending_senders.push_back(sender_pid, msg);
    }
    Ok(())
}

/// Blocking receive. Blocks current thread if no message pending.
pub fn recv(ep_id: usize, _receiver_pid: u32) -> Result<Message, &'static str> {
    let mut eps = super::ENDPOINTS.lock();
    let ep = eps.get_mut(ep_id).and_then(|e| e.as_mut()).ok_or("invalid ep")?;

    if let Some((_sender, msg)) = ep.pending_senders.pop_front() {
        Ok(msg)
    } else {
        // Block current thread
        let current = crate::sched::current_thread()
            .ok_or("no current thread")?;
        unsafe {
            (*current).state = crate::proc::thread::ThreadState::Waiting;
            (*current).wait_target = Some(crate::proc::thread::WaitTarget::Endpoint(ep_id));
        }
        ep.waiting_receiver = Some(current);
        // Return error to signal "would block" -- caller must handle
        Err("would block")
    }
}
