// V26: Distributed IPC wire protocol
//
// Defines the on-the-wire message format for inter-node communication.
// All multi-byte values are little-endian.
//
// Node discovery:
//   PING:  [magic:2][node_id:4]                        = 6 bytes
//   PONG:  [magic:2][node_id:4]                        = 6 bytes
//
// Remote message:
//   DATA:  [magic:2][src_node:1][remote_ep:2][payload_len:1][payload:<=59]  = 6..65 bytes
//
// Capability transfer:
//   CAP:   [magic:2][src_node:1][cap_type:1][rights:1][slot:4][res_data:64] = 73 bytes
//
// Remote memory operations:
//   MEM_ALLOC_REQ:  [magic:2][requester:1][num_pages:1]   = 4 bytes
//   MEM_ALLOC_RESP: [magic:2][requester:1][num_pages:1][handles:4*num_pages] = 4..68 bytes
//   MEM_FREE:       [magic:2][requester:1][handle:4]      = 7 bytes
//   MEM_READ:       [magic:2][requester:1][handle:4][offset:4][len:2] = 13 bytes
//   MEM_READ_RESP:  [magic:2][requester:1][handle:4][offset:4][len:2][data...]
//
// Remote process operations:
//   PROCLIST_REQ:   [magic:2][requester:1]               = 3 bytes
//   PROCLIST_RESP:  [magic:2][count:2][entries...]       = variable
//   PROC_ENTRY:     [pid:4][state:1][namelen:1][name:<=48] = 6..54 bytes

// ── Magic constants ─────────────────────────────────────────────────────────────

pub const MAGIC_PING: u16 = 0xD1A0;
pub const MAGIC_PONG: u16 = 0xD1A1;
pub const MAGIC_DATA: u16 = 0xD1A2;
pub const MAGIC_CAP: u16 = 0xD1A3;
pub const MAGIC_MEM_ALLOC_REQ: u16 = 0xD1B0;
pub const MAGIC_MEM_ALLOC_RESP: u16 = 0xD1B1;
pub const MAGIC_MEM_FREE: u16 = 0xD1B2;
pub const MAGIC_MEM_READ: u16 = 0xD1B3;
pub const MAGIC_MEM_READ_RESP: u16 = 0xD1B4;
pub const MAGIC_PROCLIST_REQ: u16 = 0xD1C0;
pub const MAGIC_PROCLIST_RESP: u16 = 0xD1C1;

pub const DIST_IPC_PORT: u16 = 0xD1C;

/// Maximum wire payload we can fit in a single packet.
pub const MAX_WIRE_PAYLOAD: usize = 59;

// ── Ping / Pong (node discovery) ────────────────────────────────────────────────

/// Format a PING packet into the given buffer.
/// Returns the number of bytes written.
pub fn write_ping(buf: &mut [u8], node_id: u32) -> Option<usize> {
    if buf.len() < 6 {
        return None;
    }
    let magic = MAGIC_PING.to_le_bytes();
    let id = node_id.to_le_bytes();
    buf[0] = magic[0];
    buf[1] = magic[1];
    buf[2] = id[0];
    buf[3] = id[1];
    buf[4] = id[2];
    buf[5] = id[3];
    Some(6)
}

/// Format a PONG packet into the given buffer.
pub fn write_pong(buf: &mut [u8], node_id: u32) -> Option<usize> {
    if buf.len() < 6 {
        return None;
    }
    let magic = MAGIC_PONG.to_le_bytes();
    let id = node_id.to_le_bytes();
    buf[0] = magic[0];
    buf[1] = magic[1];
    buf[2] = id[0];
    buf[3] = id[1];
    buf[4] = id[2];
    buf[5] = id[3];
    Some(6)
}

/// Parse a PING packet. Returns the sender's node_id on success.
pub fn parse_ping(buf: &[u8]) -> Option<u32> {
    if buf.len() < 6 {
        return None;
    }
    let magic = u16::from_le_bytes([buf[0], buf[1]]);
    if magic != MAGIC_PING {
        return None;
    }
    let node_id = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]);
    Some(node_id)
}

/// Parse a PONG packet. Returns the sender's node_id on success.
pub fn parse_pong(buf: &[u8]) -> Option<u32> {
    if buf.len() < 6 {
        return None;
    }
    let magic = u16::from_le_bytes([buf[0], buf[1]]);
    if magic != MAGIC_PONG {
        return None;
    }
    let node_id = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]);
    Some(node_id)
}

// ── Data message serialization ──────────────────────────────────────────────────

/// Format a DATA packet: [magic:2][src_node:1][remote_ep:2][payload_len:1][payload:<=59]
/// Returns bytes written, or None if buffer too small.
pub fn write_data(buf: &mut [u8], src_node: u8, remote_ep: u16, payload: &[u8]) -> Option<usize> {
    let plen = payload.len();
    if plen > MAX_WIRE_PAYLOAD {
        return None;
    }
    let total = 6 + plen; // magic(2) + src_node(1) + ep(2) + plen(1) + payload
    if buf.len() < total {
        return None;
    }
    let magic = MAGIC_DATA.to_le_bytes();
    let ep_bytes = remote_ep.to_le_bytes();
    buf[0] = magic[0];
    buf[1] = magic[1];
    buf[2] = src_node;
    buf[3] = ep_bytes[0];
    buf[4] = ep_bytes[1];
    buf[5] = plen as u8;
    if plen > 0 {
        buf[6..6 + plen].copy_from_slice(payload);
    }
    Some(total)
}

/// Parse a DATA packet. Returns (src_node, remote_ep, payload_slice) on success.
pub fn parse_data<'a>(buf: &'a [u8]) -> Option<(u8, u16, &'a [u8])> {
    if buf.len() < 6 {
        return None;
    }
    let magic = u16::from_le_bytes([buf[0], buf[1]]);
    if magic != MAGIC_DATA {
        return None;
    }
    let src_node = buf[2];
    let remote_ep = u16::from_le_bytes([buf[3], buf[4]]);
    let plen = buf[5] as usize;
    if buf.len() < 6 + plen {
        return None;
    }
    Some((src_node, remote_ep, &buf[6..6 + plen]))
}

// ── Capability transfer serialization ───────────────────────────────────────────

/// Format a CAP packet: [magic:2][src_node:1][cap_type:1][rights:1][slot:4][res_data:64]
/// Returns bytes written, or None if buffer too small.
pub fn write_cap(
    buf: &mut [u8],
    src_node: u8,
    cap_type: u8,
    rights: u8,
    slot: u32,
    resource_data: &[u8; 64],
) -> Option<usize> {
    if buf.len() < 73 {
        return None;
    }
    let magic = MAGIC_CAP.to_le_bytes();
    let slot_bytes = slot.to_le_bytes();
    buf[0] = magic[0];
    buf[1] = magic[1];
    buf[2] = src_node;
    buf[3] = cap_type;
    buf[4] = rights;
    buf[5] = slot_bytes[0];
    buf[6] = slot_bytes[1];
    buf[7] = slot_bytes[2];
    buf[8] = slot_bytes[3];
    buf[9..73].copy_from_slice(resource_data);
    Some(73)
}

/// Parse a CAP packet. Returns (src_node, cap_type, rights, slot, resource_data).
pub fn parse_cap(buf: &[u8]) -> Option<(u8, u8, u8, u32, [u8; 64])> {
    if buf.len() < 73 {
        return None;
    }
    let magic = u16::from_le_bytes([buf[0], buf[1]]);
    if magic != MAGIC_CAP {
        return None;
    }
    let src_node = buf[2];
    let cap_type = buf[3];
    let rights = buf[4];
    let slot = u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]);
    let mut res_data = [0u8; 64];
    res_data.copy_from_slice(&buf[9..73]);
    Some((src_node, cap_type, rights, slot, res_data))
}

// ── Remote memory operation serialization ───────────────────────────────────────

/// Format a MEM_ALLOC_REQ packet: [magic:2][requester:1][num_pages:1]
pub fn write_mem_alloc_req(buf: &mut [u8], requester: u8, num_pages: u8) -> Option<usize> {
    if buf.len() < 4 {
        return None;
    }
    let magic = MAGIC_MEM_ALLOC_REQ.to_le_bytes();
    buf[0] = magic[0];
    buf[1] = magic[1];
    buf[2] = requester;
    buf[3] = num_pages;
    Some(4)
}

/// Parse MEM_ALLOC_REQ packet. Returns (requester, num_pages).
pub fn parse_mem_alloc_req(buf: &[u8]) -> Option<(u8, u8)> {
    if buf.len() < 4 {
        return None;
    }
    let magic = u16::from_le_bytes([buf[0], buf[1]]);
    if magic != MAGIC_MEM_ALLOC_REQ {
        return None;
    }
    Some((buf[2], buf[3]))
}

/// Format a MEM_ALLOC_RESP packet.
/// Format: [magic:2][requester:1][num_pages:1][handle:4]...
/// Each handle is a u32 representing a remote memory handle.
pub fn write_mem_alloc_resp(buf: &mut [u8], requester: u8, handles: &[u32]) -> Option<usize> {
    let num = handles.len();
    if num > 16 {
        return None;
    }
    let total = 4 + num * 4;
    if buf.len() < total {
        return None;
    }
    let magic = MAGIC_MEM_ALLOC_RESP.to_le_bytes();
    buf[0] = magic[0];
    buf[1] = magic[1];
    buf[2] = requester;
    buf[3] = num as u8;
    for (i, h) in handles.iter().enumerate() {
        let hb = h.to_le_bytes();
        let off = 4 + i * 4;
        buf[off] = hb[0];
        buf[off + 1] = hb[1];
        buf[off + 2] = hb[2];
        buf[off + 3] = hb[3];
    }
    Some(total)
}

/// Parse MEM_ALLOC_RESP. Returns (requester, Vec<handle>).
pub fn parse_mem_alloc_resp(buf: &[u8]) -> Option<(u8, &[u8])> {
    if buf.len() < 4 {
        return None;
    }
    let magic = u16::from_le_bytes([buf[0], buf[1]]);
    if magic != MAGIC_MEM_ALLOC_RESP {
        return None;
    }
    let requester = buf[2];
    let num = buf[3] as usize;
    if buf.len() < 4 + num * 4 {
        return None;
    }
    Some((requester, &buf[4..4 + num * 4]))
}

/// Format a MEM_FREE packet: [magic:2][requester:1][handle:4]
pub fn write_mem_free(buf: &mut [u8], requester: u8, handle: u32) -> Option<usize> {
    if buf.len() < 7 {
        return None;
    }
    let magic = MAGIC_MEM_FREE.to_le_bytes();
    let hb = handle.to_le_bytes();
    buf[0] = magic[0];
    buf[1] = magic[1];
    buf[2] = requester;
    buf[3] = hb[0];
    buf[4] = hb[1];
    buf[5] = hb[2];
    buf[6] = hb[3];
    Some(7)
}

/// Parse MEM_FREE packet. Returns (requester, handle).
pub fn parse_mem_free(buf: &[u8]) -> Option<(u8, u32)> {
    if buf.len() < 7 {
        return None;
    }
    let magic = u16::from_le_bytes([buf[0], buf[1]]);
    if magic != MAGIC_MEM_FREE {
        return None;
    }
    let requester = buf[2];
    let handle = u32::from_le_bytes([buf[3], buf[4], buf[5], buf[6]]);
    Some((requester, handle))
}

// ── Remote process list serialization ───────────────────────────────────────────

/// Write a PROCLIST_REQ packet: [magic:2][requester:1]
pub fn write_proclist_req(buf: &mut [u8], requester: u8) -> Option<usize> {
    if buf.len() < 3 {
        return None;
    }
    let magic = MAGIC_PROCLIST_REQ.to_le_bytes();
    buf[0] = magic[0];
    buf[1] = magic[1];
    buf[2] = requester;
    Some(3)
}

/// Parse PROCLIST_REQ. Returns requester node id.
pub fn parse_proclist_req(buf: &[u8]) -> Option<u8> {
    if buf.len() < 3 {
        return None;
    }
    let magic = u16::from_le_bytes([buf[0], buf[1]]);
    if magic != MAGIC_PROCLIST_REQ {
        return None;
    }
    Some(buf[2])
}

/// Write a PROCLIST_RESP packet header + entries.
/// Entry format: [pid:4][state:1][namelen:1][name:<=48]
pub fn write_proclist_resp(buf: &mut [u8], entries: &[(u32, u8, &[u8])]) -> Option<usize> {
    let magic = MAGIC_PROCLIST_RESP.to_le_bytes();
    let count = entries.len();
    if count > 64 {
        return None;
    }
    // Header: magic(2) + count(2)
    let mut pos = 4;
    for (pid, state, name) in entries {
        let nlen = name.len().min(48);
        let entry_size = 6 + nlen; // pid(4) + state(1) + namelen(1) + name(nlen)
        if buf.len() < pos + entry_size {
            return None;
        }
        let pid_bytes = pid.to_le_bytes();
        buf[pos] = pid_bytes[0];
        buf[pos + 1] = pid_bytes[1];
        buf[pos + 2] = pid_bytes[2];
        buf[pos + 3] = pid_bytes[3];
        buf[pos + 4] = *state;
        buf[pos + 5] = nlen as u8;
        buf[pos + 6..pos + 6 + nlen].copy_from_slice(&name[..nlen]);
        pos += entry_size;
    }
    // Write header now that we know total size
    buf[0] = magic[0];
    buf[1] = magic[1];
    let count_bytes = (count as u16).to_le_bytes();
    buf[2] = count_bytes[0];
    buf[3] = count_bytes[1];
    Some(pos)
}

/// A parsed remote process entry.
#[derive(Debug, Clone, Copy)]
pub struct RemoteProcEntry {
    pub pid: u32,
    pub state: u8,
    pub name: [u8; 48],
    pub name_len: usize,
}

/// Parse a PROCLIST_RESP buffer. Returns the entries.
/// Caller must provide an output buffer for the entries.
pub fn parse_proclist_resp(buf: &[u8], out: &mut [RemoteProcEntry]) -> Option<usize> {
    if buf.len() < 4 {
        return None;
    }
    let magic = u16::from_le_bytes([buf[0], buf[1]]);
    if magic != MAGIC_PROCLIST_RESP {
        return None;
    }
    let count = u16::from_le_bytes([buf[2], buf[3]]) as usize;
    if count > out.len() {
        return None;
    }
    let mut pos = 4;
    for i in 0..count {
        if pos + 6 > buf.len() {
            return None;
        }
        let pid = u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        let state = buf[pos + 4];
        let nlen = buf[pos + 5] as usize;
        if pos + 6 + nlen > buf.len() {
            return None;
        }
        let mut name = [0u8; 48];
        let copy_len = nlen.min(48);
        name[..copy_len].copy_from_slice(&buf[pos + 6..pos + 6 + copy_len]);
        out[i] = RemoteProcEntry {
            pid,
            state,
            name,
            name_len: copy_len,
        };
        pos += 6 + nlen;
    }
    Some(count)
}
