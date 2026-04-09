//! ARP Protocol
//!
//! Address Resolution Protocol for IPv4

use super::*;

/// ARP operation
pub const ARP_OP_REQUEST: u16 = 1;
pub const ARP_OP_REPLY: u16 = 2;

/// ARP header
#[repr(C)]
pub struct ArpHeader {
    /// Hardware type (1 = Ethernet)
    pub hw_type: [u8; 2],
    /// Protocol type (0x0800 = IPv4)
    pub proto_type: [u8; 2],
    /// Hardware size (6 for Ethernet)
    pub hw_size: u8,
    /// Protocol size (4 for IPv4)
    pub proto_size: u8,
    /// Operation (1 = request, 2 = reply)
    pub op: [u8; 2],
    /// Sender MAC
    pub sender_mac: [u8; 6],
    /// Sender IP
    pub sender_ip: [u8; 4],
    /// Target MAC
    pub target_mac: [u8; 6],
    /// Target IP
    pub target_ip: [u8; 4],
}

impl ArpHeader {
    pub fn hw_type_u16(&self) -> u16 {
        ((self.hw_type[0] as u16) << 8) | (self.hw_type[1] as u16)
    }

    pub fn proto_type_u16(&self) -> u16 {
        ((self.proto_type[0] as u16) << 8) | (self.proto_type[1] as u16)
    }

    pub fn op_u16(&self) -> u16 {
        ((self.op[0] as u16) << 8) | (self.op[1] as u16)
    }

    pub fn sender_ip(&self) -> IpAddr {
        IpAddr(((self.sender_ip[0] as u32) << 24)
            | ((self.sender_ip[1] as u32) << 16)
            | ((self.sender_ip[2] as u32) << 8)
            | (self.sender_ip[3] as u32))
    }

    pub fn target_ip(&self) -> IpAddr {
        IpAddr(((self.target_ip[0] as u32) << 24)
            | ((self.target_ip[1] as u32) << 16)
            | ((self.target_ip[2] as u32) << 8)
            | (self.target_ip[3] as u32))
    }
}

/// ARP cache entry
#[derive(Debug, Clone, Copy)]
pub struct ArpEntry {
    pub ip: IpAddr,
    pub mac: MacAddr,
    pub age: usize,
}

impl Default for ArpEntry {
    fn default() -> Self {
        Self {
            ip: IpAddr::ANY,
            mac: MacAddr::default(),
            age: 0,
        }
    }
}

/// ARP cache
const ARP_CACHE_SIZE: usize = 32;

pub struct ArpCache {
    entries: [Option<ArpEntry>; ARP_CACHE_SIZE],
}

impl ArpCache {
    pub fn new() -> Self {
        Self {
            entries: [None; ARP_CACHE_SIZE],
        }
    }

    /// Lookup MAC address for IP
    pub fn lookup(&self, ip: IpAddr) -> Option<MacAddr> {
        for entry in &self.entries {
            if let Some(e) = entry {
                if e.ip == ip {
                    return Some(e.mac);
                }
            }
        }
        None
    }

    /// Add entry to cache
    pub fn insert(&mut self, ip: IpAddr, mac: MacAddr) {
        // First try to update existing
        for entry in &mut self.entries {
            if let Some(e) = entry {
                if e.ip == ip {
                    e.mac = mac;
                    e.age = 0;
                    return;
                }
            }
        }

        // Otherwise find empty slot
        for entry in &mut self.entries {
            if entry.is_none() {
                *entry = Some(ArpEntry { ip, mac, age: 0 });
                return;
            }
        }

        // Cache full - would need eviction
    }
}

impl Default for ArpCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Process incoming ARP packet
pub fn arp_input(buffer: &mut NetBuffer, iface: &NetInterface) -> bool {
    let header_len = 14; // Ethernet header
    if buffer.len < header_len + 28 {
        return false;
    }

    let arp_data = &buffer.data[header_len..];
    let header = unsafe { &*(arp_data.as_ptr() as *const ArpHeader) };

    // Verify it's Ethernet + IPv4
    if header.hw_type_u16() != 1 || header.proto_type_u16() != ETH_TYPE_IPV4 {
        return false;
    }

    let op = header.op_u16();
    let _sender_ip = header.sender_ip();
    let _sender_mac = MacAddr(header.sender_mac);
    let target_ip = header.target_ip();

    match op {
        ARP_OP_REQUEST => {
            // Someone is asking for our MAC
            if target_ip == iface.ip {
                // Would send ARP reply
                true
            } else {
                true
            }
        }
        ARP_OP_REPLY => {
            // Someone replied with their MAC
            true
        }
        _ => false,
    }
}