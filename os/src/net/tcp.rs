//! TCP Protocol
//!
//! Implements TCP streaming protocol

use super::*;

/// TCP header
#[repr(C)]
pub struct TcpHeader {
    /// Source port (big endian)
    pub src_port: [u8; 2],
    /// Destination port (big endian)
    pub dst_port: [u8; 2],
    /// Sequence number (big endian)
    pub seq_num: [u8; 4],
    /// Acknowledgment number (big endian)
    pub ack_num: [u8; 4],
    /// Data offset + reserved + flags
    pub offset_flags: [u8; 2],
    /// Window size
    pub window: [u8; 2],
    /// Checksum
    pub checksum: [u8; 2],
    /// Urgent pointer
    pub urgent: [u8; 2],
}

impl TcpHeader {
    pub fn src_port_u16(&self) -> u16 {
        ((self.src_port[0] as u16) << 8) | (self.src_port[1] as u16)
    }

    pub fn dst_port_u16(&self) -> u16 {
        ((self.dst_port[0] as u16) << 8) | (self.dst_port[1] as u16)
    }

    pub fn seq_num_u32(&self) -> u32 {
        ((self.seq_num[0] as u32) << 24)
            | ((self.seq_num[1] as u32) << 16)
            | ((self.seq_num[2] as u32) << 8)
            | (self.seq_num[3] as u32)
    }

    pub fn ack_num_u32(&self) -> u32 {
        ((self.ack_num[0] as u32) << 24)
            | ((self.ack_num[1] as u32) << 16)
            | ((self.ack_num[2] as u32) << 8)
            | (self.ack_num[3] as u32)
    }

    pub fn data_offset(&self) -> usize {
        ((self.offset_flags[0] >> 4) as usize) * 4
    }

    pub fn flags(&self) -> u8 {
        self.offset_flags[1]
    }

    pub fn checksum_u16(&self) -> u16 {
        ((self.checksum[0] as u16) << 8) | (self.checksum[1] as u16)
    }
}

/// TCP flags
pub const TCP_FIN: u8 = 1;
pub const TCP_SYN: u8 = 2;
pub const TCP_RST: u8 = 4;
pub const TCP_PSH: u8 = 8;
pub const TCP_ACK: u8 = 16;
pub const TCP_URG: u8 = 32;

/// TCP connection state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

/// TCP connection
#[derive(Debug, Clone, Copy)]
pub struct TcpConnection {
    /// Local port
    pub local_port: u16,
    /// Remote port
    pub remote_port: u16,
    /// Local IP
    pub local_ip: IpAddr,
    /// Remote IP
    pub remote_ip: IpAddr,
    /// Connection state
    pub state: TcpState,
    /// Sequence numbers
    pub snd_nxt: u32,
    pub rcv_nxt: u32,
    /// Remote sequence number (for ACK)
    pub remote_seq: u32,
}

impl TcpConnection {
    pub fn new(local_port: u16, local_ip: IpAddr, remote_port: u16, remote_ip: IpAddr) -> Self {
        Self {
            local_port,
            local_ip,
            remote_port,
            remote_ip,
            state: TcpState::Closed,
            snd_nxt: 0,
            rcv_nxt: 0,
            remote_seq: 0,
        }
    }

    pub fn is_listening(&self) -> bool {
        self.state == TcpState::Listen
    }

    pub fn is_established(&self) -> bool {
        self.state == TcpState::Established
    }
}

/// TCP pseudo-header for checksum
#[repr(C)]
struct PseudoHeader {
    src_ip: [u8; 4],
    dst_ip: [u8; 4],
    zero: u8,
    proto: u8,
    length: [u8; 2],
}

impl PseudoHeader {
    fn new(src: IpAddr, dst: IpAddr, proto: u8, len: u16) -> Self {
        Self {
            src_ip: src.octets(),
            dst_ip: dst.octets(),
            zero: 0,
            proto,
            length: [(len >> 8) as u8, len as u8],
        }
    }
}

/// Process incoming TCP packet
pub fn tcp_input(
    buffer: &mut NetBuffer,
    _src_ip: IpAddr,
    _dst_ip: IpAddr,
) -> bool {
    let ip_len = 14 + 20; // Ethernet + IPv4
    if buffer.len < ip_len + 20 {
        return false;
    }

    let tcp_data = &buffer.data[ip_len..];
    let header = unsafe { &*(tcp_data.as_ptr() as *const TcpHeader) };

    let _src_port = header.src_port_u16();
    let dst_port = header.dst_port_u16();
    let _seq = header.seq_num_u32();
    let _ack = header.ack_num_u32();
    let flags = header.flags();
    let _data_len = buffer.len - ip_len - header.data_offset();

    crate::println!("[tcp] packet received");

    // Handle based on flags
    if flags & TCP_SYN != 0 {
        // SYN - new connection
        if dst_port == 80 || dst_port == 443 {
            // HTTP/HTTPS - accept
            crate::println!("[tcp] SYN received, would accept connection");
        }
    }

    if flags & TCP_ACK != 0 {
        // ACK - data acknowledged
    }

    if flags & TCP_FIN != 0 {
        // FIN - close connection
        crate::println!("[tcp] FIN received");
    }

    true
}

/// Build TCP header
pub fn tcp_output(
    buffer: &mut NetBuffer,
    src_ip: IpAddr,
    dst_ip: IpAddr,
    src_port: u16,
    dst_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    payload_len: usize,
) -> Result<(), &'static str> {
    let tcp_len = 20 + payload_len;
    let total_len = 14 + 20 + tcp_len; // Eth + IP + TCP

    if total_len > buffer.data.len() {
        return Err("Buffer too small");
    }

    // Shift data to make room
    let data_start = 14 + 20; // After Ethernet + IPv4
    if payload_len > 0 {
        buffer.data.copy_within(data_start..data_start + payload_len, data_start + 20);
    }

    // Write TCP header
    let tcp_header = unsafe { &mut *(buffer.data[14 + 20..].as_ptr() as *mut TcpHeader) };

    tcp_header.src_port = [(src_port >> 8) as u8, src_port as u8];
    tcp_header.dst_port = [(dst_port >> 8) as u8, dst_port as u8];
    tcp_header.seq_num = [
        (seq >> 24) as u8,
        (seq >> 16) as u8,
        (seq >> 8) as u8,
        seq as u8,
    ];
    tcp_header.ack_num = [
        (ack >> 24) as u8,
        (ack >> 16) as u8,
        (ack >> 8) as u8,
        ack as u8,
    ];
    tcp_header.offset_flags = [0x50, flags];  // Data offset 5 (20 bytes), flags
    tcp_header.window = [0xFF, 0xFF];  // Max window
    tcp_header.urgent = [0, 0];

    // Compute TCP checksum with pseudo-header
    let pseudo = PseudoHeader::new(src_ip, dst_ip, IP_PROTO_TCP, tcp_len as u16);
    let mut sum_data = [0u8; 12 + 65535];
    sum_data[..12].copy_from_slice(unsafe {
        core::slice::from_raw_parts(&pseudo as *const _ as *const u8, 12)
    });
    sum_data[12..12 + tcp_len].copy_from_slice(&buffer.data[14 + 20..14 + 20 + tcp_len]);

    let csum = checksum(&sum_data[..12 + tcp_len], 0);
    tcp_header.checksum = [(csum >> 8) as u8, csum as u8];

    // Update buffer length
    buffer.len = total_len;

    Ok(())
}
