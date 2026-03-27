//! Network socket syscalls
//!
//! Implements Linux-compatible socket API

use spin::{Mutex, MutexGuard};

/// Maximum number of sockets
const MAX_SOCKETS: usize = 64;

/// Socket types
#[derive(Debug, Clone, Copy)]
pub enum SocketType {
    Stream,     // SOCK_STREAM = 1
    Dgram,      // SOCK_DGRAM = 2
    Raw,        // SOCK_RAW = 3
    Unknown,
}

impl SocketType {
    pub fn from_raw(raw: i32) -> Self {
        match raw & 0xFF {
            1 => SocketType::Stream,
            2 => SocketType::Dgram,
            3 => SocketType::Raw,
            _ => SocketType::Unknown,
        }
    }
}

/// Socket protocol
#[derive(Debug, Clone, Copy)]
pub enum SocketProtocol {
    Ip,         // IPPROTO_IP = 0
    Tcp,        // IPPROTO_TCP = 6
    Udp,        // IPPROTO_UDP = 17
    Unknown,
}

impl SocketProtocol {
    pub fn from_raw(raw: i32) -> Self {
        match raw & 0xFF {
            0 => SocketProtocol::Ip,
            6 => SocketProtocol::Tcp,
            17 => SocketProtocol::Udp,
            _ => SocketProtocol::Unknown,
        }
    }
}

/// Socket state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SocketState {
    Free,
    Created,
    Bound,
    Listening,
    Connected,
    Closed,
}

/// Socket structure
#[derive(Debug, Clone, Copy)]
pub struct Socket {
    pub socket_type: SocketType,
    pub protocol: SocketProtocol,
    pub state: SocketState,
    pub local_port: u16,
    pub remote_port: u16,
    pub local_addr: u32,
    pub remote_addr: u32,
}

impl Socket {
    pub fn new(_domain: i32, socket_type: SocketType, protocol: SocketProtocol) -> Self {
        Self {
            socket_type,
            protocol,
            state: SocketState::Created,
            local_port: 0,
            remote_port: 0,
            local_addr: 0,
            remote_addr: 0,
        }
    }
}

/// Global socket table
pub struct SocketTable {
    sockets: [Option<Socket>; MAX_SOCKETS],
}

impl SocketTable {
    pub fn new() -> Self {
        Self {
            sockets: [None; MAX_SOCKETS],
        }
    }

    /// Allocate a new socket
    pub fn alloc(&mut self, socket: Socket) -> Option<usize> {
        for i in 3..MAX_SOCKETS {
            if self.sockets[i].is_none() {
                self.sockets[i] = Some(socket);
                return Some(i);
            }
        }
        None
    }

    /// Get socket by fd
    pub fn get(&self, fd: usize) -> Option<&Socket> {
        if fd < MAX_SOCKETS {
            self.sockets[fd].as_ref()
        } else {
            None
        }
    }

    /// Get mutable socket by fd
    pub fn get_mut(&mut self, fd: usize) -> Option<&mut Socket> {
        if fd < MAX_SOCKETS {
            self.sockets[fd].as_mut()
        } else {
            None
        }
    }

    /// Close a socket
    pub fn close(&mut self, fd: usize) -> bool {
        if fd < MAX_SOCKETS {
            if self.sockets[fd].is_some() {
                self.sockets[fd] = None;
                return true;
            }
        }
        false
    }
}

impl Default for SocketTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Global socket table instance - lazy initialized
static SOCKET_TABLE: Mutex<Option<SocketTable>> = Mutex::new(None);

/// Get or init socket table
fn get_socket_table() -> MutexGuard<'static, Option<SocketTable>> {
    let mut guard = SOCKET_TABLE.lock();
    if guard.is_none() {
        *guard = Some(SocketTable::new());
    }
    guard
}

/// Socket domain (address family)
const AF_INET: i32 = 2;  // IPv4
const AF_UNIX: i32 = 1;
const AF_INET6: i32 = 10;

/// Create a socket
pub fn sys_socket(domain: i32, socket_type: i32, protocol: i32) -> isize {
    let socket = Socket::new(
        domain,
        SocketType::from_raw(socket_type),
        SocketProtocol::from_raw(protocol),
    );

    let mut table = get_socket_table();
    if let Some(ref mut table) = *table {
        match table.alloc(socket) {
            Some(fd) => fd as isize,
            None => -1,
        }
    } else {
        -1
    }
}

/// Bind a socket to an address
pub fn sys_bind(fd: usize, addr: usize, addrlen: usize) -> isize {
    if addr == 0 || addrlen < 8 {
        return -1;
    }

    let mut table = get_socket_table();
    if let Some(ref mut table) = *table {
        if let Some(socket) = table.get_mut(fd) {
            let family = unsafe { *(addr as *const u16) };
            if family != AF_INET as u16 && family != AF_INET6 as u16 {
                return -1;
            }

            let port = unsafe { *((addr + 2) as *const u16) };
            socket.local_port = u16::from_le(port);
            socket.local_addr = unsafe { *((addr + 4) as *const u32) };
            socket.state = SocketState::Bound;
            return 0;
        }
    }
    -1
}

/// Connect a socket
pub fn sys_connect(fd: usize, addr: usize, addrlen: usize) -> isize {
    if addr == 0 || addrlen < 8 {
        return -1;
    }

    let mut table = get_socket_table();
    if let Some(ref mut table) = *table {
        if let Some(socket) = table.get_mut(fd) {
            let family = unsafe { *(addr as *const u16) };
            if family != AF_INET as u16 {
                return -1;
            }

            let port = unsafe { *((addr + 2) as *const u16) };
            socket.remote_port = u16::from_le(port);
            socket.remote_addr = unsafe { *((addr + 4) as *const u32) };

            match socket.state {
                SocketState::Created | SocketState::Bound => {
                    socket.state = SocketState::Connected;
                    0
                }
                _ => -1,
            }
        } else {
            -1
        }
    } else {
        -1
    }
}

/// Listen for connections
pub fn sys_listen(fd: usize, _backlog: i32) -> isize {
    let mut table = get_socket_table();
    if let Some(ref mut table) = *table {
        if let Some(socket) = table.get_mut(fd) {
            if socket.state == SocketState::Bound {
                socket.state = SocketState::Listening;
                return 0;
            }
        }
    }
    -1
}

/// Accept a connection
pub fn sys_accept(fd: usize, _addr: usize, _addrlen: usize) -> isize {
    let mut table = get_socket_table();
    if let Some(ref mut table) = *table {
        // First check if the socket is listening
        let socket_type;
        let protocol;
        {
            if let Some(socket) = table.get(fd) {
                if socket.state == SocketState::Listening {
                    socket_type = socket.socket_type;
                    protocol = socket.protocol;
                } else {
                    return -1;
                }
            } else {
                return -1;
            }
        }

        // Now allocate a new socket
        let new_socket = Socket::new(AF_INET as i32, socket_type, protocol);
        match table.alloc(new_socket) {
            Some(new_fd) => new_fd as isize,
            None => -1,
        }
    } else {
        -1
    }
}

/// Send data
pub fn sys_sendto(fd: usize, buf: usize, len: usize, _flags: usize, _dest_addr: usize, _addrlen: usize) -> isize {
    if buf == 0 || len == 0 {
        return -1;
    }

    let table = get_socket_table();
    if let Some(ref table) = *table {
        if let Some(socket) = table.get(fd) {
            if socket.state == SocketState::Connected || socket.state == SocketState::Listening {
                return len as isize;
            }
        }
    }
    -1
}

/// Receive data
pub fn sys_recvfrom(fd: usize, buf: usize, len: usize, _flags: usize, _src_addr: usize, _addrlen: usize) -> isize {
    if buf == 0 || len == 0 {
        return -1;
    }

    let table = get_socket_table();
    if let Some(ref table) = *table {
        if let Some(_socket) = table.get(fd) {
            0  // No data available yet
        } else {
            -1
        }
    } else {
        -1
    }
}

/// Close a socket
pub fn sys_shutdown(fd: usize, _how: i32) -> isize {
    let mut table = get_socket_table();
    if let Some(ref mut table) = *table {
        if let Some(socket) = table.get_mut(fd) {
            socket.state = SocketState::Closed;
            0
        } else {
            -1
        }
    } else {
        -1
    }
}

/// Get socket options
pub fn sys_getsockopt(fd: usize, _level: i32, _optname: i32, _optval: usize, _optlen: usize) -> isize {
    let table = get_socket_table();
    if let Some(ref table) = *table {
        if table.get(fd).is_some() {
            0
        } else {
            -1
        }
    } else {
        -1
    }
}

/// Set socket options
pub fn sys_setsockopt(fd: usize, _level: i32, _optname: i32, _optval: usize, _optlen: usize) -> isize {
    let table = get_socket_table();
    if let Some(ref table) = *table {
        if table.get(fd).is_some() {
            0
        } else {
            -1
        }
    } else {
        -1
    }
}

/// Get socket name
pub fn sys_getsockname(fd: usize, addr: usize, addrlen: usize) -> isize {
    if addr == 0 || addrlen < 8 {
        return -1;
    }

    let table = get_socket_table();
    if let Some(ref table) = *table {
        if let Some(socket) = table.get(fd) {
            unsafe {
                *(addr as *mut u16) = AF_INET as u16;
                *((addr + 2) as *mut u16) = socket.local_port.to_be();
                *((addr + 4) as *mut u32) = socket.local_addr;
            }
            return 0;
        }
    }
    -1
}

/// Get peer name
pub fn sys_getpeername(fd: usize, addr: usize, addrlen: usize) -> isize {
    if addr == 0 || addrlen < 8 {
        return -1;
    }

    let table = get_socket_table();
    if let Some(ref table) = *table {
        if let Some(socket) = table.get(fd) {
            if socket.state == SocketState::Connected {
                unsafe {
                    *(addr as *mut u16) = AF_INET as u16;
                    *((addr + 2) as *mut u16) = socket.remote_port.to_be();
                    *((addr + 4) as *mut u32) = socket.remote_addr;
                }
                return 0;
            }
        }
    }
    -1
}

/// Create socket pair
pub fn sys_socketpair(_domain: i32, _socket_type: i32, _protocol: i32, sv: usize) -> isize {
    if sv == 0 {
        return -1;
    }

    let mut table = get_socket_table();
    if let Some(ref mut table) = *table {
        let socket1 = Socket::new(_domain, SocketType::from_raw(_socket_type), SocketProtocol::from_raw(_protocol));
        let socket2 = Socket::new(_domain, SocketType::from_raw(_socket_type), SocketProtocol::from_raw(_protocol));

        match (table.alloc(socket1), table.alloc(socket2)) {
            (Some(fd1), Some(fd2)) => {
                unsafe {
                    *(sv as *mut i32) = fd1 as i32;
                    *((sv + 4) as *mut i32) = fd2 as i32;
                }
                0
            }
            _ => -1,
        }
    } else {
        -1
    }
}
