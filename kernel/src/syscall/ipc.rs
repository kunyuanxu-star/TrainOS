use crate::ipc;
use crate::ipc::message::{Message, MAX_PAYLOAD};

pub fn sys_ep_create() -> Result<usize, &'static str> {
    let ep_id = ipc::create_endpoint();
    // TODO: create cap for this EP in caller's CNode
    Ok(ep_id)
}

/// sys_send(ep_id: usize, opcode: u16, payload_ptr: usize, payload_len: usize) -> Result
/// payload: up to payload_len bytes at user-space payload_ptr (max 64)
pub fn sys_send(ep_id: usize, opcode: u16, payload_ptr: usize, payload_len: usize) -> Result<usize, &'static str> {
    let sender_pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);

    let mut msg = Message::new(sender_pid, opcode as u16);

    // Copy payload from user space.
    // We are running with the user process's satp active, so user virtual
    // addresses are directly accessible. sstatus.SUM=1 permits S-mode
    // access to User (U=1) pages.
    if payload_ptr != 0 && payload_len > 0 {
        let len = core::cmp::min(payload_len, MAX_PAYLOAD);
        unsafe {
            let src = core::slice::from_raw_parts(payload_ptr as *const u8, len);
            msg.payload[..len].copy_from_slice(src);
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
                        let dst = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, len);
                        dst.copy_from_slice(&msg.payload[..len]);
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
