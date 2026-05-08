use crate::cap::ops;
use crate::cap::types::{CapType, ResourceData, Rights, RIGHT_RECV, RIGHT_SEND};
use crate::ipc;
use crate::ipc::message::{Message, MAX_PAYLOAD};

/// Check if the current process has an EP capability with the required rights.
/// V2.2: simplified check - returns true for all IPC (enforcement disabled).
/// Full enforcement will be enabled in V2.3 when services are updated with capability transfer.
fn check_ep_cap(_ep_id: usize, _required_rights: Rights) -> bool {
    // TODO: In V2.3, replace with proper check:
    // 1. Get current process's CNode
    // 2. Iterate all slots
    // 3. For each EP cap, get resource and check ep_id + rights
    true
}

/// Helper: store an EP capability slot in the calling process's CNode.
/// Returns true if stored successfully, false otherwise.
fn store_ep_cap(ep_res_id: usize) -> bool {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    let procs = crate::proc::PROCESSES.lock();
    let proc = match procs.iter().find(|p| p.pid == pid) {
        Some(p) => p,
        None => return false,
    };
    let cnode_id = proc.cnode_id;
    drop(procs);

    let res = match ops::get_resource(cnode_id) {
        Some(r) => r,
        None => return false,
    };
    if let ResourceData::CNode { ref slots } = &res.data {
        let mut slots = slots.lock();
        // Try to find an empty (Null) slot first
        for slot in slots.iter_mut() {
            if slot.cap_type == CapType::Null {
                slot.cap_type = CapType::EP;
                slot.rights = RIGHT_SEND | RIGHT_RECV;
                slot.resource_id = ep_res_id;
                return true;
            }
        }
        // If no empty slot, append
        slots.push(crate::cap::types::Slot {
            cap_type: CapType::EP,
            rights: RIGHT_SEND | RIGHT_RECV,
            resource_id: ep_res_id,
        });
        return true;
    }
    false
}

pub fn sys_ep_create() -> Result<usize, &'static str> {
    let ep_id = ipc::create_endpoint();

    // Create a capability resource for this EP and store it in the caller's CNode
    let ep_res_id = ops::alloc_resource(CapType::EP, ResourceData::EP { ep_id });
    store_ep_cap(ep_res_id);

    Ok(ep_id)
}

/// sys_send(ep_id: usize, opcode: u16, payload_ptr: usize, payload_len: usize) -> Result
/// payload: up to payload_len bytes at user-space payload_ptr (max 64)
pub fn sys_send(
    ep_id: usize,
    opcode: u16,
    payload_ptr: usize,
    payload_len: usize,
) -> Result<usize, &'static str> {
    if !check_ep_cap(ep_id, RIGHT_SEND) {
        return Err("no send cap");
    }
    let sender_pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);

    let mut msg = Message::new(sender_pid, opcode);

    // Copy payload from user space.
    // We are running with the user process's satp active, so user virtual
    // addresses are directly accessible. sstatus.SUM=1 permits S-mode
    // access to User (U=1) pages.
    if payload_ptr != 0 && payload_len > 0 {
        let len = core::cmp::min(payload_len, MAX_PAYLOAD);
        unsafe {
            let src = payload_ptr as *const u8;
            for i in 0..len {
                msg.payload[i] = core::ptr::read_volatile(src.add(i));
            }
            msg.payload_len = len;
        }
    }

    ipc::endpoint::send(ep_id, sender_pid, msg)?;
    Ok(0)
}

/// sys_recv(ep_id: usize, buf_ptr: usize, buf_len: usize) -> Result
/// Returns: (opcode << 24) | (sender_pid & 0x00FF_FFFF)
/// Also copies message payload to user buffer at buf_ptr (up to buf_len bytes).
/// Blocks until a message is received, retrying after being woken from wait.
pub fn sys_recv(ep_id: usize, buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if !check_ep_cap(ep_id, RIGHT_RECV) {
        return Err("no recv cap");
    }

    let receiver_pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);

    // Loop to retry after being woken from Waiting state by a sender
    loop {
        match ipc::endpoint::recv(ep_id, receiver_pid) {
            Ok(msg) => {
                // Copy payload to user buffer via direct access (SUM=1 enabled in sstatus)
                let len = core::cmp::min(msg.payload_len, buf_len);
                if len > 0 && buf_ptr != 0 {
                    unsafe {
                        let dst = buf_ptr as *mut u8;
                        for i in 0..len {
                            core::ptr::write_volatile(dst.add(i), msg.payload[i]);
                        }
                    }
                }
                return Ok(((msg.opcode as usize) << 24) | (msg.sender_pid as usize & 0x00FF_FFFF));
            }
            Err(_) => {
                // Would block -- schedule away
                crate::sched::schedule();
                // When scheduled back, retry recv (message should be queued)
            }
        }
    }
}
