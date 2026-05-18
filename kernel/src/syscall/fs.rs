// Filesystem syscalls — IPC-to-VFS translation
//
// These syscalls translate POSIX filesystem operations into IPC messages
// to the VFS service (EP 2).

use crate::ipc::message::Message;

fn current_pid() -> u32 {
    crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0)
}

/// Send a VFS request and wait for the response.
fn vfs_request(opcode: u16, path: &str, data: &[u8]) -> Result<Message, &'static str> {
    let sender_pid = current_pid();
    let reply_ep = crate::ipc::create_endpoint();

    let mut msg = Message::new(sender_pid, opcode);

    // Payload format: [reply_ep:2] [path_len:1] [path:path_len] [data_len:1] [data]
    msg.payload[0] = reply_ep as u8;
    msg.payload[1] = (reply_ep >> 8) as u8;
    let plen = path.len().min(31);
    msg.payload[2] = plen as u8;
    for i in 0..plen {
        msg.payload[3 + i] = path.as_bytes()[i];
    }
    let data_off = 3 + plen;
    let dlen = data.len().min(64 - data_off - 1);
    msg.payload[data_off] = dlen as u8;
    for i in 0..dlen {
        msg.payload[data_off + 1 + i] = data[i];
    }
    msg.payload_len = data_off + 1 + dlen;

    crate::ipc::endpoint::send(2, sender_pid, msg)
        .ok()
        .ok_or("vfs send failed")?;

    // Wait for response
    loop {
        match crate::ipc::endpoint::recv(reply_ep, sender_pid) {
            Ok(resp) => return Ok(resp),
            Err(_) => {
                crate::sched::schedule();
            }
        }
    }
}

/// sys_pipe(fds_ptr) — create a pipe.
/// fds_ptr points to [i32; 2]: fds[0]=read end, fds[1]=write end.
/// In the microkernel, pipe = two endpoints connected.
pub fn sys_pipe(fds_ptr: usize) -> Result<usize, &'static str> {
    let read_ep = crate::ipc::create_endpoint();
    let write_ep = crate::ipc::create_endpoint();

    if fds_ptr != 0 {
        unsafe {
            let fds = fds_ptr as *mut u32;
            fds.write_volatile(read_ep as u32);
            fds.add(1).write_volatile(write_ep as u32);
        }
    }

    Ok(0)
}

/// sys_mkdir(path_ptr, mode) — create a directory.
pub fn sys_mkdir(path_ptr: usize, _mode: usize) -> Result<usize, &'static str> {
    let path = read_user_str(path_ptr)?;
    vfs_request(3, &path, b"DIR")?; // WRITE with "DIR" marker creates dir
    Ok(0)
}

/// sys_rmdir(path_ptr) — remove a directory.
pub fn sys_rmdir(path_ptr: usize) -> Result<usize, &'static str> {
    let path = read_user_str(path_ptr)?;
    vfs_request(5, &path, &[])?; // DELETE
    Ok(0)
}

/// sys_unlink(path_ptr) — delete a file.
pub fn sys_unlink(path_ptr: usize) -> Result<usize, &'static str> {
    let path = read_user_str(path_ptr)?;
    vfs_request(5, &path, &[])?; // DELETE
    Ok(0)
}

/// sys_rename(old_ptr, new_ptr) — rename a file.
pub fn sys_rename(old_ptr: usize, new_ptr: usize) -> Result<usize, &'static str> {
    let old_path = read_user_str(old_ptr)?;
    let new_path = read_user_str(new_ptr)?;

    // Read old file content, then write to new path, then delete old
    let old_data = vfs_request(2, &old_path, &[])?;

    let mut combined = [0u8; 64];
    let mut clen = 0;
    for i in 0..new_path.len().min(31) {
        combined[i] = new_path.as_bytes()[i];
    }
    clen = new_path.len().min(31);
    vfs_request(3, &new_path, &combined[..clen])?; // WRITE to new path
    vfs_request(5, &old_path, &[])?; // DELETE old path

    Ok(0)
}

/// sys_getdents64(fd, buf_ptr, buf_len) — get directory entries.
pub fn sys_getdents64(fd: usize, buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if fd != 0 {
        return Err("bad fd");
    }

    // Get directory listing from VFS
    let resp = vfs_request(6, "/", &[])?;

    let copy_len = core::cmp::min(resp.payload_len, buf_len);
    if buf_ptr != 0 && copy_len > 0 {
        unsafe {
            let dst = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, copy_len);
            dst.copy_from_slice(&resp.payload[..copy_len]);
        }
    }

    Ok(copy_len)
}

/// sys_fcntl(fd, cmd, arg) — file descriptor control.
pub fn sys_fcntl(fd: usize, cmd: usize, _arg: usize) -> Result<usize, &'static str> {
    match cmd {
        0 => Ok(fd), // F_DUPFD
        1 => Ok(0),  // F_GETFD
        2 => Ok(0),  // F_SETFD
        3 => Ok(0),  // F_GETFL
        4 => Ok(0),  // F_SETFL
        _ => Err("unsupported fcntl"),
    }
}

/// sys_chdir(path_ptr) — change working directory.
pub fn sys_chdir(_path_ptr: usize) -> Result<usize, &'static str> {
    // Simplified: always succeeds
    Ok(0)
}

/// sys_access(path_ptr, mode) — check file accessibility.
pub fn sys_access(_path_ptr: usize, _mode: usize) -> Result<usize, &'static str> {
    // Simplified: always succeeds (root can access everything)
    Ok(0)
}

/// sys_ioctl(fd, request, arg) — device control.
pub fn sys_ioctl(_fd: usize, _req: usize, _arg: usize) -> Result<usize, &'static str> {
    // Stub: returns success for most ioctls
    Ok(0)
}

/// sys_truncate(path_ptr, length) — truncate a file.
pub fn sys_truncate(path_ptr: usize, _length: usize) -> Result<usize, &'static str> {
    let path = read_user_str(path_ptr)?;
    // Truncate: write empty data to the file
    vfs_request(3, &path, &[])?; // WRITE empty overwrites
    Ok(0)
}

/// Read a null-terminated string from user space.
fn read_user_str(ptr: usize) -> Result<&'static str, &'static str> {
    // Safe: SUM=1 means kernel can access user pages
    if ptr == 0 {
        return Err("null pointer");
    }
    // For simplicity, return a reference to the first 32 bytes
    // A full implementation would copy to kernel buffer
    Ok("") // stub — caller provides real path
}
