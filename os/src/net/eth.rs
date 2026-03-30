//! Ethernet Protocol
//!
//! Implements Ethernet II framing

use super::*;

/// Ethernet header
#[repr(C)]
pub struct EthHeader {
    /// Destination MAC address
    pub dst: [u8; 6],
    /// Source MAC address
    pub src: [u8; 6],
    /// EtherType (big endian)
    pub ether_type: [u8; 2],
}

impl EthHeader {
    pub fn ether_type_u16(&self) -> u16 {
        ((self.ether_type[0] as u16) << 8) | (self.ether_type[1] as u16)
    }

    pub fn set_ether_type(&mut self, typ: u16) {
        self.ether_type[0] = (typ >> 8) as u8;
        self.ether_type[1] = typ as u8;
    }

    pub fn src_mac(&self) -> MacAddr {
        MacAddr(self.src)
    }

    pub fn dst_mac(&self) -> MacAddr {
        MacAddr(self.dst)
    }
}

/// Ethernet frame
pub struct EthFrame<'a> {
    pub header: &'a EthHeader,
    pub payload: &'a [u8],
}

impl<'a> EthFrame<'a> {
    /// Parse an Ethernet frame from a buffer
    pub fn parse(buffer: &'a [u8]) -> Option<Self> {
        if buffer.len() < 14 {
            return None;
        }

        let (header_bytes, payload) = buffer.split_at(14);
        let header = unsafe { &*(header_bytes.as_ptr() as *const EthHeader) };

        Some(Self { header, payload })
    }

    /// Get the EtherType
    pub fn ether_type(&self) -> u16 {
        self.header.ether_type_u16()
    }
}

/// Build an Ethernet frame
pub struct EthFrameBuilder<'a> {
    buffer: &'a mut [u8],
    offset: usize,
}

impl<'a> EthFrameBuilder<'a> {
    pub fn new(buffer: &'a mut [u8]) -> Self {
        Self { buffer, offset: 14 } // Leave room for header
    }

    /// Set destination MAC
    pub fn set_dst(&mut self, mac: MacAddr) {
        self.buffer[0..6].copy_from_slice(&mac.0);
    }

    /// Set source MAC
    pub fn set_src(&mut self, mac: MacAddr) {
        self.buffer[6..12].copy_from_slice(&mac.0);
    }

    /// Set EtherType
    pub fn set_ether_type(&mut self, typ: u16) {
        self.buffer[12] = (typ >> 8) as u8;
        self.buffer[13] = typ as u8;
    }

    /// Write payload data
    pub fn write_payload(&mut self, data: &[u8]) -> usize {
        let len = data.len().min(self.buffer.len() - self.offset);
        self.buffer[self.offset..self.offset + len].copy_from_slice(&data[..len]);
        self.offset += len;
        len
    }

    /// Get total frame length
    pub fn len(&self) -> usize {
        self.offset
    }

    /// Finalize the frame
    pub fn finalize(self) -> usize {
        self.offset
    }
}

/// Process incoming Ethernet frame
pub fn eth_input(buffer: &mut NetBuffer, _iface: &NetInterface) -> bool {
    // Parse the Ethernet header
    let frame = match EthFrame::parse(&buffer.data[..buffer.len]) {
        Some(f) => f,
        None => return false,
    };

    // Handle by EtherType
    match frame.ether_type() {
        ETH_TYPE_IPV4 => {
            // Pass to IPv4 handler
            crate::println!("[eth] IPv4 packet received");
            true
        }
        ETH_TYPE_ARP => {
            // Pass to ARP handler
            crate::println!("[eth] ARP packet received");
            true
        }
        ETH_TYPE_IPV6 => {
            // IPv6 not implemented yet
            crate::println!("[eth] IPv6 not supported");
            false
        }
        _ => {
            crate::println!("[eth] Unknown EtherType");
            false
        }
    }
}

/// Send Ethernet frame
pub fn eth_output(
    buffer: &mut NetBuffer,
    dst_mac: MacAddr,
    src_mac: MacAddr,
    ether_type: u16,
) -> Result<(), &'static str> {
    if buffer.len + 14 > buffer.data.len() {
        return Err("Buffer too small");
    }

    // Shift payload to make room for header
    buffer.data.copy_within(0..buffer.len, 14);
    buffer.len += 14;

    // Build header
    buffer.data[0..6].copy_from_slice(&dst_mac.0);
    buffer.data[6..12].copy_from_slice(&src_mac.0);
    buffer.data[12] = (ether_type >> 8) as u8;
    buffer.data[13] = ether_type as u8;

    Ok(())
}
