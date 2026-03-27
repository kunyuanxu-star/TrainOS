//! TCP/IP Network Stack
//!
//! Implements basic networking protocols

pub mod eth;
pub mod ipv4;
pub mod tcp;
pub mod udp;
pub mod arp;
pub mod dns;

use spin::Mutex;

/// Maximum packet size
pub const MAX_PACKET_SIZE: usize = 65535;

/// Ethernet frame types
pub const ETH_TYPE_IPV4: u16 = 0x0800;
pub const ETH_TYPE_IPV6: u16 = 0x86DD;
pub const ETH_TYPE_ARP: u16 = 0x0806;

/// IP protocols
pub const IP_PROTO_ICMP: u8 = 1;
pub const IP_PROTO_TCP: u8 = 6;
pub const IP_PROTO_UDP: u8 = 17;

/// Port numbers
pub const PORT_ANY: u16 = 0;

/// Well-known ports
pub const PORT_ECHO: u16 = 7;
pub const PORT_DISCARD: u16 = 9;
pub const PORT_FTP: u16 = 21;
pub const PORT_SSH: u16 = 22;
pub const PORT_TELNET: u16 = 23;
pub const PORT_SMTP: u16 = 25;
pub const PORT_DNS: u16 = 53;
pub const PORT_HTTP: u16 = 80;
pub const PORT_HTTPS: u16 = 443;

/// Network buffer
#[derive(Debug)]
pub struct NetBuffer {
    /// Data buffer
    pub data: [u8; MAX_PACKET_SIZE],
    /// Length of valid data
    pub len: usize,
    /// Offset to payload start
    pub offset: usize,
}

impl NetBuffer {
    pub fn new() -> Self {
        Self {
            data: [0; MAX_PACKET_SIZE],
            len: 0,
            offset: 0,
        }
    }

    /// Get header as a slice
    pub fn header(&self) -> &[u8] {
        &self.data[self.offset..self.len]
    }

    /// Get header as mutable slice
    pub fn header_mut(&mut self) -> &mut [u8] {
        &mut self.data[self.offset..self.len]
    }

    /// Get payload
    pub fn payload(&self) -> &[u8] {
        &self.data[self.len..]
    }

    /// Push header to front
    pub fn push_header(&mut self, header_len: usize) {
        self.offset -= header_len;
        self.len += header_len;
    }

    /// Pull header from front
    pub fn pull_header(&mut self, header_len: usize) -> &[u8] {
        let result = &self.data[self.offset..self.offset + header_len];
        self.offset += header_len;
        result
    }
}

impl Default for NetBuffer {
    fn default() -> Self {
        Self::new()
    }
}

/// MAC address
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacAddr(pub [u8; 6]);

impl MacAddr {
    pub const BROADCAST: MacAddr = MacAddr([0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);

    pub fn new(a: u8, b: u8, c: u8, d: u8, e: u8, f: u8) -> Self {
        Self([a, b, c, d, e, f])
    }

    pub fn is_broadcast(&self) -> bool {
        self.0 == Self::BROADCAST.0
    }

    pub fn is_multicast(&self) -> bool {
        self.0[0] & 0x01 != 0
    }

    pub fn to_u64(&self) -> u64 {
        ((self.0[0] as u64) << 40)
            | ((self.0[1] as u64) << 32)
            | ((self.0[2] as u64) << 24)
            | ((self.0[3] as u64) << 16)
            | ((self.0[4] as u64) << 8)
            | (self.0[5] as u64)
    }
}

impl Default for MacAddr {
    fn default() -> Self {
        Self([0; 6])
    }
}

/// IPv4 address
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IpAddr(pub u32);

impl IpAddr {
    pub const ANY: IpAddr = IpAddr(0);
    pub const LOOPBACK: IpAddr = IpAddr(0x7F_00_00_01);
    pub const BROADCAST: IpAddr = IpAddr(0xFF_FF_FF_FF);

    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self(((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8) | (d as u32))
    }

    pub fn octets(&self) -> [u8; 4] {
        [
            (self.0 >> 24) as u8,
            (self.0 >> 16) as u8,
            (self.0 >> 8) as u8,
            self.0 as u8,
        ]
    }

    pub fn is_loopback(&self) -> bool {
        (self.0 & 0xFF_00_00_00) == 0x7F_00_00_00
    }

    pub fn is_private(&self) -> bool {
        // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
        (self.0 & 0xFF_00_00_00) == 0x0A_00_00_00
            || (self.0 & 0xFF_F0_00_00) == 0xAC_10_00_00
            || (self.0 & 0xFF_FF_00_00) == 0xC0_A8_00_00
    }
}

impl Default for IpAddr {
    fn default() -> Self {
        Self::ANY
    }
}

impl core::fmt::Display for IpAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let octets = self.octets();
        write!(f, "{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3])
    }
}

/// Socket address for IP
#[repr(C)]
pub struct SockAddrIn {
    pub family: u16,     // AF_INET = 2
    pub port: u16,        // Port number (network order)
    pub addr: u32,        // IPv4 address (network order)
    pub zero: [u8; 8],   // Padding
}

impl SockAddrIn {
    pub fn new(port: u16, addr: IpAddr) -> Self {
        Self {
            family: 2,  // AF_INET
            port: port.to_be(),
            addr: addr.0,
            zero: [0; 8],
        }
    }

    pub fn ip(&self) -> IpAddr {
        IpAddr(self.addr)
    }

    pub fn port_be(&self) -> u16 {
        u16::from_be(self.port)
    }
}

/// Network interface
pub struct NetInterface {
    /// Interface name
    pub name: [u8; 16],
    /// MAC address
    pub mac: MacAddr,
    /// IPv4 address
    pub ip: IpAddr,
    /// Subnet mask
    pub mask: IpAddr,
    /// Gateway
    pub gateway: IpAddr,
    /// MTU
    pub mtu: u16,
    /// Is up
    pub up: bool,
}

impl Default for NetInterface {
    fn default() -> Self {
        Self {
            name: [0; 16],
            mac: MacAddr::default(),
            ip: IpAddr::ANY,
            mask: IpAddr::new(255, 255, 255, 0),
            gateway: IpAddr::ANY,
            mtu: 1500,
            up: false,
        }
    }
}

impl NetInterface {
    /// Check if an IP is on the local network
    pub fn is_local(&self, ip: IpAddr) -> bool {
        (ip.0 & self.mask.0) == (self.ip.0 & self.mask.0)
    }
}

/// Global network interface
static NET_INTERFACE: Mutex<Option<NetInterface>> = Mutex::new(None);

/// Initialize network with default settings
pub fn init() {
    let mut iface = NET_INTERFACE.lock();
    if iface.is_none() {
        *iface = Some(NetInterface {
            name: [b'e', b't', b'h', b'0', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            mac: MacAddr::new(0x52, 0x54, 0x00, 0x12, 0x34, 0x56),
            ip: IpAddr::new(10, 0, 2, 15),
            mask: IpAddr::new(255, 255, 255, 0),
            gateway: IpAddr::new(10, 0, 2, 1),
            mtu: 1500,
            up: true,
        });
    }
    crate::println!("[net] Network initialized");
}

/// Get network interface
pub fn get_interface() -> spin::MutexGuard<'static, Option<NetInterface>> {
    NET_INTERFACE.lock()
}

/// Compute checksum
pub fn checksum(data: &[u8], initial: u32) -> u16 {
    let mut sum = initial;
    let mut i = 0;

    // Sum all 16-bit words
    while i + 1 < data.len() {
        sum += ((data[i] as u32) << 8) | (data[i + 1] as u32);
        i += 2;
    }

    // Handle odd byte
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }

    // Fold 32-bit sum to 16 bits
    while sum > 0xFFFF {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }

    !(sum as u16)
}

/// Verify checksum
pub fn verify_checksum(data: &[u8]) -> bool {
    checksum(data, 0) == 0
}
