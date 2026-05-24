// Socket syscalls — IPC-to-TCP/NET translation
//
// These syscalls provide a BSD socket-like API on top of TrainOS IPC.
// socket() creates an endpoint, bind/connect register with TCP/net service,
// sendto/recvfrom use IPC send/recv.

use crate::ipc::message::Message;

fn current_pid() -> u32 {
    crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0)
}

/// sys_socket(domain, type, protocol) — create a communication endpoint.
/// Returns fd (ep_id).
pub fn sys_socket(_domain: usize, _typ: usize, _proto: usize) -> Result<usize, &'static str> {
    let ep = crate::ipc::create_endpoint();
    Ok(ep)
}

/// sys_bind(fd, addr_ptr, addr_len) — bind socket to address.
/// For TCP: registers with TCP service.
pub fn sys_bind(fd: usize, addr_ptr: usize, _addr_len: usize) -> Result<usize, &'static str> {
    let sender_pid = current_pid();

    // V27.3: Network sandbox check — extract port from sockaddr
    let port = if addr_ptr != 0 {
        unsafe { (addr_ptr as *const u16).read_volatile() }
    } else {
        0
    };
    if !crate::aslr::sandbox_net_check(sender_pid, port, true) {
        return Err("sandbox: bind denied");
    }

    let mut msg = Message::new(sender_pid, 1); // LISTEN
    msg.payload_len = 2;
    msg.payload[0] = (fd >> 8) as u8;
    msg.payload[1] = fd as u8;

    crate::ipc::endpoint::send(3, sender_pid, msg)
        .ok()
        .ok_or("bind send failed")?;

    Ok(0)
}

/// sys_listen(fd, backlog) — listen for connections.
pub fn sys_listen(fd: usize, _backlog: usize) -> Result<usize, &'static str> {
    let sender_pid = current_pid();
    let mut msg = Message::new(sender_pid, 1); // LISTEN opcode to TCP
    msg.payload_len = 2;
    msg.payload[0] = (fd >> 8) as u8;
    msg.payload[1] = fd as u8;

    // Send to TCP service (we'll use a special endpoint or net)
    crate::ipc::endpoint::send(3, sender_pid, msg)
        .ok()
        .ok_or("listen send failed")?;

    Ok(0)
}

/// sys_accept(fd) — accept a connection.
/// Returns new fd for the connection.
pub fn sys_accept(fd: usize) -> Result<usize, &'static str> {
    let sender_pid = current_pid();

    // Wait for incoming connection on this fd
    loop {
        match crate::ipc::endpoint::recv(fd, sender_pid) {
            Ok(_msg) => {
                let new_fd = crate::ipc::create_endpoint();
                return Ok(new_fd);
            }
            Err(_) => {
                crate::sched::schedule();
            }
        }
    }
}

/// sys_connect(fd, addr_ptr, addr_len) — connect to remote socket.
pub fn sys_connect(fd: usize, addr_ptr: usize, _addr_len: usize) -> Result<usize, &'static str> {
    let sender_pid = current_pid();

    // V27.3: Network sandbox check — extract port from sockaddr
    let port = if addr_ptr != 0 {
        unsafe { (addr_ptr as *const u16).read_volatile() }
    } else {
        0
    };
    if !crate::aslr::sandbox_net_check(sender_pid, port, false) {
        return Err("sandbox: connect denied");
    }

    let mut msg = Message::new(sender_pid, 2); // CONNECT opcode to TCP
    msg.payload_len = 2;
    msg.payload[0] = (fd >> 8) as u8;
    msg.payload[1] = fd as u8;

    crate::ipc::endpoint::send(3, sender_pid, msg)
        .ok()
        .ok_or("connect send failed")?;

    // Wait for response
    loop {
        match crate::ipc::endpoint::recv(fd, sender_pid) {
            Ok(_msg) => return Ok(0),
            Err(_) => {
                crate::sched::schedule();
            }
        }
    }
}

/// sys_sendto(fd, buf_ptr, len, flags, addr_ptr, addr_len) — send data.
pub fn sys_sendto(
    fd: usize, buf_ptr: usize, len: usize, _flags: usize, addr_ptr: usize, _addr_len: usize,
) -> Result<usize, &'static str> {
    let sender_pid = current_pid();
    let data_len = core::cmp::min(len, 62); // 64 - 2 (port)

    let mut msg = Message::new(sender_pid, 2); // SEND opcode for net service
    // Extract port from address
    let port = if addr_ptr != 0 {
        unsafe { (addr_ptr as *const u16).read_volatile() }
    } else {
        80u16 // default HTTP port
    };

    msg.payload[0] = (port >> 8) as u8;
    msg.payload[1] = port as u8;
    msg.payload[2] = data_len as u8;

    if buf_ptr != 0 && data_len > 0 {
        unsafe {
            let src = core::slice::from_raw_parts(buf_ptr as *const u8, data_len);
            msg.payload[3..3 + data_len].copy_from_slice(src);
        }
    }
    msg.payload_len = 3 + data_len;

    // Send to NET service (EP 3)
    crate::ipc::endpoint::send(3, sender_pid, msg)
        .ok()
        .ok_or("sendto failed")?;

    Ok(data_len)
}

/// sys_recvfrom(fd, buf_ptr, len, flags, addr_ptr, addr_len_ptr) — receive data.
pub fn sys_recvfrom(
    fd: usize, buf_ptr: usize, len: usize, _flags: usize, _addr_ptr: usize, _addr_len_ptr: usize,
) -> Result<usize, &'static str> {
    let sender_pid = current_pid();

    loop {
        match crate::ipc::endpoint::recv(fd, sender_pid) {
            Ok(msg) => {
                let copy_len = core::cmp::min(msg.payload_len, len);
                if buf_ptr != 0 && copy_len > 0 {
                    unsafe {
                        let dst = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, copy_len);
                        dst.copy_from_slice(&msg.payload[..copy_len]);
                    }
                }
                return Ok(copy_len);
            }
            Err(_) => {
                crate::sched::schedule();
            }
        }
    }
}

// ── V30 Socket syscalls ──────────────────────────────────────────────────────

/// sys_getsockopt(fd, level, optname, optval, optlen) — get socket options.
pub fn sys_getsockopt(fd: usize, _level: usize, optname: usize, optval_ptr: usize, optlen_ptr: usize) -> Result<usize, &'static str> {
    let sender_pid = current_pid();
    let _ = sender_pid;
    if optval_ptr == 0 || optlen_ptr == 0 { return Err("null optval/optlen"); }

    unsafe {
        let optlen = (optlen_ptr as *const u32).read_volatile() as usize;
        match optname {
            1 => { // SO_REUSEADDR
                if optlen >= 4 {
                    (optval_ptr as *mut u32).write_volatile(1);
                    (optlen_ptr as *mut u32).write_volatile(4);
                }
                Ok(0)
            }
            2 => { // SO_TYPE = SOCK_STREAM (1) or SOCK_DGRAM (2)
                if optlen >= 4 {
                    (optval_ptr as *mut u32).write_volatile(1); // SOCK_STREAM
                    (optlen_ptr as *mut u32).write_volatile(4);
                }
                Ok(0)
            }
            3 => { // SO_ERROR
                if optlen >= 4 {
                    (optval_ptr as *mut u32).write_volatile(0); // no error
                    (optlen_ptr as *mut u32).write_volatile(4);
                }
                Ok(0)
            }
            7 => { // SO_SNDBUF
                if optlen >= 4 {
                    (optval_ptr as *mut u32).write_volatile(8192);
                    (optlen_ptr as *mut u32).write_volatile(4);
                }
                Ok(0)
            }
            8 => { // SO_RCVBUF
                if optlen >= 4 {
                    (optval_ptr as *mut u32).write_volatile(8192);
                    (optlen_ptr as *mut u32).write_volatile(4);
                }
                Ok(0)
            }
            _ => Ok(0), // unknown option — return 0
        }
    }
}

/// sys_setsockopt(fd, level, optname, optval, optlen) — set socket options.
pub fn sys_setsockopt(fd: usize, _level: usize, _optname: usize, _optval_ptr: usize, _optlen: usize) -> Result<usize, &'static str> {
    let sender_pid = current_pid();
    let _ = sender_pid;
    Ok(0) // Accept all options silently
}

/// sys_getpeername(fd, addr_ptr, addrlen_ptr) — get peer address.
pub fn sys_getpeername(fd: usize, addr_ptr: usize, _addrlen_ptr: usize) -> Result<usize, &'static str> {
    let sender_pid = current_pid();
    let _ = sender_pid;
    if addr_ptr == 0 { return Err("null addr"); }
    // Return a dummy sockaddr_in (AF_INET, port 0, addr 0)
    unsafe {
        let ptr = addr_ptr as *mut u16;
        ptr.write_volatile(2); // AF_INET
        ptr.add(1).write_volatile(0); // port
        let ip_ptr = addr_ptr as *mut u32;
        ip_ptr.add(1).write_volatile(0); // addr
    }
    Ok(0)
}

/// sys_getsockname(fd, addr_ptr, addrlen_ptr) — get socket name.
pub fn sys_getsockname(fd: usize, addr_ptr: usize, _addrlen_ptr: usize) -> Result<usize, &'static str> {
    let sender_pid = current_pid();
    let _ = sender_pid;
    if addr_ptr == 0 { return Err("null addr"); }
    unsafe {
        let ptr = addr_ptr as *mut u16;
        ptr.write_volatile(2); // AF_INET
        ptr.add(1).write_volatile(0);
        let ip_ptr = addr_ptr as *mut u32;
        ip_ptr.add(1).write_volatile(0);
    }
    Ok(0)
}

/// sys_shutdown(fd, how) — shut down part of a full-duplex connection.
pub fn sys_shutdown(fd: usize, _how: usize) -> Result<usize, &'static str> {
    let sender_pid = current_pid();
    let _ = sender_pid;
    // Acknowledge shutdown — no real teardown needed
    Ok(0)
}
