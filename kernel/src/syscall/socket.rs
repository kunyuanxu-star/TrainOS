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
pub fn sys_bind(fd: usize, _addr_ptr: usize, _addr_len: usize) -> Result<usize, &'static str> {
    let sender_pid = current_pid();
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
pub fn sys_connect(fd: usize, _addr_ptr: usize, _addr_len: usize) -> Result<usize, &'static str> {
    let sender_pid = current_pid();
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
