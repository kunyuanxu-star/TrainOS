// POSIX-compatible file I/O syscalls with proper fd table and VFS forwarding.
//
// Architecture:
//   Per-process fd table: flat static array (pid, fd) -> path | socket | pipe
//   open/read/write/close/stat/lseek/dup all operate through the fd table.
//   File data is stored in the VFS service (EP 2) by path.
//
// Fd types:
//   0 = FILE (path-based, forwarded to VFS)
//   1 = SOCKET (endpoint-based)
//   2 = PIPE (endpoint pair)

use crate::ipc::message::Message;

const MAX_FDS: usize = 64;
const MAX_PATH: usize = 32;

#[derive(Clone, Copy)]
pub(crate) enum FdType { File, Socket, Pipe, Epoll }

pub(crate) struct FdEntry {
    pub pid: u32,
    pub fd: usize,
    pub fd_type: FdType,
    pub path: [u8; MAX_PATH],
    pub path_len: usize,
    pub offset: usize,
    pub ep: usize,      // for socket/pipe/epoll
    pub used: bool,
}

const EMPTY_FD: FdEntry = FdEntry {
    pid: 0, fd: 0, fd_type: FdType::File,
    path: [0; MAX_PATH], path_len: 0, offset: 0, ep: 0, used: false,
};

static mut FD_TABLE: [FdEntry; MAX_FDS] = [EMPTY_FD; MAX_FDS];

fn current_pid() -> u32 {
    crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0)
}

// ── FD table helpers ─────────────────────────────────────────────────────────

unsafe fn alloc_fd(pid: u32) -> Option<usize> {
    // Start from fd=3 (0=stdin, 1=stdout, 2=stderr)
    let start = 3;
    for fd in start..MAX_FDS {
        // Check if fd is already used by this pid
        let mut used = false;
        for i in 0..MAX_FDS {
            if FD_TABLE[i].used && FD_TABLE[i].pid == pid && FD_TABLE[i].fd == fd {
                used = true; break;
            }
        }
        if !used { return Some(fd); }
    }
    None
}

unsafe fn add_fd(pid: u32, fd: usize, fd_type: FdType, path: &[u8], ep: usize) -> bool {
    for i in 0..MAX_FDS {
        if !FD_TABLE[i].used {
            FD_TABLE[i].used = true;
            FD_TABLE[i].pid = pid;
            FD_TABLE[i].fd = fd;
            FD_TABLE[i].fd_type = fd_type;
            let plen = path.len().min(MAX_PATH);
            FD_TABLE[i].path_len = plen;
            for j in 0..plen { FD_TABLE[i].path[j] = path[j]; }
            FD_TABLE[i].offset = 0;
            FD_TABLE[i].ep = ep;
            return true;
        }
    }
    false
}

unsafe fn find_fd(pid: u32, fd: usize) -> Option<*mut FdEntry> {
    for i in 0..MAX_FDS {
        if FD_TABLE[i].used && FD_TABLE[i].pid == pid && FD_TABLE[i].fd == fd {
            return Some(&mut FD_TABLE[i] as *mut FdEntry);
        }
    }
    None
}

/// Public wrapper for fd lookup, used by V30 filesystem syscalls.
pub unsafe fn find_fd_internal(pid: u32, fd: usize) -> Option<*mut FdEntry> {
    find_fd(pid, fd)
}

unsafe fn remove_fd(pid: u32, fd: usize) {
    for i in 0..MAX_FDS {
        if FD_TABLE[i].used && FD_TABLE[i].pid == pid && FD_TABLE[i].fd == fd {
            FD_TABLE[i].used = false;
            return;
        }
    }
}

// ── Helper: read user-space string ───────────────────────────────────────────

fn read_user_string(ptr: usize, max_len: usize, out: &mut [u8]) -> Result<usize, &'static str> {
    if ptr == 0 { return Err("null str ptr"); }
    let mut len = 0;
    unsafe {
        let src = ptr as *const u8;
        while len < max_len && len < out.len() {
            let c = src.add(len).read_volatile();
            if c == 0 { break; }
            out[len] = c;
            len += 1;
        }
    }
    Ok(len)
}

// ── VFS IPC helper ───────────────────────────────────────────────────────────

fn vfs_send_recv(opcode: u16, path: &[u8], write_data: &[u8]) -> Result<Message, &'static str> {
    let sender_pid = current_pid();
    let reply_ep = crate::ipc::create_endpoint();

    let mut msg = Message::new(sender_pid, opcode);
    msg.payload[0] = reply_ep as u8;
    msg.payload[1] = (reply_ep >> 8) as u8;
    let plen = path.len().min(31);
    msg.payload[2] = plen as u8;
    for i in 0..plen { msg.payload[3 + i] = path[i]; }
    let data_off = 3 + plen;
    let dlen = write_data.len().min(64 - data_off - 1);
    msg.payload[data_off] = dlen as u8;
    for i in 0..dlen { msg.payload[data_off + 1 + i] = write_data[i]; }
    msg.payload_len = data_off + 1 + dlen;

    // Send to VFS (EP 2)
    crate::ipc::endpoint::send(2, sender_pid, msg).ok().ok_or("vfs send failed")?;

    // Wait for response
    loop {
        match crate::ipc::endpoint::recv(reply_ep, sender_pid) {
            Ok(resp) => return Ok(resp),
            Err(_) => { crate::sched::schedule(); }
        }
    }
}

fn vfs_read(path: &[u8]) -> Result<Message, &'static str> {
    vfs_send_recv(2, path, &[])
}

fn vfs_write(path: &[u8], data: &[u8]) -> Result<Message, &'static str> {
    vfs_send_recv(3, path, data)
}

fn vfs_list(path: &[u8]) -> Result<Message, &'static str> {
    vfs_send_recv(6, path, &[])
}

fn vfs_delete(path: &[u8]) -> Result<Message, &'static str> {
    vfs_send_recv(5, path, &[])
}

// ── Helper: get current process page table root ──────────────────────────────

fn current_root_pt() -> Option<usize> {
    let pid = current_pid();
    let procs = crate::proc::PROCESSES.lock();
    for proc in procs.iter() {
        if proc.pid == pid {
            return Some(proc.page_table_root);
        }
    }
    None
}

// ── POSIX syscalls ───────────────────────────────────────────────────────────

/// sys_open(path_ptr, flags, mode) — open a file by path.
/// Returns fd on success.
pub fn sys_open(path_ptr: usize, flags: usize, _mode: usize) -> Result<usize, &'static str> {
    let pid = current_pid();
    let mut path_buf = [0u8; MAX_PATH];
    let plen = read_user_string(path_ptr, MAX_PATH, &mut path_buf)?;
    let path = &path_buf[..plen];

    // V27.3: Sandbox check — open requires read permission (or write if O_CREAT)
    let wants_write = (flags & 0o100) != 0 || (flags & 0o2) != 0;
    if !crate::aslr::sandbox_check(pid, path, wants_write) {
        return Err("sandbox: open denied");
    }

    // If flags has O_CREAT, ensure the file exists by writing empty if it doesn't
    if flags & 0o100 != 0 {
        let _ = vfs_write(path, &[]); // touch the file
    }

    // Check the file exists by reading it (or getting its stat)
    let _resp = vfs_read(path)?;

    let fd = unsafe { alloc_fd(pid).ok_or("no free fd")? };
    unsafe { add_fd(pid, fd, FdType::File, path, 0); }

    Ok(fd)
}

/// sys_read(fd, buf_ptr, count) — read from a file descriptor.
pub fn sys_read(fd: usize, buf_ptr: usize, count: usize) -> Result<usize, &'static str> {
    let pid = current_pid();

    // V21.9: Validate user buffer bounds
    if buf_ptr != 0 && count > 0 {
        if let Some(root_pt) = current_root_pt() {
            if !crate::mem::sv39::is_user_range_valid(root_pt, buf_ptr, count) {
                return Err("buffer out of bounds");
            }
        }
    }

    if fd <= 2 {
        // stdin (0) — read a char from SBI console
        if fd == 0 {
            let c: usize;
            unsafe { core::arch::asm!("ecall", in("a7") 2usize, lateout("a0") c); }
            if buf_ptr != 0 && count > 0 {
                unsafe { (buf_ptr as *mut u8).write_volatile(c as u8); }
            }
            return Ok(1);
        }
        // stdout/stderr — not readable
        return Err("bad fd for read");
    }

    let entry = unsafe { find_fd(pid, fd).ok_or("bad fd")? };
    let path = unsafe {
        let e = &*entry;
        if matches!(e.fd_type, FdType::Socket) || matches!(e.fd_type, FdType::Pipe) {
            // For sockets/pipes, read from the endpoint directly
            let sender_pid = current_pid();
            loop {
                match crate::ipc::endpoint::recv(e.ep, sender_pid) {
                    Ok(msg) => {
                        let copy_len = core::cmp::min(msg.payload_len, count.min(64));
                        if buf_ptr != 0 && copy_len > 0 {
                            unsafe {
                                let dst = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, copy_len);
                                dst.copy_from_slice(&msg.payload[..copy_len]);
                            }
                        }
                        return Ok(copy_len);
                    }
                    Err(_) => { crate::sched::schedule(); }
                }
            }
        }
        core::slice::from_raw_parts(e.path.as_ptr(), e.path_len)
    };

    // V27.3: Sandbox check for file reads
    if !crate::aslr::sandbox_check(pid, path, false) {
        return Err("sandbox: read denied");
    }

    // File read via VFS
    let resp = vfs_read(path)?;

    // Skip "ENOENT" response
    if resp.payload_len >= 6 && &resp.payload[..6] == b"ENOENT" {
        return Ok(0);
    }

    let offset = unsafe { (*entry).offset };
    let available = if resp.payload_len > offset { resp.payload_len - offset } else { 0 };
    let copy_len = core::cmp::min(available, count);

    if buf_ptr != 0 && copy_len > 0 {
        unsafe {
            let dst = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, copy_len);
            dst.copy_from_slice(&resp.payload[offset..offset + copy_len]);
        }
    }

    unsafe { (*entry).offset += copy_len; }
    Ok(copy_len)
}

/// sys_write(fd, buf_ptr, count) — write to a file descriptor.
pub fn sys_write(fd: usize, buf_ptr: usize, count: usize) -> Result<usize, &'static str> {
    let pid = current_pid();

    // V21.9: Validate user buffer bounds
    if buf_ptr != 0 && count > 0 {
        if let Some(root_pt) = current_root_pt() {
            if !crate::mem::sv39::is_user_range_valid(root_pt, buf_ptr, count) {
                return Err("buffer out of bounds");
            }
        }
    }

    if fd <= 2 {
        // stdout (1) / stderr (2) — write to SBI console
        if buf_ptr != 0 && count > 0 {
            unsafe {
                let src = core::slice::from_raw_parts(buf_ptr as *const u8, count);
                for &c in src {
                    core::arch::asm!("ecall", in("a7") 1usize, in("a0") c as usize);
                }
            }
        }
        return Ok(count);
    }

    let entry = unsafe { find_fd(pid, fd).ok_or("bad fd")? };

    unsafe {
        let e = &mut *entry;
        if matches!(e.fd_type, FdType::Socket) || matches!(e.fd_type, FdType::Pipe) {
            // For sockets/pipes, send to the endpoint
            let sender_pid = current_pid();
            let data_len = core::cmp::min(count, 62);
            let mut msg = Message::new(sender_pid, 0);
            if buf_ptr != 0 && data_len > 0 {
                let src = core::slice::from_raw_parts(buf_ptr as *const u8, data_len);
                msg.payload[..data_len].copy_from_slice(src);
            }
            msg.payload_len = data_len;
            crate::ipc::endpoint::send(e.ep, sender_pid, msg).ok().ok_or("send failed")?;
            return Ok(data_len);
        }
    }

    // File write via VFS
    let path = unsafe {
        let e = &*entry;
        core::slice::from_raw_parts(e.path.as_ptr(), e.path_len)
    };

    // V27.3: Sandbox check for file writes
    if !crate::aslr::sandbox_check(pid, path, true) {
        return Err("sandbox: write denied");
    }

    let write_len = core::cmp::min(count, 60); // leave room for IPC headers
    let data = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, write_len) };

    vfs_write(path, data)?;
    Ok(write_len)
}

/// sys_close(fd) — close a file descriptor.
pub fn sys_close(fd: usize) -> Result<usize, &'static str> {
    let pid = current_pid();
    if fd <= 2 { return Ok(0); } // can't close stdin/stdout/stderr
    unsafe { remove_fd(pid, fd); }
    Ok(0)
}

/// sys_stat(fd, buf_ptr) — get file status.
pub fn sys_stat(fd: usize, _buf_ptr: usize) -> Result<usize, &'static str> {
    let pid = current_pid();
    if fd <= 2 { return Ok(0); } // console fds

    let entry = unsafe { find_fd(pid, fd).ok_or("bad fd")? };
    let path = unsafe { core::slice::from_raw_parts((*entry).path.as_ptr(), (*entry).path_len) };

    let resp = vfs_read(path)?;
    Ok(resp.payload_len)
}

/// sys_lseek(fd, offset, whence) — reposition read/write offset.
pub fn sys_lseek(fd: usize, offset: isize, whence: usize) -> Result<usize, &'static str> {
    let pid = current_pid();
    let entry = unsafe { find_fd(pid, fd).ok_or("bad fd")? };
    unsafe {
        match whence {
            0 => (*entry).offset = offset as usize,       // SEEK_SET
            1 => (*entry).offset = ((*entry).offset as isize + offset) as usize, // SEEK_CUR
            2 => { /* SEEK_END — need file size, stub */ }
            _ => return Err("bad whence"),
        }
    }
    Ok(unsafe { (*entry).offset })
}

/// sys_dup(fd) — duplicate a file descriptor.
pub fn sys_dup(fd: usize) -> Result<usize, &'static str> {
    let pid = current_pid();
    let entry = unsafe { find_fd(pid, fd).ok_or("bad fd")? };

    let new_fd = unsafe { alloc_fd(pid).ok_or("no free fd")? };
    unsafe {
        let e = &*entry;
        add_fd(pid, new_fd, e.fd_type, &[], e.ep);
        // Copy the path
        if let Some(new_entry) = find_fd(pid, new_fd) {
            let src = core::slice::from_raw_parts(e.path.as_ptr(), e.path_len);
            let dst = core::slice::from_raw_parts_mut((*new_entry).path.as_mut_ptr(), MAX_PATH);
            dst[..e.path_len].copy_from_slice(src);
            (*new_entry).path_len = e.path_len;
        }
    }

    Ok(new_fd)
}

/// sys_getcwd(buf_ptr, size) — get current working directory.
pub fn sys_getcwd(buf_ptr: usize, size: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 || size == 0 { return Err("invalid args"); }
    let cwd = b"/";
    let len = cwd.len().min(size);
    unsafe {
        let dst = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, len);
        dst.copy_from_slice(&cwd[..len]);
    }
    Ok(0)
}
