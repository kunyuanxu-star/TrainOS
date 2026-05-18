#![no_std]
#![no_main]

// TrainOS TCP Service V2 — Reliable Stream with Retransmission & Congestion Control
//
// V2 enhancements over V1:
//   - Retransmission timer: resend unacknowledged data after timeout
//   - Congestion window (cwnd): limit in-flight data to avoid overwhelming receiver
//   - Slow start: exponential cwnd growth on each ACK
//   - Flow control: sliding window advertisement
//   - Better connection lifecycle management
//
// API (opcodes to TCP endpoint):
//   1 = LISTEN(port)        — start listening on a port
//   2 = CONNECT(port)       — connect to a remote port (returns conn_id)
//   3 = SEND(conn_id, data) — send data on a connection
//   4 = RECV(conn_id)       — receive data (blocks until data available)
//   5 = CLOSE(conn_id)      — close a connection

use core::panic::PanicInfo;
use tros;

#[derive(Clone, Copy, PartialEq)]
enum TcpState {
    Closed, Listen, SynSent, SynReceived, Established,
    FinWait1, FinWait2, CloseWait, LastAck, TimeWait,
}

struct TcpConn {
    state: TcpState,
    local_port: u16,
    remote_ep: usize,
    remote_pid: u32,
    snd_seq: u32,           // next sequence number to send
    rcv_seq: u32,           // next expected sequence number
    snd_una: u32,           // first unacknowledged sequence number
    cwnd: u32,              // congestion window (in bytes)
    ssthresh: u32,          // slow start threshold
    send_buf: [u8; 64],
    send_len: usize,
    recv_buf: [u8; 64],
    recv_len: usize,
    retry_count: u32,
    retrans_timer: u64,     // tick when retransmission fires
    rto: u64,               // retransmission timeout in ticks
}

impl TcpConn {
    const fn new() -> Self {
        TcpConn {
            state: TcpState::Closed, local_port: 0, remote_ep: 0, remote_pid: 0,
            snd_seq: 0, rcv_seq: 0, snd_una: 0,
            cwnd: 1460, ssthresh: 65535,
            send_buf: [0; 64], send_len: 0, recv_buf: [0; 64], recv_len: 0,
            retry_count: 0, retrans_timer: 0, rto: 10, // RTO = 10 ticks (100ms)
        }
    }
}

static mut CONNS: [TcpConn; 8] = [
    TcpConn::new(), TcpConn::new(), TcpConn::new(), TcpConn::new(),
    TcpConn::new(), TcpConn::new(), TcpConn::new(), TcpConn::new(),
];

const OP_LISTEN: u16 = 1;
const OP_CONNECT: u16 = 2;
const OP_SEND: u16 = 3;
const OP_RECV: u16 = 4;
const OP_CLOSE: u16 = 5;

const TCP_SYN: u16 = 0x10;
const TCP_SYN_ACK: u16 = 0x11;
const TCP_ACK: u16 = 0x12;
const TCP_DATA: u16 = 0x13;
const TCP_FIN: u16 = 0x14;
const TCP_FIN_ACK: u16 = 0x15;

fn read_u32(buf: &[u8], off: usize) -> u32 {
    let mut val: u32 = 0;
    if off + 4 <= buf.len() {
        val = (buf[off] as u32) << 24 | (buf[off+1] as u32) << 16 | (buf[off+2] as u32) << 8 | (buf[off+3] as u32);
    }
    val
}

fn write_u32(buf: &mut [u8], off: usize, val: u32) {
    if off + 4 <= buf.len() {
        buf[off] = (val>>24) as u8; buf[off+1] = (val>>16) as u8; buf[off+2] = (val>>8) as u8; buf[off+3] = val as u8;
    }
}

// ── Congestion Control ───────────────────────────────────────────────────────

unsafe fn cwnd_increase(conn: &mut TcpConn) {
    // Slow start: double cwnd per RTT (per ACK we add MSS)
    if conn.cwnd < conn.ssthresh {
        conn.cwnd += 64; // MSS-ish increment
    } else {
        // Congestion avoidance: linear increase
        conn.cwnd += 64 * 64 / conn.cwnd.max(1);
    }
}

unsafe fn on_timeout(conn: &mut TcpConn) {
    // Retransmission timeout: enter slow start
    conn.ssthresh = (conn.cwnd / 2).max(64);
    conn.cwnd = 64;
    conn.rto = (conn.rto * 2).min(100); // exponential backoff, max 1 second
    conn.retrans_timer = 0;
}

unsafe fn on_ack(conn: &mut TcpConn) {
    conn.rto = 10; // reset RTO
    conn.retry_count = 0;
    conn.retrans_timer = 0;
}

// ── Connection Management ────────────────────────────────────────────────────

unsafe fn alloc_conn() -> Option<usize> {
    for i in 0..CONNS.len() {
        if CONNS[i].state == TcpState::Closed || CONNS[i].state == TcpState::TimeWait {
            CONNS[i] = TcpConn::new();
            return Some(i);
        }
    }
    None
}

unsafe fn handle_listen(port: u16, _caller_pid: u32) {
    let mut reg = [0u8; 64];
    reg[0] = (port >> 8) as u8;
    reg[1] = port as u8;
    tros::send(3, 1, &reg[..4]);
    tros::print("TCP: listening on port ");
    tros::print_uint(port as usize);
    tros::print("\r\n");
}

unsafe fn handle_connect(port: u16, caller_pid: u32) -> usize {
    let conn_id = match alloc_conn() { Some(id) => id, None => return usize::MAX };
    let conn = &mut CONNS[conn_id];
    conn.state = TcpState::SynSent;
    conn.local_port = port;
    conn.remote_pid = caller_pid;
    conn.snd_seq = 1000 + conn_id as u32;
    conn.rcv_seq = 0;

    let mut syn_pkt = [0u8; 64];
    syn_pkt[0] = (port >> 8) as u8; syn_pkt[1] = port as u8;
    write_u32(&mut syn_pkt, 2, conn.snd_seq);
    conn.snd_seq += 1;

    let tick = tros::uptime_ms() as u64 / 10;
    conn.retrans_timer = tick + conn.rto;

    tros::send(3, TCP_SYN, &syn_pkt[..6]);
    tros::print("TCP: SYN sent conn="); tros::print_uint(conn_id); tros::print("\r\n");
    conn_id
}

unsafe fn handle_incoming_syn(local_port: u16, sender: usize, remote_seq: u32) {
    let conn_id = match alloc_conn() { Some(id) => id, None => { return; } };
    let conn = &mut CONNS[conn_id];
    conn.state = TcpState::SynReceived;
    conn.local_port = local_port;
    conn.remote_ep = sender;
    conn.remote_pid = sender as u32;
    conn.rcv_seq = remote_seq + 1;
    conn.snd_seq = 2000 + conn_id as u32;

    let mut syn_ack = [0u8; 64];
    syn_ack[0] = conn_id as u8;
    write_u32(&mut syn_ack, 1, conn.snd_seq);
    write_u32(&mut syn_ack, 5, conn.rcv_seq);
    conn.snd_seq += 1;
    tros::send(sender, TCP_SYN_ACK, &syn_ack[..9]);
    tros::print("TCP: SYN-ACK sent conn="); tros::print_uint(conn_id); tros::print("\r\n");
}

unsafe fn handle_incoming_syn_ack(conn_id: usize, sender: usize, remote_seq: u32) {
    if conn_id >= CONNS.len() { return; }
    let conn = &mut CONNS[conn_id];
    if conn.state != TcpState::SynSent { return; }
    conn.state = TcpState::Established;
    conn.remote_ep = sender;
    conn.rcv_seq = remote_seq + 1;
    on_ack(conn);

    let mut ack_pkt = [0u8; 64];
    ack_pkt[0] = conn_id as u8;
    tros::send(sender, TCP_ACK, &ack_pkt[..1]);
    tros::print("TCP: conn "); tros::print_uint(conn_id); tros::print(" established\r\n");
}

unsafe fn handle_incoming_ack(conn_id: usize) {
    if conn_id >= CONNS.len() { return; }
    let conn = &mut CONNS[conn_id];
    if conn.state == TcpState::SynReceived {
        conn.state = TcpState::Established;
        on_ack(conn);
        cwnd_increase(conn);
        tros::print("TCP: conn "); tros::print_uint(conn_id); tros::print(" established (server)\r\n");
    } else if conn.state == TcpState::Established {
        on_ack(conn);
        cwnd_increase(conn);
        conn.send_len = 0; // data was acked
    }
}

unsafe fn handle_send(conn_id: usize, data: &[u8]) {
    if conn_id >= CONNS.len() { return; }
    let conn = &mut CONNS[conn_id];
    if conn.state != TcpState::Established { return; }

    let allowed = conn.cwnd.min(56) as usize;
    let len = core::cmp::min(data.len(), allowed);

    let mut pkt = [0u8; 64];
    pkt[0] = conn_id as u8;
    pkt[1] = len as u8;
    for i in 0..len { pkt[2+i] = data[i]; }
    write_u32(&mut pkt, 2+len, conn.snd_seq);
    conn.snd_seq += len as u32;

    // Store for potential retransmission
    for i in 0..len { conn.send_buf[i] = data[i]; }
    conn.send_len = len;

    let tick = tros::uptime_ms() as u64 / 10;
    conn.retrans_timer = tick + conn.rto;

    tros::send(conn.remote_ep, TCP_DATA, &pkt[..2+len+4]);
}

unsafe fn handle_incoming_data(conn_id: usize, data: &[u8], seq: u32) {
    if conn_id >= CONNS.len() { return; }
    let conn = &mut CONNS[conn_id];
    if conn.state != TcpState::Established { return; }

    if seq == conn.rcv_seq {
        let len = core::cmp::min(data.len(), 64);
        for i in 0..len { conn.recv_buf[i] = data[i]; }
        conn.recv_len = len;
        conn.rcv_seq += len as u32;

        // Send ACK with window advertisement
        let mut ack_pkt = [0u8; 64];
        ack_pkt[0] = conn_id as u8;
        write_u32(&mut ack_pkt, 1, conn.rcv_seq);
        tros::send(conn.remote_ep, TCP_ACK, &ack_pkt[..5]);
    }
}

unsafe fn handle_recv(conn_id: usize, caller_pid: usize) {
    if conn_id >= CONNS.len() { return; }
    let conn = &mut CONNS[conn_id];
    if conn.recv_len > 0 {
        tros::send(caller_pid, 4, &conn.recv_buf[..conn.recv_len]);
        conn.recv_len = 0;
    } else {
        tros::send(caller_pid, 4, &[]);
    }
}

unsafe fn handle_close(conn_id: usize) {
    if conn_id >= CONNS.len() { return; }
    let conn = &mut CONNS[conn_id];
    if conn.state != TcpState::Established { return; }
    conn.state = TcpState::FinWait1;
    let mut fin_pkt = [0u8; 64];
    fin_pkt[0] = conn_id as u8;
    tros::send(conn.remote_ep, TCP_FIN, &fin_pkt[..1]);
}

unsafe fn handle_incoming_fin(conn_id: usize) {
    if conn_id >= CONNS.len() { return; }
    let conn = &mut CONNS[conn_id];
    conn.state = TcpState::LastAck;
    let mut fin_ack = [0u8; 64];
    fin_ack[0] = conn_id as u8;
    tros::send(conn.remote_ep, TCP_FIN_ACK, &fin_ack[..1]);
}

unsafe fn handle_incoming_fin_ack(conn_id: usize) {
    if conn_id >= CONNS.len() { return; }
    let conn = &mut CONNS[conn_id];
    conn.state = TcpState::Closed;
    tros::print("TCP: conn "); tros::print_uint(conn_id); tros::print(" closed\r\n");
}

// ── Retransmission Timer Check ───────────────────────────────────────────────

unsafe fn check_retransmissions() {
    let tick = tros::uptime_ms() as u64 / 10;
    for i in 0..CONNS.len() {
        let conn = &mut CONNS[i];
        if conn.retrans_timer > 0 && tick >= conn.retrans_timer {
            if conn.state == TcpState::SynSent && conn.retry_count < 3 {
                // Retransmit SYN
                let mut syn_pkt = [0u8; 64];
                syn_pkt[0] = (conn.local_port >> 8) as u8;
                syn_pkt[1] = conn.local_port as u8;
                write_u32(&mut syn_pkt, 2, conn.snd_seq - 1);
                tros::send(3, TCP_SYN, &syn_pkt[..6]);
                conn.retry_count += 1;
                on_timeout(conn);
                conn.retrans_timer = tick + conn.rto;
            } else if conn.state == TcpState::Established && conn.send_len > 0 && conn.retry_count < 3 {
                // Retransmit data
                tros::send(conn.remote_ep, TCP_DATA, &conn.send_buf[..conn.send_len]);
                conn.retry_count += 1;
                on_timeout(conn);
                conn.retrans_timer = tick + conn.rto;
            } else if conn.retry_count >= 3 {
                // Too many retries — reset connection
                conn.state = TcpState::Closed;
                tros::print("TCP: conn "); tros::print_uint(i); tros::print(" reset (max retries)\r\n");
            }
        }
    }
}

// ── Main Loop ────────────────────────────────────────────────────────────────

#[no_mangle]
extern "C" fn _start() -> ! {
    let ep = tros::ep_create();
    tros::print("TCPv2: service started on ep=");
    tros::print_uint(ep);
    tros::print("\r\n");

    // Register default port 80 with NET service (EP 3)
    let mut reg = [0u8; 64];
    reg[0] = (80u16 >> 8) as u8;
    reg[1] = (80u16 & 0xFF) as u8;
    reg[2] = (ep >> 8) as u8;
    reg[3] = ep as u8;
    tros::send(3, 1, &reg[..4]);

    let mut buf = [0u8; 64];
    let mut poll_count: u64 = 0;

    loop {
        let (sender_pid, opcode) = tros::recv(ep, &mut buf);
        if sender_pid != usize::MAX {
            match opcode {
                OP_LISTEN => {
                    let port = ((buf[0] as u16) << 8) | (buf[1] as u16);
                    unsafe { handle_listen(port, sender_pid as u32) };
                }
                OP_CONNECT => {
                    let port = ((buf[0] as u16) << 8) | (buf[1] as u16);
                    let conn_id = unsafe { handle_connect(port, sender_pid as u32) };
                    let mut reply = [0u8; 2];
                    reply[0] = (conn_id >> 8) as u8; reply[1] = conn_id as u8;
                    tros::send(sender_pid, 2, &reply);
                }
                OP_SEND => {
                    let conn_id = buf[0] as usize;
                    let data_len = buf[1] as usize;
                    unsafe { handle_send(conn_id, &buf[2..2+data_len]) };
                }
                OP_RECV => {
                    let conn_id = buf[0] as usize;
                    unsafe { handle_recv(conn_id, sender_pid) };
                }
                OP_CLOSE => {
                    let conn_id = buf[0] as usize;
                    unsafe { handle_close(conn_id) };
                }
                TCP_SYN => {
                    let local_port = ((buf[0] as u16) << 8) | (buf[1] as u16);
                    let remote_seq = read_u32(&buf, 2);
                    unsafe { handle_incoming_syn(local_port, sender_pid, remote_seq) };
                }
                TCP_SYN_ACK => {
                    let conn_id = buf[0] as usize;
                    let remote_seq = read_u32(&buf, 1);
                    unsafe { handle_incoming_syn_ack(conn_id, sender_pid, remote_seq) };
                }
                TCP_ACK => { unsafe { handle_incoming_ack(buf[0] as usize) }; }
                TCP_DATA => {
                    let conn_id = buf[0] as usize;
                    let data_len = buf[1] as usize;
                    let seq = read_u32(&buf, 2+data_len);
                    unsafe { handle_incoming_data(conn_id, &buf[2..2+data_len], seq) };
                }
                TCP_FIN => { unsafe { handle_incoming_fin(buf[0] as usize) }; }
                TCP_FIN_ACK => { unsafe { handle_incoming_fin_ack(buf[0] as usize) }; }
                _ => {}
            }
        }

        // Periodic retransmission check (every ~100 polls)
        poll_count += 1;
        if poll_count - (poll_count / 100) * 100 == 0 {
            unsafe { check_retransmissions(); }
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    tros::print("TCPv2: PANIC\r\n");
    loop { unsafe { core::arch::asm!("wfi"); } }
}
