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

/// Read a null-terminated string from user space into the given buffer.
/// Returns the number of bytes read (excluding null terminator).
fn read_user_path(ptr: usize, out: &mut [u8]) -> Result<usize, &'static str> {
    if ptr == 0 { return Err("null pointer"); }
    let max = out.len().min(63);
    unsafe {
        let src = ptr as *const u8;
        let mut len = 0;
        while len < max {
            let c = src.add(len).read_volatile();
            if c == 0 { break; }
            out[len] = c;
            len += 1;
        }
        Ok(len)
    }
}

/// Send a VFS request and wait for the response.
/// path is a byte slice (not necessarily null-terminated).
fn vfs_request(opcode: u16, path: &[u8], data: &[u8]) -> Result<Message, &'static str> {
    let sender_pid = current_pid();
    let reply_ep = crate::ipc::create_endpoint();

    let mut msg = Message::new(sender_pid, opcode);

    // Payload: [reply_ep:2] [path_len:1] [path:path_len] [data_len:1] [data...]
    msg.payload[0] = reply_ep as u8;
    msg.payload[1] = (reply_ep >> 8) as u8;
    let plen = path.len().min(31);
    msg.payload[2] = plen as u8;
    for i in 0..plen { msg.payload[3 + i] = path[i]; }
    let data_off = 3 + plen;
    let dlen = data.len().min(64 - data_off - 1);
    msg.payload[data_off] = dlen as u8;
    for i in 0..dlen { msg.payload[data_off + 1 + i] = data[i]; }
    msg.payload_len = data_off + 1 + dlen;

    crate::ipc::endpoint::send(2, sender_pid, msg).ok().ok_or("vfs send failed")?;

    loop {
        match crate::ipc::endpoint::recv(reply_ep, sender_pid) {
            Ok(resp) => return Ok(resp),
            Err(_) => { crate::sched::schedule(); }
        }
    }
}

/// sys_pipe(fds_ptr) — create a pipe. fds[0]=read, fds[1]=write.
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
    let mut path = [0u8; 32];
    let plen = read_user_path(path_ptr, &mut path)?;
    vfs_request(3, &path[..plen], b"DIR")?; // WRITE with marker
    Ok(0)
}

/// sys_rmdir(path_ptr) — remove a directory.
pub fn sys_rmdir(path_ptr: usize) -> Result<usize, &'static str> {
    let mut path = [0u8; 32];
    let plen = read_user_path(path_ptr, &mut path)?;
    vfs_request(5, &path[..plen], &[])?;
    Ok(0)
}

/// sys_unlink(path_ptr) — delete a file.
pub fn sys_unlink(path_ptr: usize) -> Result<usize, &'static str> {
    let mut path = [0u8; 32];
    let plen = read_user_path(path_ptr, &mut path)?;
    vfs_request(5, &path[..plen], &[])?;
    Ok(0)
}

/// sys_rename(old_ptr, new_ptr) — rename a file.
pub fn sys_rename(old_ptr: usize, new_ptr: usize) -> Result<usize, &'static str> {
    let mut old_path = [0u8; 32];
    let mut new_path = [0u8; 32];
    let olen = read_user_path(old_ptr, &mut old_path)?;
    let nlen = read_user_path(new_ptr, &mut new_path)?;

    // Read old content, write to new path, delete old
    let old_data = vfs_request(2, &old_path[..olen], &[])?;
    let dlen = old_data.payload_len.min(60);
    vfs_request(3, &new_path[..nlen], &old_data.payload[..dlen])?;
    vfs_request(5, &old_path[..olen], &[])?;

    Ok(0)
}

/// sys_getdents64(fd, buf_ptr, buf_len) — get directory entries.
pub fn sys_getdents64(fd: usize, buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if fd > 2 {
        // File fd — check if it's a valid fd
        let pid = current_pid();
        // For simplicity, list root directory
        let resp = vfs_request(6, b"/", &[])?;
        let copy_len = core::cmp::min(resp.payload_len, buf_len);
        if buf_ptr != 0 && copy_len > 0 {
            unsafe {
                let dst = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, copy_len);
                dst.copy_from_slice(&resp.payload[..copy_len]);
            }
        }
        return Ok(copy_len);
    }

    // List root directory via VFS
    let resp = vfs_request(6, b"/", &[])?;
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
        0 => Ok(fd),          // F_DUPFD — return same fd
        1 | 2 => Ok(0),       // F_GETFD / F_SETFD
        3 | 4 => Ok(0),       // F_GETFL / F_SETFL
        _ => Err("unsupported fcntl cmd"),
    }
}

/// sys_chdir(path_ptr) — change working directory.
pub fn sys_chdir(_path_ptr: usize) -> Result<usize, &'static str> {
    Ok(0) // single-directory filesystem for now
}

/// sys_access(path_ptr, mode) — check file accessibility.
pub fn sys_access(path_ptr: usize, _mode: usize) -> Result<usize, &'static str> {
    let mut path = [0u8; 32];
    let plen = read_user_path(path_ptr, &mut path)?;
    // Try to read the file to check existence
    match vfs_request(2, &path[..plen], &[]) {
        Ok(_) => Ok(0),
        Err(_) => Err("ENOENT"),
    }
}

/// sys_ioctl(fd, request, arg) — device control.
pub fn sys_ioctl(_fd: usize, _req: usize, _arg: usize) -> Result<usize, &'static str> {
    Ok(0)
}

/// sys_truncate(path_ptr, length) — truncate a file.
pub fn sys_truncate(path_ptr: usize, _length: usize) -> Result<usize, &'static str> {
    let mut path = [0u8; 32];
    let plen = read_user_path(path_ptr, &mut path)?;
    vfs_request(3, &path[..plen], &[])?; // WRITE empty = truncate
    Ok(0)
}
