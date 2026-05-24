// V30: /sys filesystem — minimal device model
//
// Implements a minimal /sys filesystem for Linux ABI compatibility:
//   /sys/devices/         — list of registered devices
//   /sys/class/block/     — block devices
//   /sys/class/net/       — network devices
//   /sys/devices/system/cpu/ — CPU information

use alloc::string::String;
use core::fmt::Write;

const SYS_BUF_SIZE: usize = 4096;

/// Fill a buffer with the contents of a /sys file.
/// Returns the number of bytes written, or 0 if the file is not found.
pub fn sysfs_read(path: &[u8], buf: &mut [u8]) -> usize {
    if buf.is_empty() { return 0; }

    // Strip leading '/'
    let p = if path.len() > 0 && path[0] == b'/' { &path[1..] } else { path };
    if p.is_empty() || p == b"/" {
        return sysfs_list_root(buf);
    }

    match p {
        b"devices" => sysfs_devices(buf),
        b"devices/system" => sysfs_system(buf),
        b"devices/system/cpu" => sysfs_cpu(buf),
        b"class" => sysfs_class(buf),
        b"class/block" => sysfs_block(buf),
        b"class/net" => sysfs_net(buf),
        b"class/block/sda" => sysfs_block_device("sda", buf),
        b"class/net/eth0" => sysfs_net_device("eth0", buf),
        _ => 0,
    }
}

fn sysfs_list_root(buf: &mut [u8]) -> usize {
    let entries = b"devices\0class\0";
    let len = entries.len().min(buf.len());
    buf[..len].copy_from_slice(&entries[..len]);
    len
}

fn sysfs_devices(buf: &mut [u8]) -> usize {
    let entries = b"system\0";
    let len = entries.len().min(buf.len());
    buf[..len].copy_from_slice(&entries[..len]);
    len
}

fn sysfs_system(buf: &mut [u8]) -> usize {
    let entries = b"cpu\0";
    let len = entries.len().min(buf.len());
    buf[..len].copy_from_slice(&entries[..len]);
    len
}

fn sysfs_cpu(buf: &mut [u8]) -> usize {
    let cpu_count = crate::per_cpu::hart_count();
    let mut s = String::new();
    let _ = write!(s, "cpu{}", 0);
    for i in 1..cpu_count {
        let _ = write!(s, " cpu{}", i);
    }
    if cpu_count > 0 {
        let _ = write!(s, "\n");
    }
    let s = s.into_bytes();
    let len = s.len().min(buf.len());
    buf[..len].copy_from_slice(&s[..len]);
    len
}

fn sysfs_class(buf: &mut [u8]) -> usize {
    let entries = b"block\0net\0";
    let len = entries.len().min(buf.len());
    buf[..len].copy_from_slice(&entries[..len]);
    len
}

fn sysfs_block(buf: &mut [u8]) -> usize {
    let entries = b"sda\0";
    let len = entries.len().min(buf.len());
    buf[..len].copy_from_slice(&entries[..len]);
    len
}

fn sysfs_net(buf: &mut [u8]) -> usize {
    let entries = b"eth0\0lo\0";
    let len = entries.len().min(buf.len());
    buf[..len].copy_from_slice(&entries[..len]);
    len
}

fn sysfs_block_device(name: &str, buf: &mut [u8]) -> usize {
    let size_sectors = 65536u64; // 32MB default
    let info = format_str(format_args!(
        "name: {}\n\
         size: {}\n\
         type: disk\n\
         removable: 0\n\
         ro: 0\n",
        name, size_sectors,
    ));
    let info = info.as_bytes();
    let len = info.len().min(buf.len());
    buf[..len].copy_from_slice(&info[..len]);
    len
}

fn sysfs_net_device(name: &str, buf: &mut [u8]) -> usize {
    let info = format_str(format_args!(
        "name: {}\n\
         address: 52:54:00:12:34:56\n\
         mtu: 1500\n\
         speed: 1000\n\
         duplex: full\n\
         operational: up\n",
        name,
    ));
    let info = info.as_bytes();
    let len = info.len().min(buf.len());
    buf[..len].copy_from_slice(&info[..len]);
    len
}

fn format_str(args: core::fmt::Arguments<'_>) -> alloc::string::String {
    use core::fmt::Write;
    let mut s = alloc::string::String::new();
    let _ = s.write_fmt(args);
    s
}
