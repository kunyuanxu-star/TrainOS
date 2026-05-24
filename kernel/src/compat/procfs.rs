// V30: /proc filesystem completeness
//
// Implements full /proc directory tree for Linux ABI compatibility.

use crate::proc::process::ProcessState;
use alloc::string::String;
use alloc::format;

const PROC_BUF_SIZE: usize = 4096;

pub fn procfs_read(path: &[u8], buf: &mut [u8], pid: u32) -> usize {
    if buf.is_empty() { return 0; }
    let p = if path.len() > 0 && path[0] == b'/' { &path[1..] } else { path };

    if p.is_empty() || p == b"/" {
        return procfs_list_root(buf);
    }

    if p == b"self" {
        let s = format!("{}", pid);
        let b = s.as_bytes();
        let len = b.len().min(buf.len());
        buf[..len].copy_from_slice(&b[..len]);
        return len;
    }

    if let Ok(target_pid) = parse_pid(p) {
        let subpath = extract_subpath(p, target_pid);
        return procfs_pid_read(target_pid, subpath, buf);
    }

    match p {
        b"cpuinfo" => procfs_cpuinfo(buf),
        b"meminfo" => procfs_meminfo(buf),
        b"mounts" => procfs_mounts(buf),
        b"stat" => procfs_stat(buf),
        b"loadavg" => procfs_loadavg(buf),
        b"uptime" => procfs_uptime(buf),
        b"version" => procfs_version(buf),
        b"proc" => procfs_list_processes(buf),
        _ => 0,
    }
}

fn procfs_list_root(buf: &mut [u8]) -> usize {
    let s = "cpuinfo\0meminfo\0mounts\0stat\0loadavg\0uptime\0version\0self\0proc\0";
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_cpuinfo(buf: &mut [u8]) -> usize {
    let cpu_count = crate::per_cpu::hart_count();
    let mut s = String::new();
    for cpu in 0..cpu_count {
        use core::fmt::Write;
        let _ = write!(s,
            "processor\t: {}\nhart\t\t: {}\nisa\t\t: rv64imafdc\nmmu\t\t: sv39\nclock\t\t: 100MHz\n\n",
            cpu, cpu,
        );
    }
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_meminfo(buf: &mut [u8]) -> usize {
    let total_pages = crate::mem::buddy::total_pages() as u64;
    let free_pages = total_pages.saturating_sub(crate::mem::buddy::allocated_pages() as u64);
    let total_kb = total_pages * 4;
    let free_kb = free_pages * 4;
    let used_kb = total_kb.saturating_sub(free_kb);

    let s = format!(
        "MemTotal:       {:>8} kB\nMemFree:        {:>8} kB\nMemAvailable:   {:>8} kB\n\
         Buffers:        {:>8} kB\nCached:         {:>8} kB\n\
         SwapTotal:      {:>8} kB\nSwapFree:       {:>8} kB\n\
         Active:         {:>8} kB\nInactive:       {:>8} kB\n\
         Dirty:          {:>8} kB\nWriteback:      {:>8} kB\n",
        total_kb, free_kb, free_kb / 2, 0u64, 0u64,
        0u64, 0u64, used_kb / 2, used_kb / 4, 0u64, 0u64,
    );
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_mounts(buf: &mut [u8]) -> usize {
    let s = "rootfs / rootfs rw 0 0\n\
             proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0\n\
             sysfs /sys sysfs rw,nosuid,nodev,noexec,relatime 0 0\n\
             tmpfs /tmp tmpfs rw,nosuid,nodev,noexec,relatime 0 0\n";
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_stat(buf: &mut [u8]) -> usize {
    let ticks = unsafe { crate::trap::TICK_COUNT } as u64;
    let cpu_count = crate::per_cpu::hart_count() as u64;
    let procs_count = {
        let procs = crate::proc::PROCESSES.lock();
        procs.len() as u64
    };

    let s = format!(
        "cpu  {} {} {} {} 0 0 0 0 0 0\n\
         cpu0 {} {} {} {} 0 0 0 0 0 0\n\
         intr 0 0 0 0 0\nctxt 0\nbtime 0\n\
         processes {}\nprocs_running 1\nprocs_blocked 0\n",
        ticks, 0u64, 0u64, 0u64,
        ticks, 0u64, 0u64, 0u64,
        procs_count,
    );
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_loadavg(buf: &mut [u8]) -> usize {
    let (alive, total) = {
        let procs = crate::proc::PROCESSES.lock();
        let a = procs.iter().filter(|p| p.state != ProcessState::Dead).count();
        let t = procs.len();
        (a, t)
    };
    let s = format!("0.00 0.00 0.00 {}/{}\n", alive, total);
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_uptime(buf: &mut [u8]) -> usize {
    let ticks = unsafe { crate::trap::TICK_COUNT } as u64;
    let seconds = (ticks * 10) / 1000;
    let idle_seconds = seconds / 2;
    let s = format!("{}.{:02} {}.{:02}\n", seconds, 0u64, idle_seconds, 0u64);
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_version(buf: &mut [u8]) -> usize {
    let s = "Linux version 5.15.0-trainos (root@trainos) (riscv64-linux-gnu-gcc) #1 SMP PREEMPT TrainOS V30\n";
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_pid_read(target_pid: u32, subpath: &[u8], buf: &mut [u8]) -> usize {
    if subpath.is_empty() || subpath == b"/" {
        return procfs_pid_list(target_pid, buf);
    }
    let p = if subpath.len() > 0 && subpath[0] == b'/' { &subpath[1..] } else { subpath };
    match p {
        b"maps" => procfs_pid_maps(target_pid, buf),
        b"status" => procfs_pid_status(target_pid, buf),
        b"cmdline" => procfs_pid_cmdline(target_pid, buf),
        b"fd" => procfs_pid_fd_list(target_pid, buf),
        _ => {
            if let Some(rest) = p.strip_prefix(b"fd/") {
                return procfs_pid_fd_read(target_pid, rest, buf);
            }
            0
        }
    }
}

fn procfs_pid_list(pid: u32, buf: &mut [u8]) -> usize {
    let s = "maps\0status\0cmdline\0fd\0";
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_pid_maps(pid: u32, buf: &mut [u8]) -> usize {
    let name = {
        let procs = crate::proc::PROCESSES.lock();
        let proc = procs.iter().find(|p| p.pid == pid);
        match proc {
            Some(p) => format_pid_name(p),
            None => return 0,
        }
    };

    let s = format!(
        "00000000-00001000 r--p 00000000 00:00 0          [sigpage]\n\
         00010000-00020000 rw-p 00000000 00:00 0          [heap]\n\
         10000000-10001000 r-xp 00000000 00:00 0          {}\n\
         10001000-10002000 r--p 00000000 00:00 0          {}\n\
         10002000-10003000 rw-p 00001000 00:00 0          {}\n\
         7f00000000-7f00001000 rw-p 00000000 00:00 0      [stack]\n",
        name, name, name,
    );
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_pid_status(pid: u32, buf: &mut [u8]) -> usize {
    let (name, state_char, uid, gid, parent) = {
        let procs = crate::proc::PROCESSES.lock();
        let proc = procs.iter().find(|p| p.pid == pid);
        match proc {
            Some(p) => {
                let sc = match p.state {
                    ProcessState::Ready => 'R',
                    ProcessState::Running => 'R',
                    ProcessState::Waiting => 'S',
                    ProcessState::Dead => 'Z',
                };
                (format_pid_name(p), sc, p.uid, p.gid, p.parent.unwrap_or(0))
            }
            None => return 0,
        }
    };

    let s = format!(
        "Name:\t{}\nState:\t{}\nTgid:\t{}\nPid:\t{}\nPPid:\t{}\n\
         Uid:\t{}\t{}\t{}\t{}\nGid:\t{}\t{}\t{}\t{}\n\
         VmSize:\t256 kB\nVmRSS:\t64 kB\nThreads:\t1\n",
        name, state_char, pid, pid, parent,
        uid, uid, uid, uid, gid, gid, gid, gid,
    );
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_pid_cmdline(pid: u32, buf: &mut [u8]) -> usize {
    let name = {
        let procs = crate::proc::PROCESSES.lock();
        let proc = procs.iter().find(|p| p.pid == pid);
        match proc {
            Some(p) => format_pid_name(p),
            None => return 0,
        }
    };
    let b = name.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_pid_fd_list(pid: u32, buf: &mut [u8]) -> usize {
    let mut s = String::new();
    for fd in 0..64u32 {
        let found = unsafe { crate::syscall::posix::find_fd_internal(pid, fd as usize) };
        if found.is_some() {
            use core::fmt::Write;
            let _ = write!(s, "{}\0", fd);
        }
    }
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_pid_fd_read(pid: u32, fd_str: &[u8], buf: &mut [u8]) -> usize {
    let mut fd: u32 = 0;
    for &c in fd_str {
        if c >= b'0' && c <= b'9' {
            fd = fd * 10 + (c - b'0') as u32;
        } else {
            break;
        }
    }
    let s = format!("/proc/{}/fd/{}\0", pid, fd);
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn procfs_list_processes(buf: &mut [u8]) -> usize {
    let procs = crate::proc::PROCESSES.lock();
    let mut s = String::new();
    for proc in procs.iter() {
        if proc.state == ProcessState::Dead { continue; }
        use core::fmt::Write;
        let _ = write!(s, "{}\n", proc.pid);
    }
    drop(procs);
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

fn parse_pid(s: &[u8]) -> Result<u32, ()> {
    let mut pid: u32 = 0;
    for &c in s.iter() {
        if c >= b'0' && c <= b'9' {
            pid = pid * 10 + (c - b'0') as u32;
        } else if c == b'/' || c == 0 {
            return Ok(pid);
        } else {
            return Err(());
        }
    }
    Ok(pid)
}

fn extract_subpath<'a>(path: &'a [u8], _pid: u32) -> &'a [u8] {
    let mut offset = 0;
    while offset < path.len() && path[offset] >= b'0' && path[offset] <= b'9' {
        offset += 1;
    }
    if offset < path.len() && path[offset] == b'/' {
        &path[offset..]
    } else {
        &[]
    }
}

fn format_pid_name<'a>(proc: &'a crate::proc::process::Process) -> alloc::string::String {
    unsafe {
        for i in 0..crate::syscall::proc::PROCESS_NAME_COUNT {
            if crate::syscall::proc::PROCESS_NAMES[i].0 == proc.pid {
                let name = &crate::syscall::proc::PROCESS_NAMES[i].1;
                let len = name.iter().position(|&c| c == 0).unwrap_or(16);
                return core::str::from_utf8(&name[..len]).unwrap_or("trainos").into();
            }
        }
    }
    "trainos".into()
}
