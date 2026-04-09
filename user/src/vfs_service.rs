//! VFS Service - Virtual Filesystem Service
//!
//! Provides procfs and sysfs virtual filesystems

#![no_std]
#![no_main]

mod procfs;
mod sysfs;

mod driver_mmio;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}

/// Write character to console
fn putchar(c: u8) {
    unsafe {
        core::arch::asm!("li a7, 1; mv a0, {0}; ecall", in(reg) c);
    }
}

/// Print string
fn print(s: &str) {
    for b in s.bytes() {
        putchar(b);
        if b == b'\n' {
            putchar(b'\r');
        }
    }
}

/// Print hex number
fn print_hex(val: usize) {
    let hex = b"0123456789abcdef";
    for i in (0..16).rev() {
        putchar(hex[(val >> (i * 4)) & 0xf as usize]);
    }
}

/// Make a syscall
fn syscall(n: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize) -> isize {
    let ret;
    unsafe {
        core::arch::asm!(
            "mv a7, {syscall_num}",
            "mv a0, {arg0}; mv a1, {arg1}; mv a2, {arg2}; mv a3, {arg3}; mv a4, {arg4}; mv a5, {arg5}",
            "ecall",
            lateout("a0") ret,
            arg0 = in(reg) a0,
            arg1 = in(reg) a1,
            arg2 = in(reg) a2,
            arg3 = in(reg) a3,
            arg4 = in(reg) a4,
            arg5 = in(reg) a5,
            syscall_num = in(reg) n,
        );
    }
    ret
}

// Syscall numbers
const SYS_ENDPOINT_CREATE: usize = 1000;
const SYS_SEND: usize = 1002;
const SYS_RECV: usize = 1003;
const SYS_SCHED_YIELD: usize = 124;

/// VFS port
const VFS_PORT: u32 = 5;

/// VFS operations
const VFS_OP_READ: u32 = 0;
const VFS_OP_WRITE: u32 = 1;
const VFS_OP_LOOKUP: u32 = 2;

/// Handle procfs lookup
fn handle_procfs_lookup(path: &str) -> Option<procfs::ProcfsEntry> {
    match path {
        "/" => Some(procfs::ProcfsEntry::new_dir("proc")),
        "/self" => Some(procfs::ProcfsEntry::new_dir("self")),
        "/cmdline" => Some(procfs::ProcfsEntry::new_file("cmdline", 256)),
        "/meminfo" => Some(procfs::ProcfsEntry::new_file("meminfo", 256)),
        "/cpuinfo" => Some(procfs::ProcfsEntry::new_file("cpuinfo", 256)),
        "/version" => Some(procfs::ProcfsEntry::new_file("version", 64)),
        _ => None,
    }
}

/// Handle sysfs lookup
fn handle_sysfs_lookup(path: &str) -> Option<sysfs::SysfsEntry> {
    match path {
        "/" => Some(sysfs::SysfsEntry::new_dir("sys")),
        "/class" => Some(sysfs::SysfsEntry::new_dir("class")),
        "/class/net" => Some(sysfs::SysfsEntry::new_dir("net")),
        "/class/net/eth0" => Some(sysfs::SysfsEntry::new_dir("eth0")),
        "/class/net/eth0/address" => Some(sysfs::SysfsEntry::new_file("address")),
        "/class/net/eth0/mtu" => Some(sysfs::SysfsEntry::new_file("mtu")),
        "/class/net/eth0/flags" => Some(sysfs::SysfsEntry::new_file("flags")),
        "/devices" => Some(sysfs::SysfsEntry::new_dir("devices")),
        _ => None,
    }
}

/// Read from procfs
fn read_procfs(path: &str, buf: &mut [u8]) -> usize {
    match path {
        "/cmdline" => procfs::read_cmdline(0, buf),
        "/meminfo" => procfs::read_meminfo(buf),
        "/cpuinfo" => procfs::read_cpuinfo(buf),
        "/version" => procfs::read_version(buf),
        _ => 0,
    }
}

/// Read from sysfs
fn read_sysfs(path: &str, buf: &mut [u8]) -> usize {
    if path.starts_with("/class/net/eth0/") {
        let file = &path[17..];
        match file {
            "address" => sysfs::read_net_address(buf, "eth0"),
            "mtu" => sysfs::read_net_mtu(buf, "eth0"),
            "flags" => sysfs::read_net_flags(buf, "eth0"),
            _ => 0,
        }
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn _start() {
    print("vfs: VFS service starting\n");

    // Create endpoint for VFS service
    let vfs_port = syscall(SYS_ENDPOINT_CREATE, 0, 0, 0, 0, 0, 0) as u32;
    if vfs_port < 2 {
        print("vfs: Failed to create endpoint\n");
        loop {
            unsafe { core::arch::asm!("wfi"); }
        }
    }

    print("vfs: Listening on port ");
    print_hex(vfs_port as usize);
    print("\n");

    // Buffer for requests
    let mut req_buf: [u8; 512] = [0; 512];
    let mut resp_buf: [u8; 512] = [0; 512];

    loop {
        // Receive request
        let size = syscall(SYS_RECV, vfs_port as usize, req_buf.as_mut_ptr() as usize, 512, 0, 0, 0) as usize;

        if size > 20 {
            // Parse request
            let from: u32 = unsafe { *(req_buf.as_ptr() as *const u32) };
            let reply_port: u32 = unsafe { *(req_buf.as_ptr().add(16) as *const u32) };
            let payload = 20usize;

            let op: u32 = unsafe { *(req_buf.as_ptr().add(payload) as *const u32) };

            if op == VFS_OP_LOOKUP {
                // Lookup path
                let path_len = size - payload - 4;
                let path_buf = &req_buf[payload + 4..payload + 4 + path_len.min(256)];
                let path_str = core::str::from_utf8(path_buf).unwrap_or("");

                // Check procfs first
                if let Some(entry) = handle_procfs_lookup(path_str) {
                    unsafe { *(resp_buf.as_mut_ptr() as *mut u32) = 0; } // OK
                    resp_buf[4..].copy_from_slice(unsafe {
                        core::slice::from_raw_parts(
                            &entry as *const procfs::ProcfsEntry as *const u8,
                            core::mem::size_of::<procfs::ProcfsEntry>(),
                        )
                    });
                    let resp_size = 4 + core::mem::size_of::<procfs::ProcfsEntry>();

                    if reply_port > 0 {
                        syscall(SYS_SEND, from as usize, reply_port as usize,
                               resp_buf.as_ptr() as usize, resp_size, 0, 0);
                    }
                } else if let Some(_entry) = handle_sysfs_lookup(path_str) {
                    // For now, just acknowledge lookup success
                    unsafe { *(resp_buf.as_mut_ptr() as *mut u32) = 0; } // OK
                    unsafe { *(resp_buf.as_mut_ptr().add(4) as *mut u32) = 0; } // Not found indicator

                    if reply_port > 0 {
                        syscall(SYS_SEND, from as usize, reply_port as usize,
                               resp_buf.as_ptr() as usize, 8, 0, 0);
                    }
                } else {
                    // Not found
                    unsafe { *(resp_buf.as_mut_ptr() as *mut u32) = 1; } // ERR
                    if reply_port > 0 {
                        syscall(SYS_SEND, from as usize, reply_port as usize,
                               resp_buf.as_ptr() as usize, 4, 0, 0);
                    }
                }
            } else if op == VFS_OP_READ {
                // Read file content
                let path_len = unsafe { *(req_buf.as_ptr().add(payload + 4) as *const u32) as usize };
                let path_start = payload + 8;
                let path_buf = &req_buf[path_start..path_start + path_len.min(256)];

                // Determine filesystem based on path
                let result = if path_buf.starts_with(b"/proc") || path_buf.starts_with(b"/self") {
                    read_procfs(core::str::from_utf8(path_buf).unwrap_or(""), &mut resp_buf[4..])
                } else if path_buf.starts_with(b"/sys") {
                    read_sysfs(core::str::from_utf8(path_buf).unwrap_or(""), &mut resp_buf[4..])
                } else {
                    0
                };

                unsafe { *(resp_buf.as_mut_ptr() as *mut u32) = 0; } // OK
                unsafe { *(resp_buf.as_mut_ptr().add(4) as *mut u32) = result as u32; }

                if reply_port > 0 {
                    syscall(SYS_SEND, from as usize, reply_port as usize,
                           resp_buf.as_ptr() as usize, 8 + result, 0, 0);
                }
            }
        }

        syscall(SYS_SCHED_YIELD, 0, 0, 0, 0, 0, 0);
    }
}