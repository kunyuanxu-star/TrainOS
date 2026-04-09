//! IPC system call handlers

use crate::ipc::{endpoint, channel, message::MAX_MESSAGE_SIZE, Pid, PortId};
use crate::process::get_current_pid;

/// Syscall numbers for IPC (custom TrainOS numbers)
pub const ENDPOINT_CREATE: usize = 1000;
pub const ENDPOINT_DELETE: usize = 1001;
pub const SEND: usize = 1002;
pub const RECV: usize = 1003;
pub const CALL: usize = 1004;

/// sys_endpoint_create - Create a new endpoint
/// Returns port_id on success, -1 on error
pub fn sys_endpoint_create() -> isize {
    let pid = get_current_pid();

    match endpoint::create_endpoint(pid) {
        Some((port, _entry)) => port as isize,
        None => -1,
    }
}

/// sys_endpoint_delete - Delete an endpoint
/// a0 = port_id
pub fn sys_endpoint_delete(port: usize) -> isize {
    let pid = get_current_pid();
    if endpoint::delete_endpoint(port as PortId, pid) {
        0
    } else {
        -1
    }
}

/// sys_send - Send a message
/// a0 = target_pid, a1 = port, a2 = data ptr, a3 = size
pub fn sys_send(target_pid: usize, port: usize, data_ptr: usize, size: usize) -> isize {
    if data_ptr == 0 || size == 0 {
        return -1;
    }

    let safe_size = size.min(MAX_MESSAGE_SIZE - 16);
    let data = unsafe {
        core::slice::from_raw_parts(data_ptr as *const u8, safe_size)
    };

    channel::send(target_pid as Pid, port as PortId, data)
}

/// sys_recv - Receive a message
/// a0 = port, a1 = buffer ptr, a2 = buffer size
pub fn sys_recv(port: usize, buf_ptr: usize, buf_size: usize) -> isize {
    if buf_ptr == 0 || buf_size == 0 {
        return -1;
    }

    let mut buf = unsafe {
        core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_size)
    };

    channel::recv(port as PortId, &mut buf)
}

/// sys_call - Synchronous RPC call
/// a0 = target_pid, a1 = port, a2 = data ptr, a3 = size, a4 = reply ptr, a5 = reply size
pub fn sys_call(target_pid: usize, port: usize, data_ptr: usize, size: usize,
                reply_ptr: usize, reply_size: usize) -> isize {
    // First send the message
    let send_result = sys_send(target_pid, port, data_ptr, size);
    if send_result < 0 {
        return send_result;
    }

    // Create ephemeral reply port
    let reply_port = sys_endpoint_create();
    if reply_port < 0 {
        return -1;
    }

    // Wait for reply
    let recv_result = sys_recv(reply_port as usize, reply_ptr, reply_size);

    // Clean up reply endpoint
    let _ = sys_endpoint_delete(reply_port as usize);

    recv_result
}