//! DNS Protocol
//!
//! Domain Name System client

use super::*;

/// DNS header
#[repr(C)]
pub struct DnsHeader {
    /// Transaction ID
    pub id: [u8; 2],
    /// Flags
    pub flags: [u8; 2],
    /// Number of questions
    pub qdcount: [u8; 2],
    /// Number of answers
    pub ancount: [u8; 2],
    /// Number of authority records
    pub nscount: [u8; 2],
    /// Number of additional records
    pub arcount: [u8; 2],
}

impl DnsHeader {
    pub fn id_u16(&self) -> u16 {
        ((self.id[0] as u16) << 8) | (self.id[1] as u16)
    }

    pub fn qdcount_u16(&self) -> u16 {
        ((self.qdcount[0] as u16) << 8) | (self.qdcount[1] as u16)
    }
}

/// DNS record types
pub const DNS_TYPE_A: u16 = 1;
pub const DNS_TYPE_NS: u16 = 2;
pub const DNS_TYPE_CNAME: u16 = 5;
pub const DNS_TYPE_SOA: u16 = 6;
pub const DNS_TYPE_PTR: u16 = 12;
pub const DNS_TYPE_AAAA: u16 = 28;
pub const DNS_TYPE_SRV: u16 = 33;
pub const DNS_TYPE_ANY: u16 = 255;

/// DNS result codes
pub const DNS_RCODE_OK: u8 = 0;
pub const DNS_RCODE_FORMAT_ERROR: u8 = 1;
pub const DNS_RCODE_SERVER_FAILURE: u8 = 2;
pub const DNS_RCODE_NAME_ERROR: u8 = 3;
pub const DNS_RCODE_NOT_IMPLEMENTED: u8 = 4;
pub const DNS_RCODE_REFUSED: u8 = 5;

/// DNS client
pub struct DnsClient {
    /// DNS server IP
    server: IpAddr,
    /// Transaction ID counter
    tx_id: u16,
}

impl DnsClient {
    pub fn new(server: IpAddr) -> Self {
        Self {
            server,
            tx_id: 0,
        }
    }

    /// Create a DNS query for an A record
    pub fn query(&mut self, _name: &str) -> Option<DnsQuery> {
        self.tx_id = self.tx_id.wrapping_add(1);

        let mut query = DnsQuery {
            id: self.tx_id,
            qtype: DNS_TYPE_A,
            data: [0u8; 512],
            len: 0,
        };

        // Build query
        let data = &mut query.data[..];

        // Header
        data[0] = (query.id >> 8) as u8;
        data[1] = query.id as u8;
        data[2] = 0x01;  // Flags: standard query
        data[3] = 0x00;
        data[4] = 0x00;  // QDCOUNT = 1
        data[5] = 0x01;
        data[6] = 0x00;  // ANCOUNT = 0
        data[7] = 0x00;
        data[8] = 0x00;  // NSCOUNT = 0
        data[9] = 0x00;
        data[10] = 0x00; // ARCOUNT = 0
        data[11] = 0x00;

        query.len = 12;
        Some(query)
    }
}

/// DNS query structure
pub struct DnsQuery {
    /// Transaction ID
    pub id: u16,
    /// Query type
    pub qtype: u16,
    /// Raw query data
    pub data: [u8; 512],
    /// Query length
    pub len: usize,
}

/// Parse DNS response - returns first IP address found
pub fn parse_response(data: &[u8], result_ip: &mut IpAddr) -> bool {
    if data.len() < 12 {
        return false;
    }

    let header = unsafe { &*(data.as_ptr() as *const DnsHeader) };

    // Check response code
    let rcode = header.flags[1] & 0x0F;
    if rcode != DNS_RCODE_OK {
        crate::println!("[dns] Response error");
        return false;
    }

    // Parse answers
    let qdcount = header.qdcount_u16() as usize;
    let ancount = ((header.ancount[0] as usize) << 8) | (header.ancount[1] as usize);

    let mut offset = 12;

    // Skip questions
    for _ in 0..qdcount {
        // Skip name
        while offset < data.len() && data[offset] != 0 {
            if data[offset] & 0xC0 == 0xC0 {
                offset += 2;
                break;
            }
            offset += data[offset] as usize + 1;
        }
        offset += 1;
        offset += 4;
    }

    // Parse first answer
    for _ in 0..ancount.min(1) {
        if offset >= data.len() {
            break;
        }

        // Skip name
        if data[offset] & 0xC0 == 0xC0 {
            offset += 2;
        } else {
            while offset < data.len() && data[offset] != 0 {
                offset += data[offset] as usize + 1;
            }
            offset += 1;
        }

        if offset + 10 > data.len() {
            break;
        }

        let rtype = ((data[offset] as u16) << 8) | (data[offset + 1] as u16);
        offset += 8;
        let rdlen = ((data[offset] as usize) << 8) | (data[offset + 1] as usize);
        offset += 2;

        if rtype == DNS_TYPE_A && rdlen == 4 {
            *result_ip = IpAddr(
                ((data[offset] as u32) << 24)
                    | ((data[offset + 1] as u32) << 16)
                    | ((data[offset + 2] as u32) << 8)
                    | (data[offset + 3] as u32),
            );
            return true;
        }

        offset += rdlen;
    }

    false
}
