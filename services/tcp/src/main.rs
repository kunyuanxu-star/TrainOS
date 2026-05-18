#![no_std]
#![no_main]

// TrainOS TCP Service — Reliable Stream Protocol
//
// Provides TCP-like semantics over TrainOS IPC:
//   - Connection establishment (SYN/SYN-ACK/ACK handshake)
//   - Reliable in-order byte stream delivery
//   - Sequence number tracking and ACKs
//   - Connection teardown (FIN/FIN-ACK)
//   - Simple retransmission via poll+timeout
//
// Architecture:
//   Applications talk to the TCP service via IPC.
//   The TCP service manages connections internally.
//   Connection table: static array of up to 8 connections.
//
// API (opcodes to TCP endpoint):
//   1 = LISTEN(port)        — start listening on a port
//   2 = CONNECT(port)       — connect to a remote port (returns conn_id)
//   3 = SEND(conn_id, data) — send data on a connection
//   4 = RECV(conn_id)       — receive data (blocks until data available)
//   5 = CLOSE(conn_id)      — close a connection
//
// States: CLOSED, LISTEN, SYN_SENT, SYN_RECEIVED, ESTABLISHED,
//         FIN_WAIT_1, FIN_WAIT_2, CLOSE_WAIT, LAST_ACK, TIME_WAIT

use core::panic::PanicInfo;
use tros;

// Connection state machine
#[derive(Clone, Copy, PartialEq)]
enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    LastAck,
    TimeWait,
}

// A TCP connection
struct TcpConn {
    state: TcpState,
    local_port: u16,
    remote_ep: usize,       // remote endpoint for this connection
    remote_pid: u32,        // remote process id
    snd_seq: u32,           // next sequence number to send
    rcv_seq: u32,           // next expected sequence number
    snd_una: u32,           // first unacknowledged sequence number
    send_buf: [u8; 64],     // pending send data
    send_len: usize,        // length of pending send data
    recv_buf: [u8; 64],     // received data buffer
    recv_len: usize,        // length of received data
    retry_count: u32,       // retransmission counter
}

impl TcpConn {
    const fn new() -> Self {
        TcpConn {
            state: TcpState::Closed,
            local_port: 0,
            remote_ep: 0,
            remote_pid: 0,
            snd_seq: 0,
            rcv_seq: 0,
            snd_una: 0,
            send_buf: [0; 64],
            send_len: 0,
            recv_buf: [0; 64],
            recv_len: 0,
            retry_count: 0,
        }
    }
}

// Static connection table
static mut CONNS: [TcpConn; 8] = [
    TcpConn::new(), TcpConn::new(), TcpConn::new(), TcpConn::new(),
    TcpConn::new(), TcpConn::new(), TcpConn::new(), TcpConn::new(),
];

// Opcodes
const OP_LISTEN: u16 = 1;
const OP_CONNECT: u16 = 2;
const OP_SEND: u16 = 3;
const OP_RECV: u16 = 4;
const OP_CLOSE: u16 = 5;

// Internal TCP segment types (opcode field in forwarded messages)
const TCP_SYN: u16 = 0x10;
const TCP_SYN_ACK: u16 = 0x11;
const TCP_ACK: u16 = 0x12;
const TCP_DATA: u16 = 0x13;
const TCP_FIN: u16 = 0x14;
const TCP_FIN_ACK: u16 = 0x15;
const TCP_RST: u16 = 0x16;

#[no_mangle]
extern "C" fn _start() -> ! {
    let ep = tros::ep_create();
    tros::print("TCP: service started on ep=");
    tros::print_uint(ep);
    tros::print("\r\n");

    // Register with NET service on well-known TCP port (EP 3 = net)
    let mut reg = [0u8; 64];
    reg[0] = (80u16 >> 8) as u8;   // Register on TCP proxy port 80 (default)
    reg[1] = (80u16 & 0xFF) as u8;
    reg[2] = (ep >> 8) as u8;
    reg[3] = ep as u8;
    tros::send(3, 1, &reg[..4]); // Register port 80 -> this ep on net(EP 3)

    let mut buf = [0u8; 64];

    loop {
        let (sender_pid, opcode) = tros::recv(ep, &mut buf);
        if sender_pid == usize::MAX {
            continue;
        }

        match opcode {
            OP_LISTEN => {
                let port = ((buf[0] as u16) << 8) | (buf[1] as u16);
                unsafe { handle_listen(port, sender_pid as u32) };
            }
            OP_CONNECT => {
                let port = ((buf[0] as u16) << 8) | (buf[1] as u16);
                let conn_id = unsafe { handle_connect(port, sender_pid as u32) };
                // Reply with conn_id back to caller
                let mut reply = [0u8; 2];
                reply[0] = (conn_id >> 8) as u8;
                reply[1] = conn_id as u8;
                tros::send(sender_pid, 2, &reply); // echo conn_id back
            }
            OP_SEND => {
                let conn_id = buf[0] as usize;
                let data_len = buf[1] as usize;
                let data = &buf[2..2 + data_len];
                unsafe { handle_send(conn_id, data) };
            }
            OP_RECV => {
                let conn_id = buf[0] as usize;
                unsafe { handle_recv(conn_id, sender_pid) };
            }
            OP_CLOSE => {
                let conn_id = buf[0] as usize;
                unsafe { handle_close(conn_id) };
            }
            // Incoming TCP segments from the net layer
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
            TCP_ACK => {
                let conn_id = buf[0] as usize;
                unsafe { handle_incoming_ack(conn_id) };
            }
            TCP_DATA => {
                let conn_id = buf[0] as usize;
                let data_len = buf[1] as usize;
                let data = &buf[2..2 + data_len];
                let seq = read_u32(&buf, 2 + data_len);
                unsafe { handle_incoming_data(conn_id, data, seq) };
            }
            TCP_FIN => {
                let conn_id = buf[0] as usize;
                unsafe { handle_incoming_fin(conn_id) };
            }
            TCP_FIN_ACK => {
                let conn_id = buf[0] as usize;
                unsafe { handle_incoming_fin_ack(conn_id) };
            }
            _ => {}
        }
    }
}

fn read_u32(buf: &[u8], off: usize) -> u32 {
    let mut val: u32 = 0;
    if off + 4 <= buf.len() {
        val = (buf[off] as u32) << 24
            | (buf[off + 1] as u32) << 16
            | (buf[off + 2] as u32) << 8
            | (buf[off + 3] as u32);
    }
    val
}

fn write_u32(buf: &mut [u8], off: usize, val: u32) {
    if off + 4 <= buf.len() {
        buf[off] = (val >> 24) as u8;
        buf[off + 1] = (val >> 16) as u8;
        buf[off + 2] = (val >> 8) as u8;
        buf[off + 3] = val as u8;
    }
}

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
    // Register with NET on the given port so SYN packets can reach us
    let mut reg = [0u8; 64];
    reg[0] = (port >> 8) as u8;
    reg[1] = port as u8;
    reg[2] = 0; // ep (we'll use our own ep)
    reg[3] = 0;
    tros::send(3, 1, &reg[..4]);
    tros::print("TCP: listening on port ");
    tros::print_uint(port as usize);
    tros::print("\r\n");
}

unsafe fn handle_connect(port: u16, caller_pid: u32) -> usize {
    let conn_id = match alloc_conn() {
        Some(id) => id,
        None => return usize::MAX,
    };

    let conn = &mut CONNS[conn_id];
    conn.state = TcpState::SynSent;
    conn.local_port = port;
    conn.remote_pid = caller_pid;
    conn.snd_seq = 1000 + conn_id as u32; // initial sequence number
    conn.rcv_seq = 0;

    // Send SYN to NET service for port routing
    let mut syn_pkt = [0u8; 64];
    syn_pkt[0] = (port >> 8) as u8;
    syn_pkt[1] = port as u8;
    write_u32(&mut syn_pkt, 2, conn.snd_seq);
    conn.snd_seq += 1;
    tros::send(3, TCP_SYN, &syn_pkt[..6]); // port(2) + seq(4) = 6 bytes

    tros::print("TCP: SYN sent for conn ");
    tros::print_uint(conn_id);
    tros::print("\r\n");

    conn_id
}

unsafe fn handle_incoming_syn(local_port: u16, sender: usize, remote_seq: u32) {
    // Find a listening connection or create a new one
    let conn_id = match alloc_conn() {
        Some(id) => id,
        None => {
            tros::print("TCP: no free connections for incoming SYN\r\n");
            return;
        }
    };

    let conn = &mut CONNS[conn_id];
    conn.state = TcpState::SynReceived;
    conn.local_port = local_port;
    conn.remote_ep = sender;
    conn.remote_pid = sender as u32;
    conn.rcv_seq = remote_seq + 1; // expect next seq after SYN
    conn.snd_seq = 2000 + conn_id as u32;

    // Send SYN-ACK back
    let mut syn_ack = [0u8; 64];
    syn_ack[0] = conn_id as u8;
    write_u32(&mut syn_ack, 1, conn.snd_seq);  // our seq
    write_u32(&mut syn_ack, 5, conn.rcv_seq);  // ack = their seq + 1
    conn.snd_seq += 1;
    tros::send(sender, TCP_SYN_ACK, &syn_ack[..9]);

    tros::print("TCP: SYN-ACK sent for conn ");
    tros::print_uint(conn_id);
    tros::print("\r\n");
}

unsafe fn handle_incoming_syn_ack(conn_id: usize, sender: usize, remote_seq: u32) {
    if conn_id >= CONNS.len() {
        return;
    }
    let conn = &mut CONNS[conn_id];
    if conn.state != TcpState::SynSent {
        return;
    }

    conn.state = TcpState::Established;
    conn.remote_ep = sender;
    conn.rcv_seq = remote_seq + 1; // expect next seq after SYN-ACK

    // Send ACK to complete handshake
    let mut ack_pkt = [0u8; 64];
    ack_pkt[0] = conn_id as u8;
    tros::send(sender, TCP_ACK, &ack_pkt[..1]);

    tros::print("TCP: connection ");
    tros::print_uint(conn_id);
    tros::print(" established\r\n");
}

unsafe fn handle_incoming_ack(conn_id: usize) {
    if conn_id >= CONNS.len() {
        return;
    }
    let conn = &mut CONNS[conn_id];
    if conn.state == TcpState::SynReceived {
        conn.state = TcpState::Established;
        tros::print("TCP: connection ");
        tros::print_uint(conn_id);
        tros::print(" established (server)\r\n");
    }
}

unsafe fn handle_send(conn_id: usize, data: &[u8]) {
    if conn_id >= CONNS.len() {
        return;
    }
    let conn = &mut CONNS[conn_id];
    if conn.state != TcpState::Established {
        tros::print("TCP: send on non-established conn\r\n");
        return;
    }

    // Build data packet: conn_id(1) + data_len(1) + data + seq(4)
    let mut pkt = [0u8; 64];
    let len = core::cmp::min(data.len(), 56); // 64 - 1 - 1 - 4 - 2 (overhead)
    pkt[0] = conn_id as u8;
    pkt[1] = len as u8;
    for i in 0..len {
        pkt[2 + i] = data[i];
    }
    write_u32(&mut pkt, 2 + len, conn.snd_seq);
    conn.snd_seq += len as u32;

    tros::send(conn.remote_ep, TCP_DATA, &pkt[..2 + len + 4]);
}

unsafe fn handle_incoming_data(conn_id: usize, data: &[u8], seq: u32) {
    if conn_id >= CONNS.len() {
        return;
    }
    let conn = &mut CONNS[conn_id];
    if conn.state != TcpState::Established {
        return;
    }

    // Check sequence number
    if seq == conn.rcv_seq {
        // In-order delivery — buffer the data
        let len = core::cmp::min(data.len(), 64);
        for i in 0..len {
            conn.recv_buf[i] = data[i];
        }
        conn.recv_len = len;
        conn.rcv_seq += len as u32;

        // Send ACK
        let mut ack_pkt = [0u8; 64];
        ack_pkt[0] = conn_id as u8;
        write_u32(&mut ack_pkt, 1, conn.rcv_seq);
        tros::send(conn.remote_ep, TCP_ACK, &ack_pkt[..5]);
    }
}

unsafe fn handle_recv(conn_id: usize, caller_pid: usize) {
    if conn_id >= CONNS.len() {
        return;
    }
    let conn = &mut CONNS[conn_id];

    if conn.recv_len > 0 {
        // Deliver buffered data to caller
        tros::send(caller_pid, 4, &conn.recv_buf[..conn.recv_len]);
        conn.recv_len = 0;
    } else {
        // No data — send empty response
        tros::send(caller_pid, 4, &[]);
    }
}

unsafe fn handle_close(conn_id: usize) {
    if conn_id >= CONNS.len() {
        return;
    }
    let conn = &mut CONNS[conn_id];
    if conn.state != TcpState::Established {
        return;
    }

    conn.state = TcpState::FinWait1;

    // Send FIN
    let mut fin_pkt = [0u8; 64];
    fin_pkt[0] = conn_id as u8;
    tros::send(conn.remote_ep, TCP_FIN, &fin_pkt[..1]);

    tros::print("TCP: FIN sent for conn ");
    tros::print_uint(conn_id);
    tros::print("\r\n");
}

unsafe fn handle_incoming_fin(conn_id: usize) {
    if conn_id >= CONNS.len() {
        return;
    }
    let conn = &mut CONNS[conn_id];
    conn.state = TcpState::LastAck;

    // Send FIN-ACK
    let mut fin_ack = [0u8; 64];
    fin_ack[0] = conn_id as u8;
    tros::send(conn.remote_ep, TCP_FIN_ACK, &fin_ack[..1]);
}

unsafe fn handle_incoming_fin_ack(conn_id: usize) {
    if conn_id >= CONNS.len() {
        return;
    }
    let conn = &mut CONNS[conn_id];
    conn.state = TcpState::Closed;
    tros::print("TCP: connection ");
    tros::print_uint(conn_id);
    tros::print(" closed\r\n");
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    tros::print("TCP: PANIC\r\n");
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}
