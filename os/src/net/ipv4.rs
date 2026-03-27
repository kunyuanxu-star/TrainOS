//! IPv4 Protocol
//!
//! Implements IPv4 packet handling

use super::*;

/// IPv4 header
#[repr(C)]
pub struct IpHeader {
    /// Version (4) and IHL (5) = 0x45
    pub ver_ihl: u8,
    /// DSCP + ECN
    pub tos: u8,
    /// Total length (big endian)
    pub total_len: [u8; 2],
    /// Identification
    pub ident: [u8; 2],
    /// Flags + Fragment offset
    pub flags_frag: [u8; 2],
    /// Time to live
    pub ttl: u8,
    /// Protocol
    pub proto: u8,
    /// Header checksum
    pub checksum: [u8; 2],
    /// Source IP address
    pub src_ip: [u8; 4],
    /// Destination IP address
    pub dst_ip: [u8; 4],
}

impl IpHeader {
    pub fn version(&self) -> u8 {
        self.ver_ihl >> 4
    }

    pub fn ihl(&self) -> u8 {
        self.ver_ihl & 0x0F
    }

    pub fn header_len(&self) -> usize {
        (self.ihl() * 4) as usize
    }

    pub fn total_len_u16(&self) -> u16 {
        ((self.total_len[0] as u16) << 8) | (self.total_len[1] as u16)
    }

    pub fn set_total_len(&mut self, len: u16) {
        self.total_len[0] = (len >> 8) as u8;
        self.total_len[1] = len as u8;
    }

    pub fn proto(&self) -> u8 {
        self.proto
    }

    pub fn src_ip(&self) -> IpAddr {
        IpAddr(((self.src_ip[0] as u32) << 24)
            | ((self.src_ip[1] as u32) << 16)
            | ((self.src_ip[2] as u32) << 8)
            | (self.src_ip[3] as u32))
    }

    pub fn dst_ip(&self) -> IpAddr {
        IpAddr(((self.dst_ip[0] as u32) << 24)
            | ((self.dst_ip[1] as u32) << 16)
            | ((self.dst_ip[2] as u32) << 8)
            | (self.dst_ip[3] as u32))
    }

    pub fn set_src_ip(&mut self, ip: IpAddr) {
        let octets = ip.octets();
        self.src_ip = octets;
    }

    pub fn set_dst_ip(&mut self, ip: IpAddr) {
        let octets = ip.octets();
        self.dst_ip = octets;
    }

    pub fn checksum(&self) -> u16 {
        ((self.checksum[0] as u16) << 8) | (self.checksum[1] as u16)
    }

    pub fn set_checksum(&mut self, csum: u16) {
        self.checksum[0] = (csum >> 8) as u8;
        self.checksum[1] = csum as u8;
    }
}

/// IPv4 packet
pub struct IpPacket<'a> {
    pub header: &'a IpHeader,
    pub payload: &'a [u8],
}

impl<'a> IpPacket<'a> {
    /// Parse an IPv4 packet from a buffer
    pub fn parse(buffer: &'a [u8]) -> Option<Self> {
        if buffer.len() < 20 {
            return None;
        }

        let header = unsafe { &*(buffer.as_ptr() as *const IpHeader) };

        // Verify version
        if header.version() != 4 {
            return None;
        }

        let ihl = header.header_len();
        if ihl < 20 || buffer.len() < ihl {
            return None;
        }

        let payload = &buffer[ihl..];

        Some(Self { header, payload })
    }

    /// Get protocol
    pub fn protocol(&self) -> u8 {
        self.header.proto()
    }
}

/// Build an IPv4 packet
pub struct IpPacketBuilder<'a> {
    buffer: &'a mut [u8],
    header: &'a mut IpHeader,
}

impl<'a> IpPacketBuilder<'a> {
    pub fn new(buffer: &'a mut [u8]) -> Option<Self> {
        if buffer.len() < 20 {
            return None;
        }

        let header = unsafe { &mut *(buffer.as_ptr() as *mut IpHeader) };

        Some(Self { buffer, header })
    }

    /// Initialize header fields
    pub fn init(&mut self, total_len: u16) {
        self.header.ver_ihl = 0x45;  // Version 4, IHL 5
        self.header.tos = 0;
        self.header.set_total_len(total_len);
        self.header.ident = [0, 0];
        self.header.flags_frag = [0, 0];
        self.header.ttl = 64;
        self.header.checksum = [0, 0];
    }

    /// Set protocol
    pub fn set_protocol(&mut self, proto: u8) {
        self.header.proto = proto;
    }

    /// Set source IP
    pub fn set_src_ip(&mut self, ip: IpAddr) {
        self.header.set_src_ip(ip);
    }

    /// Set destination IP
    pub fn set_dst_ip(&mut self, ip: IpAddr) {
        self.header.set_dst_ip(ip);
    }

    /// Compute and set checksum
    pub fn set_checksum(&mut self) {
        let header_len = self.header.header_len();
        let header_bytes = &self.buffer[..header_len];

        // Zero checksum field for calculation
        self.header.checksum = [0, 0];

        let csum = checksum(header_bytes, 0);
        self.header.set_checksum(csum);
    }

    /// Get payload slice
    pub fn payload(&mut self) -> &mut [u8] {
        let header_len = self.header.header_len();
        &mut self.buffer[header_len..]
    }

    /// Write to payload
    pub fn write_payload(&mut self, data: &[u8]) -> usize {
        let payload = self.payload();
        let len = data.len().min(payload.len());
        payload[..len].copy_from_slice(&data[..len]);
        len
    }
}

/// Process incoming IPv4 packet
pub fn ipv4_input(buffer: &mut NetBuffer, _iface: &NetInterface) -> bool {
    // Parse the IPv4 header
    let packet = match IpPacket::parse(&buffer.data[14..buffer.len]) {
        Some(p) => p,
        None => return false,
    };

    // Verify checksum
    let header_bytes = &buffer.data[14..14 + packet.header.header_len()];
    if !verify_checksum(header_bytes) {
        crate::println!("[ipv4] Invalid checksum");
        return false;
    }

    // Handle by protocol
    match packet.protocol() {
        IP_PROTO_TCP => {
            crate::println!("[ipv4] TCP packet received");
            true
        }
        IP_PROTO_UDP => {
            crate::println!("[ipv4] UDP packet received");
            true
        }
        IP_PROTO_ICMP => {
            crate::println!("[ipv4] ICMP packet");
            true
        }
        _ => {
            crate::println!("[ipv4] Unknown protocol");
            false
        }
    }
}

/// Send IPv4 packet
pub fn ipv4_output(
    buffer: &mut NetBuffer,
    src_ip: IpAddr,
    dst_ip: IpAddr,
    proto: u8,
    payload_len: usize,
) -> Result<(), &'static str> {
    let total_len = (20 + payload_len) as u16;

    if 14 + 20 + payload_len > buffer.data.len() {
        return Err("Buffer too small");
    }

    // Shift payload
    buffer.data.copy_within(14..14 + payload_len, 14 + 20);

    let mut builder = match IpPacketBuilder::new(&mut buffer.data[14..]) {
        Some(b) => b,
        None => return Err("Failed to create packet"),
    };

    builder.init(total_len);
    builder.set_protocol(proto);
    builder.set_src_ip(src_ip);
    builder.set_dst_ip(dst_ip);
    builder.set_checksum();

    buffer.len = 14 + total_len as usize;

    Ok(())
}
