/// POSIX compatibility syscalls.
/// These translate POSIX calls into IPC messages to the FS service (EP 2).
use crate::ipc::message::{Message, MAX_PAYLOAD};

/// Open a file. Returns fd on success.
pub fn sys_open(_path_ptr: usize, _flags: usize, _mode: usize) -> Result<usize, &'static str> {
    // For now, all "files" go to FS service (EP 2)
    // Return fd = 0 always (single file for V2.3)
    Ok(0)
}

/// Read from fd. Returns bytes read.
pub fn sys_read(fd: usize, buf_ptr: usize, count: usize) -> Result<usize, &'static str> {
    if fd != 0 {
        return Err("bad fd");
    }

    let sender_pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    let reply_ep = crate::ipc::create_endpoint();

    // Create a message to FS: opcode 2 = READ
    // Payload: [reply_ep(2 bytes)]
    let mut msg = Message::new(sender_pid, 2);
    msg.payload[0] = reply_ep as u8;
    msg.payload[1] = (reply_ep >> 8) as u8;
    msg.payload_len = 2;

    crate::ipc::endpoint::send(2, sender_pid, msg)
        .ok()
        .ok_or("send failed")?;

    // Retry loop: recv blocks the thread (Waiting), and we retry after being woken
    loop {
        match crate::ipc::endpoint::recv(reply_ep, sender_pid) {
            Ok(resp) => {
                // Copy response to user buffer via direct access (SUM=1 in sstatus)
                let len = core::cmp::min(resp.payload_len, count);
                if buf_ptr != 0 && len > 0 {
                    unsafe {
                        let dst = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, len);
                        dst.copy_from_slice(&resp.payload[..len]);
                    }
                }
                return Ok(len);
            }
            Err(_) => {
                // Would block -- schedule away; when woken, retry recv
                crate::sched::schedule();
            }
        }
    }
}

/// Write to fd. Returns bytes written.
pub fn sys_write(fd: usize, buf_ptr: usize, count: usize) -> Result<usize, &'static str> {
    if fd != 0 {
        return Err("bad fd");
    }

    let len = core::cmp::min(count, MAX_PAYLOAD - 3);
    let sender_pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    let reply_ep = crate::ipc::create_endpoint();

    let mut msg = Message::new(sender_pid, 3); // opcode 3 = WRITE

    // Payload: [reply_ep(2 bytes), data_len(1 byte), data(...)]
    msg.payload[0] = reply_ep as u8;
    msg.payload[1] = (reply_ep >> 8) as u8;
    msg.payload[2] = len as u8;
    if buf_ptr != 0 && len > 0 {
        unsafe {
            let src = core::slice::from_raw_parts(buf_ptr as *const u8, len);
            msg.payload[3..3 + len].copy_from_slice(src);
        }
    }
    msg.payload_len = 3 + len;

    crate::ipc::endpoint::send(2, sender_pid, msg)
        .ok()
        .ok_or("send failed")?;

    loop {
        match crate::ipc::endpoint::recv(reply_ep, sender_pid) {
            Ok(_) => return Ok(len),
            Err(_) => {
                // Would block -- schedule away; retry when woken by FS reply
                crate::sched::schedule();
            }
        }
    }
}

/// Close a file descriptor.
pub fn sys_close(_fd: usize) -> Result<usize, &'static str> {
    Ok(0)
}
