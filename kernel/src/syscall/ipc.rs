use crate::ipc;
use crate::ipc::message::Message;

pub fn sys_ep_create() -> Result<usize, &'static str> {
    let ep_id = ipc::create_endpoint();
    // TODO: create cap for this EP in caller's CNode
    Ok(ep_id)
}

/// sys_send(ep_id: usize, opcode: u16, payload_ptr: usize) -> Result
/// payload: up to 64 bytes at user-space payload_ptr
pub fn sys_send(ep_id: usize, opcode: u16, payload_ptr: usize) -> Result<usize, &'static str> {
    let sender_pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);

    let mut msg = Message::new(sender_pid, opcode as u16);

    // Copy payload from user space.
    // We are running with the user process's satp active, so user virtual
    // addresses are directly accessible.
    if payload_ptr != 0 {
        let len = core::cmp::min(msg.payload.len(), 64);
        unsafe {
            let src = core::slice::from_raw_parts(payload_ptr as *const u8, len);
            msg.payload[..len].copy_from_slice(src);
            msg.payload_len = len;
        }
    }

    ipc::endpoint::send(ep_id, sender_pid, msg)?;
    Ok(0)
}

/// sys_recv(ep_id: usize) -> Result
pub fn sys_recv(ep_id: usize) -> Result<usize, &'static str> {
    let receiver_pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);

    match ipc::endpoint::recv(ep_id, receiver_pid) {
        Ok(msg) => {
            // Return something useful -- the sender pid or opcode
            Ok(msg.sender_pid as usize)
        }
        Err(_) => {
            // Would block -- schedule away
            crate::sched::schedule();
            // When we return, the message should be available
            // For now, return error
            Err("interrupted")
        }
    }
}
