// V26: Distributed IPC and cluster coordination
//
// Features:
//   26.1 Node discovery protocol (ping/pong heartbeat)
//        Remote message passing via net service forwarding
//        Distributed capability passing
//   26.2 Remote memory pooling (page allocation, migration)
//   26.3 Cluster coordination (global PID namespace, cross-node proclist)

pub mod memory;
pub mod protocol;

use crate::ipc::endpoint;
use crate::ipc::message::Message;
use crate::proc::process::ProcessState;
use protocol::*;
use spin::Mutex;

// ── Constants ───────────────────────────────────────────────────────────────────

const MAX_REMOTE_NODES: usize = 8;
const MAX_REMOTE_EPS: usize = 16;
const MAX_RCV_BUF_MSGS: usize = 32;
const HEARTBEAT_INTERVAL: usize = 500;

/// Unique cluster node ID (set at boot, typically 0 for first node).
pub static mut CLUSTER_NODE_ID: u8 = 0;

/// Well-known endpoint base. Node N receives on EP = DIST_EP_BASE + N.
pub const DIST_EP_BASE: usize = 200;

// ── Node descriptor ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct RemoteNode {
    node_id: u32,
    ip_addr: [u8; 16],
    port: u16,
    online: bool,
    last_seen_tick: usize,
    failed_pings: u8,
    registered: bool,
}

const EMPTY_NODE: RemoteNode = RemoteNode {
    node_id: 0, ip_addr: [0; 16], port: 0, online: false,
    last_seen_tick: 0, failed_pings: 0, registered: false,
};

#[derive(Clone, Copy)]
struct RemoteEndpoint {
    ep_id: usize,
    remote_node: u32,
    remote_ep: usize,
    registered: bool,
}

// ── Incoming message buffer ─────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct RcvBufEntry {
    src_node: u8,
    src_ep: u16,
    payload: [u8; 64],
    payload_len: usize,
    valid: bool,
}

// ── Global state ────────────────────────────────────────────────────────────────

static REMOTE_NODES: Mutex<[RemoteNode; MAX_REMOTE_NODES]> =
    Mutex::new([EMPTY_NODE; MAX_REMOTE_NODES]);
static REMOTE_NODE_COUNT: Mutex<usize> = Mutex::new(0);

static REMOTE_EPS: Mutex<[RemoteEndpoint; MAX_REMOTE_EPS]> = Mutex::new([
    RemoteEndpoint { ep_id: 0, remote_node: 0, remote_ep: 0, registered: false };
    MAX_REMOTE_EPS
]);
static REMOTE_EP_COUNT: Mutex<usize> = Mutex::new(0);

static RCV_BUF: Mutex<[RcvBufEntry; MAX_RCV_BUF_MSGS]> = Mutex::new([
    RcvBufEntry { src_node: 0, src_ep: 0, payload: [0; 64], payload_len: 0, valid: false };
    MAX_RCV_BUF_MSGS
]);
static RCV_BUF_COUNT: Mutex<usize> = Mutex::new(0);

static mut LAST_HEARTBEAT_TICK: usize = 0;

// ── Initialization ──────────────────────────────────────────────────────────────

pub fn init(cluster_node_id: u8) {
    unsafe { CLUSTER_NODE_ID = cluster_node_id; }
    memory::init();
    crate::println!("DIST: distributed IPC initialized (cluster node {})", cluster_node_id);
}

// ── 26.1 Node Discovery Protocol ────────────────────────────────────────────────

pub fn node_probe(node_id: u32) -> bool {
    let mut ping_buf = [0u8; 64];
    if write_ping(&mut ping_buf, node_id).is_none() {
        return false;
    }

    let target_ep = DIST_EP_BASE + node_id as usize;
    let sender_pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).unwrap_or(0);

    let mut msg = Message::new(sender_pid, 0xD1);
    msg.payload[..6].copy_from_slice(&ping_buf[..6]);
    msg.payload_len = 6;
    endpoint::send(target_ep, sender_pid, msg).ok();

    let deadline = unsafe { crate::trap::TICK_COUNT.wrapping_add(1000) };
    let mut waited = 0usize;
    while unsafe { crate::trap::TICK_COUNT } < deadline {
        if check_for_pong(node_id) {
            mark_node_online(node_id, unsafe { crate::trap::TICK_COUNT });
            return true;
        }
        crate::sched::schedule();
        waited += 1;
        if waited > 200 { break; }
    }
    false
}

fn check_for_pong(node_id: u32) -> bool {
    let buf = RCV_BUF.lock();
    let count = *RCV_BUF_COUNT.lock();
    for i in 0..count {
        if !buf[i].valid { continue; }
        if let Some(pong_node) = parse_pong(&buf[i].payload[..buf[i].payload_len]) {
            if pong_node == node_id { return true; }
        }
    }
    false
}

fn mark_node_online(node_id: u32, tick: usize) {
    let mut nodes = REMOTE_NODES.lock();
    let count = *REMOTE_NODE_COUNT.lock();
    for i in 0..count {
        if nodes[i].node_id == node_id {
            nodes[i].online = true;
            nodes[i].last_seen_tick = tick;
            nodes[i].failed_pings = 0;
            return;
        }
    }
}

pub fn mark_node_offline(node_id: u32) {
    let mut nodes = REMOTE_NODES.lock();
    let count = *REMOTE_NODE_COUNT.lock();
    for i in 0..count {
        if nodes[i].node_id == node_id { nodes[i].online = false; return; }
    }
}

pub fn heartbeat_tick() {
    let current_tick: usize;
    unsafe {
        current_tick = crate::trap::TICK_COUNT;
        if current_tick.wrapping_sub(LAST_HEARTBEAT_TICK) < HEARTBEAT_INTERVAL { return; }
        LAST_HEARTBEAT_TICK = current_tick;
    }

    let count = *REMOTE_NODE_COUNT.lock();
    if count == 0 { return; }

    let mut nodes = REMOTE_NODES.lock();
    for i in 0..count {
        if !nodes[i].registered { continue; }
        if nodes[i].online && current_tick.wrapping_sub(nodes[i].last_seen_tick) < HEARTBEAT_INTERVAL * 3 {
            continue;
        }

        let mut ping_buf = [0u8; 64];
        if write_ping(&mut ping_buf, nodes[i].node_id).is_none() { continue; }

        let target_ep = DIST_EP_BASE + nodes[i].node_id as usize;
        let mut msg = Message::new(0, 0xD1);
        msg.payload[..6].copy_from_slice(&ping_buf[..6]);
        msg.payload_len = 6;
        endpoint::send(target_ep, 0, msg).ok();

        if !nodes[i].online {
            nodes[i].failed_pings = nodes[i].failed_pings.saturating_add(1);
            if nodes[i].failed_pings >= 3 {
                crate::println!("DIST: node {} offline ({} failed)", nodes[i].node_id, nodes[i].failed_pings);
            }
        }
    }
}

fn handle_incoming_ping(src_node: u32) {
    let mut pong_buf = [0u8; 64];
    let plen = write_pong(&mut pong_buf, unsafe { CLUSTER_NODE_ID as u32 }).unwrap_or(0);
    if plen == 0 { return; }

    let mut msg = Message::new(0, 0xD1);
    msg.payload[..plen].copy_from_slice(&pong_buf[..plen]);
    msg.payload_len = plen;
    endpoint::send(DIST_EP_BASE + src_node as usize, 0, msg).ok();
}

fn handle_incoming_pong(src_node: u32) {
    mark_node_online(src_node, unsafe { crate::trap::TICK_COUNT });
}

// ── 26.1 Remote Message Passing ─────────────────────────────────────────────────

pub fn remote_send(node_id: u32, remote_ep: usize, data: &[u8]) -> Result<(), &'static str> {
    let src_node = unsafe { CLUSTER_NODE_ID };

    let mut wire_buf = [0u8; 72];
    let written = write_data(&mut wire_buf, src_node, remote_ep as u16, data)
        .ok_or("packet too large")?;

    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).unwrap_or(0);

    let is_virtual = {
        let nodes = REMOTE_NODES.lock();
        let count = *REMOTE_NODE_COUNT.lock();
        let mut virt = true;
        for i in 0..count {
            if nodes[i].node_id == node_id && nodes[i].port != 0 { virt = false; }
        }
        virt
    };

    if is_virtual {
        let target_ep = DIST_EP_BASE + node_id as usize;
        let mut msg = Message::new(pid, 0xD1);
        msg.payload[..written].copy_from_slice(&wire_buf[..written]);
        msg.payload_len = written;
        endpoint::send(target_ep, pid, msg)?;
    } else {
        let nodes = REMOTE_NODES.lock();
        let count = *REMOTE_NODE_COUNT.lock();
        let mut found = false;
        for i in 0..count {
            if nodes[i].node_id == node_id {
                let port = nodes[i].port;
                let data_len = written.min(62);
                let mut msg = Message::new(pid, 2);
                msg.payload[0] = (port >> 8) as u8;
                msg.payload[1] = port as u8;
                msg.payload[2] = data_len as u8;
                msg.payload[3..3 + data_len].copy_from_slice(&wire_buf[..data_len]);
                msg.payload_len = 3 + data_len;
                endpoint::send(3, pid, msg).ok().ok_or("net send failed")?;
                found = true;
                break;
            }
        }
        if !found { return Err("remote node not found"); }
    }
    Ok(())
}

fn queue_incoming_message(src_node: u8, src_ep: u16, payload: &[u8]) {
    let plen = payload.len().min(64);
    let mut buf = RCV_BUF.lock();
    let mut count = *RCV_BUF_COUNT.lock();

    if count >= MAX_RCV_BUF_MSGS {
        for i in 1..MAX_RCV_BUF_MSGS { buf[i - 1] = buf[i]; }
        count = MAX_RCV_BUF_MSGS - 1;
    }

    let mut p = [0u8; 64];
    p[..plen].copy_from_slice(payload);
    buf[count] = RcvBufEntry { src_node, src_ep, payload: p, payload_len: plen, valid: true };
    *RCV_BUF_COUNT.lock() = count + 1;
}

pub fn remote_recv() -> Result<(u8, u16, [u8; 64], usize), &'static str> {
    loop {
        let mut buf = RCV_BUF.lock();
        let count = *RCV_BUF_COUNT.lock();
        for i in 0..count {
            if buf[i].valid {
                let src_node = buf[i].src_node;
                let src_ep = buf[i].src_ep;
                let payload = buf[i].payload;
                let payload_len = buf[i].payload_len;
                buf[i].valid = false;
                let mut idx = 0;
                for j in 0..count {
                    if buf[j].valid {
                        if idx != j { buf[idx] = buf[j]; buf[j].valid = false; }
                        idx += 1;
                    }
                }
                *RCV_BUF_COUNT.lock() = idx;
                drop(buf);
                return Ok((src_node, src_ep, payload, payload_len));
            }
        }
        drop(buf);
        crate::sched::schedule();
    }
}

// ── 26.1 Distributed Capability Passing ─────────────────────────────────────────

pub fn remote_mint(node_id: u32, local_cap_slot: u32, remote_cnode: u32) -> Result<(), &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).ok_or("no proc")?;

    let (cap_type, rights) = {
        let procs = crate::proc::PROCESSES.lock();
        let caller = procs.iter().find(|p| p.pid == pid).ok_or("proc not found")?;
        let cnode_id = caller.cnode_id;
        drop(procs);
        let res = crate::cap::ops::get_resource(cnode_id).ok_or("cnode not found")?;
        if let crate::cap::types::ResourceData::CNode { ref slots } = &res.data {
            let s = slots.lock();
            let slot = s.get(local_cap_slot as usize).ok_or("slot not found")?;
            if slot.cap_type == crate::cap::types::CapType::Null {
                return Err("null cap");
            }
            (slot.cap_type as u8, slot.rights)
        } else {
            return Err("not a cnode");
        }
    };

    let mut res_data = [0u8; 64];
    res_data[..4].copy_from_slice(&local_cap_slot.to_le_bytes());
    res_data[4..8].copy_from_slice(&remote_cnode.to_le_bytes());

    let mut wire_buf = [0u8; 80];
    let written = write_cap(&mut wire_buf, unsafe { CLUSTER_NODE_ID },
        cap_type, rights, local_cap_slot, &res_data).ok_or("cap serialize failed")?;

    let mut msg = Message::new(pid, 0xD1);
    msg.payload[..written].copy_from_slice(&wire_buf[..written]);
    msg.payload_len = written;
    endpoint::send(DIST_EP_BASE + node_id as usize, pid, msg)?;

    crate::println!("DIST: remote_mint node={} slot={} cap_type={} rights={}",
        node_id, local_cap_slot, cap_type, rights);
    Ok(())
}

pub fn cap_receive(cnode_id: usize, buf: &[u8]) -> Result<(), &'static str> {
    let (src_node, cap_type, rights, slot, _res_data) = parse_cap(buf).ok_or("invalid cap packet")?;
    if cap_type == 0 { return Err("null cap type"); }

    let ktype = match cap_type {
        1 => crate::cap::types::CapType::Mem,
        2 => crate::cap::types::CapType::EP,
        3 => crate::cap::types::CapType::Proc,
        4 => crate::cap::types::CapType::CNode,
        _ => return Err("unknown cap type"),
    };

    let resource_id = crate::cap::ops::alloc_resource(ktype, crate::cap::types::ResourceData::Null);
    let new_slot = crate::cap::types::Slot { cap_type: ktype, rights, resource_id };

    let res = crate::cap::ops::get_resource(cnode_id).ok_or("cnode not found")?;
    if let crate::cap::types::ResourceData::CNode { ref slots } = &res.data {
        let mut s = slots.lock();
        let idx = slot as usize;
        while s.len() <= idx { s.push(crate::cap::types::Slot::null()); }
        s[idx] = new_slot;
    }
    crate::println!("DIST: cap_receive from node={} type={} rights={}", src_node, cap_type, rights);
    Ok(())
}

pub fn remote_copy(node_id: u32, src_slot: u32, remote_cnode: u32) -> Result<(), &'static str> {
    remote_mint(node_id, src_slot, remote_cnode)
}

// ── 26.2 Remote Memory Pooling ─────────────────────────────────────────────────

pub use memory::{
    remote_alloc_page, migrate_page_local, remote_free_page, alloc_remote, free_remote,
    print_pool_stats, get_available_pages, remote_alloc_pages, register_mem_node, set_available_pages,
};

// ── 26.3 Cluster Coordination ───────────────────────────────────────────────────

pub fn get_cluster_node_id() -> u8 {
    unsafe { CLUSTER_NODE_ID }
}

pub fn encode_global_pid(node_id: u8, local_pid: u32) -> u32 {
    ((node_id as u32) << 24) | (local_pid & 0x00FF_FFFF)
}

pub fn pid_to_node(pid: u32) -> u8 {
    (pid >> 24) as u8
}

pub fn pid_to_local(pid: u32) -> u32 {
    pid & 0x00FF_FFFF
}

pub fn get_global_pid() -> u32 {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).unwrap_or(0);
    encode_global_pid(unsafe { CLUSTER_NODE_ID }, pid)
}

pub fn remote_proclist(node_id: u32, out_buf: &mut [protocol::RemoteProcEntry]) -> Result<usize, &'static str> {
    let src_node = unsafe { CLUSTER_NODE_ID };

    let mut req_buf = [0u8; 8];
    let written = write_proclist_req(&mut req_buf, src_node).ok_or("serialize error")?;

    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).unwrap_or(0);

    let target_ep = DIST_EP_BASE + node_id as usize;
    let mut msg = Message::new(pid, 0xD1);
    msg.payload[..written].copy_from_slice(&req_buf[..written]);
    msg.payload_len = written;
    endpoint::send(target_ep, pid, msg)?;

    let deadline = unsafe { crate::trap::TICK_COUNT.wrapping_add(200) };
    loop {
        let buf = RCV_BUF.lock();
        let count = *RCV_BUF_COUNT.lock();
        for i in 0..count {
            if buf[i].valid {
                if let Some(count_out) = parse_proclist_resp(&buf[i].payload[..buf[i].payload_len], out_buf) {
                    return Ok(count_out);
                }
            }
        }
        drop(buf);
        if unsafe { crate::trap::TICK_COUNT } >= deadline {
            return Err("proclist timeout");
        }
        crate::sched::schedule();
    }
}

fn handle_proclist_request(src_node: u8) {
    let procs = crate::proc::PROCESSES.lock();
    let mut raw_entries: [(u32, u8, &[u8]); 64] = [(0, 0, &[]); 64];
    let mut count = 0;
    for proc in procs.iter() {
        if count >= 64 { break; }
        let state_byte = match proc.state {
            ProcessState::Ready => 0u8,
            ProcessState::Running => 1u8,
            ProcessState::Waiting => 2u8,
            ProcessState::Dead => 3u8,
        };
        raw_entries[count] = (proc.pid, state_byte, b"proc");
        count += 1;
    }
    drop(procs);

    let mut resp_buf = [0u8; 512];
    if let Some(len) = write_proclist_resp(&mut resp_buf, &raw_entries[..count]) {
        let mut msg = Message::new(0, 0xD1);
        let copy_len = len.min(64);
        msg.payload[..copy_len].copy_from_slice(&resp_buf[..copy_len]);
        msg.payload_len = copy_len;
        endpoint::send(DIST_EP_BASE + src_node as usize, 0, msg).ok();
    }
}

pub fn process_incoming_message(msg: &Message) {
    let buf = &msg.payload[..msg.payload_len];
    if buf.len() < 2 { return; }

    let magic = u16::from_le_bytes([buf[0], buf[1]]);
    match magic {
        MAGIC_PING => { if let Some(src) = parse_ping(buf) { handle_incoming_ping(src); } }
        MAGIC_PONG => { if let Some(src) = parse_pong(buf) { handle_incoming_pong(src); } }
        MAGIC_DATA => { if let Some((src, ep, pl)) = parse_data(buf) { queue_incoming_message(src, ep, pl); } }
        MAGIC_CAP => {
            let pid = crate::sched::current_thread().map(|t| unsafe { (*t).owner }).unwrap_or(0);
            let cnode_id = {
                let procs = crate::proc::PROCESSES.lock();
                procs.iter().find(|p| p.pid == pid).map(|p| p.cnode_id).unwrap_or(0)
            };
            if cnode_id > 0 { let _ = cap_receive(cnode_id, buf); }
        }
        MAGIC_PROCLIST_REQ => { if let Some(r) = parse_proclist_req(buf) { handle_proclist_request(r); } }
        _ => {}
    }
}

// ── Node registry operations ────────────────────────────────────────────────────

pub fn add_remote_node(ip: &[u8], port: u16) -> Option<u32> {
    let mut nodes = REMOTE_NODES.lock();
    let mut count = *REMOTE_NODE_COUNT.lock();
    if count >= MAX_REMOTE_NODES { return None; }

    let id = count as u32;
    let mut node = RemoteNode {
        node_id: id, ip_addr: [0; 16], port, online: false,
        last_seen_tick: 0, failed_pings: 0, registered: true,
    };
    let len = ip.len().min(16);
    for i in 0..len { node.ip_addr[i] = ip[i]; }
    nodes[count] = node;
    count += 1;
    *REMOTE_NODE_COUNT.lock() = count;
    memory::register_mem_node(id as u8);
    crate::println!("DIST: registered remote node {} (port {})", id, port);
    Some(id)
}

pub fn publish_endpoint(local_ep: usize, remote_node: u32, remote_ep: usize) -> bool {
    let mut eps = REMOTE_EPS.lock();
    let mut count = *REMOTE_EP_COUNT.lock();
    if count >= MAX_REMOTE_EPS { return false; }
    eps[count] = RemoteEndpoint { ep_id: local_ep, remote_node, remote_ep, registered: true };
    count += 1;
    *REMOTE_EP_COUNT.lock() = count;
    true
}

pub fn lookup_remote_ep(local_ep: usize) -> Option<(u32, usize)> {
    let eps = REMOTE_EPS.lock();
    let count = *REMOTE_EP_COUNT.lock();
    for i in 0..count {
        if eps[i].registered && eps[i].ep_id == local_ep {
            return Some((eps[i].remote_node, eps[i].remote_ep));
        }
    }
    None
}

pub fn remote_node_count() -> usize {
    *REMOTE_NODE_COUNT.lock()
}

pub fn is_node_online(node_id: u32) -> bool {
    let nodes = REMOTE_NODES.lock();
    let count = *REMOTE_NODE_COUNT.lock();
    for i in 0..count {
        if nodes[i].node_id == node_id { return nodes[i].online; }
    }
    false
}

pub fn get_node_info(node_id: u32) -> Option<(u32, [u8; 16], u16, bool)> {
    let nodes = REMOTE_NODES.lock();
    let count = *REMOTE_NODE_COUNT.lock();
    for i in 0..count {
        if nodes[i].node_id == node_id {
            return Some((nodes[i].node_id, nodes[i].ip_addr, nodes[i].port, nodes[i].online));
        }
    }
    None
}
