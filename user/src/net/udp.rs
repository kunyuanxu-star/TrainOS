//! UDP Protocol
//!
//! Implements UDP datagram protocol

use super::*;

/// UDP header
#[repr(C)]
pub struct UdpHeader {
    /// Source port (big endian)
    pub src_port: [u8; 2],
    /// Destination port (big endian)
    pub dst_port: [u8; 2],
    /// Length (big endian)
    pub length: [u8; 2],
    /// Checksum
    pub checksum: [u8; 2],
}

impl UdpHeader {
    pub fn src_port_u16(&self) -> u16 {
        ((self.src_port[0] as u16) << 8) | (self.src_port[1] as u16)
    }

    pub fn dst_port_u16(&self) -> u16 {
        ((self.dst_port[0] as u16) << 8) | (self.dst_port[1] as u16)
    }

    pub fn length_u16(&self) -> u16 {
        ((self.length[0] as u16) << 8) | (self.length[1] as u16)
    }
}

/// Process incoming UDP packet
pub fn udp_input(
    buffer: &mut NetBuffer,
    _src_ip: IpAddr,
    _dst_ip: IpAddr,
) -> bool {
    let header_len = 14 + 20; // Ethernet + IPv4
    if buffer.len < header_len + 8 {
        return false;
    }

    let udp_data = &buffer.data[header_len..];
    let header = unsafe { &*(udp_data.as_ptr() as *const UdpHeader) };

    let _src_port = header.src_port_u16();
    let dst_port = header.dst_port_u16();
    let _udp_len = header.length_u16() as usize;

    // Handle by port
    match dst_port {
        PORT_DNS => {
            // DNS query
            true
        }
        _ => {
            true
        }
    }
}

/// Build UDP header and payload
pub fn udp_output(
    buffer: &mut NetBuffer,
    _src_ip: IpAddr,
    _dst_ip: IpAddr,
    src_port: u16,
    dst_port: u16,
    payload: &[u8],
) -> Result<usize, &'static str> {
    let udp_len = 8 + payload.len();
    let total_len = 14 + 20 + udp_len; // Eth + IP + UDP

    if total_len > buffer.data.len() {
        return Err("Buffer too small");
    }

    // Shift payload to make room for headers
    let payload_start = 14 + 20 + 8; // After Ethernet + IPv4 + UDP
    if !payload.is_empty() {
        buffer.data.copy_within(14 + 20..14 + 20 + payload.len(), payload_start);
    }

    // Write UDP header
    let udp_header = unsafe { &mut *(buffer.data[14 + 20..].as_ptr() as *mut UdpHeader) };

    udp_header.src_port = [(src_port >> 8) as u8, src_port as u8];
    udp_header.dst_port = [(dst_port >> 8) as u8, dst_port as u8];
    udp_header.length = [(udp_len >> 8) as u8, udp_len as u8];
    udp_header.checksum = [0, 0];  // Optional for IPv4

    // Copy payload
    buffer.data[payload_start..payload_start + payload.len()].copy_from_slice(payload);

    // Update buffer length
    buffer.len = total_len;

    Ok(udp_len)
}