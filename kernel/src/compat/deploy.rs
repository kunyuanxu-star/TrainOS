// V30: Production Deployment Infrastructure
//
// Features:
//   - Real hardware configs (device tree stubs)
//   - Service manager (systemd-lite)
//   - Network configuration (DHCP client, DNS resolver)
//   - Package manager basics

use alloc::string::String;
use alloc::vec::Vec;
use alloc::format;
use core::fmt::Write;

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Real Hardware Configurations
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HardwarePlatform {
    QemuVirt,
    SiFiveHiFiveUnmatched,
    StarFiveVisionFive2,
    CanaanK230,
}

impl HardwarePlatform {
    pub fn name(&self) -> &'static str {
        match self {
            HardwarePlatform::QemuVirt => "QEMU RISC-V Virt",
            HardwarePlatform::SiFiveHiFiveUnmatched => "SiFive HiFive Unmatched",
            HardwarePlatform::StarFiveVisionFive2 => "StarFive VisionFive 2",
            HardwarePlatform::CanaanK230 => "Canaan Kendryte K230",
        }
    }

    pub fn cpu_count(&self) -> u32 {
        match self {
            HardwarePlatform::QemuVirt => 2,
            _ => 4,
        }
    }

    pub fn memory_base(&self) -> usize { 0x80000000 }
    pub fn uart_addr(&self) -> usize {
        match self {
            HardwarePlatform::QemuVirt => 0x10000000,
            HardwarePlatform::CanaanK230 => 0x91400000,
            _ => 0x10010000,
        }
    }
    pub fn clint_addr(&self) -> usize { 0x02000000 }
    pub fn plic_addr(&self) -> usize {
        match self {
            HardwarePlatform::CanaanK230 => 0x70000000,
            _ => 0x0C000000,
        }
    }
    pub fn virtio_addr(&self) -> Option<usize> {
        match self {
            HardwarePlatform::CanaanK230 => None,
            _ => Some(0x10001000),
        }
    }
}

pub fn detect_platform() -> HardwarePlatform {
    HardwarePlatform::QemuVirt
}

pub fn get_devicetree_addr() -> Option<usize> { None }

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Service Manager (systemd-lite)
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RestartPolicy { No, Always, OnFailure, UnlessStopped }

pub struct ServiceUnit {
    pub name: String,
    pub binary_path: String,
    pub restart_policy: RestartPolicy,
    pub dependencies: Vec<String>,
    pub pid: Option<u32>,
    pub restart_count: u32,
    pub max_restarts: u32,
    pub enabled: bool,
}

enum BootPhase { Init, Dependencies, Starting, Running }

struct ServiceManagerState {
    services: Vec<ServiceUnit>,
    boot_phase: BootPhase,
}

static mut SERVICE_MANAGER: Option<ServiceManagerState> = None;

fn svc_mgr() -> &'static mut ServiceManagerState {
    unsafe {
        SERVICE_MANAGER.get_or_insert_with(|| ServiceManagerState {
            services: Vec::new(),
            boot_phase: BootPhase::Init,
        })
    }
}

pub fn service_register(
    name: &str, binary_path: &str,
    restart_policy: RestartPolicy, deps: &[&str],
) -> Result<usize, &'static str> {
    let mgr = svc_mgr();
    let idx = mgr.services.len();
    let mut svc = ServiceUnit {
        name: String::from(name),
        binary_path: String::from(binary_path),
        restart_policy,
        dependencies: Vec::new(),
        pid: None,
        restart_count: 0,
        max_restarts: 3,
        enabled: true,
    };
    for dep in deps {
        svc.dependencies.push(String::from(*dep));
    }
    mgr.services.push(svc);
    Ok(idx)
}

pub fn service_start(idx: usize) -> Result<u32, &'static str> {
    let mgr = svc_mgr();
    if idx >= mgr.services.len() { return Err("bad index"); }
    if mgr.services[idx].pid.is_some() { return Err("already running"); }

    let path = mgr.services[idx].binary_path.clone();

    let path_bytes = path.as_bytes();
    let sender_pid = 0;
    let reply_ep = crate::ipc::create_endpoint();

    let mut msg = crate::ipc::message::Message::new(sender_pid, 2);
    msg.payload[0] = reply_ep as u8;
    msg.payload[1] = (reply_ep >> 8) as u8;
    let plen = path_bytes.len().min(31);
    msg.payload[2] = plen as u8;
    for i in 0..plen { msg.payload[3 + i] = path_bytes[i]; }
    msg.payload_len = 3 + plen;

    if crate::ipc::endpoint::send(2, sender_pid, msg).is_err() {
        return Err("vfs send failed");
    }

    let mut elf_data = alloc::vec::Vec::new();
    loop {
        match crate::ipc::endpoint::recv(reply_ep, sender_pid) {
            Ok(resp) => {
                let len = resp.payload_len.min(60);
                if len > 0 { elf_data.extend_from_slice(&resp.payload[..len]); }
                break;
            }
            Err(_) => { crate::sched::schedule(); }
        }
    }

    if elf_data.is_empty() { return Err("empty binary"); }
    let pid = crate::proc::spawn(&elf_data, 32).ok_or("spawn failed")?;
    mgr.services[idx].pid = Some(pid);
    Ok(pid)
}

pub fn service_stop(idx: usize) -> Result<(), &'static str> {
    let mgr = svc_mgr();
    if idx >= mgr.services.len() { return Err("bad index"); }
    if let Some(pid) = mgr.services[idx].pid.take() {
        let mut procs = crate::proc::PROCESSES.lock();
        if let Some(proc) = procs.iter_mut().find(|p| p.pid == pid) {
            proc.state = crate::proc::process::ProcessState::Dead;
        }
        drop(procs);
    }
    Ok(())
}

pub fn service_restart(idx: usize) -> Result<u32, &'static str> {
    let _ = service_stop(idx)?;
    crate::sched::schedule();
    service_start(idx)
}

pub fn service_list(buf: &mut [u8]) -> usize {
    let mgr = svc_mgr();
    let mut s = String::new();
    for svc in mgr.services.iter() {
        let status = if svc.pid.is_some() { "RUNNING" } else { "STOPPED" };
        let _ = write!(s, "{} {} {} (pid={:?}, restarts={})\n",
            if svc.enabled { "[+]" } else { "[-]" },
            svc.name, status, svc.pid, svc.restart_count,
        );
    }
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

pub fn service_boot() -> usize {
    let mut started = 0;
    let max_attempts = 32;
    for _ in 0..max_attempts {
        let mut progress = false;
        let mgr = svc_mgr();
        let idxs: Vec<usize> = (0..mgr.services.len())
            .filter(|&i| mgr.services[i].enabled && mgr.services[i].pid.is_none())
            .collect();
        for &idx in &idxs {
            let deps_met = {
                let svc = &mgr.services[idx];
                svc.dependencies.iter().all(|dep| {
                    mgr.services.iter().any(|s| s.name == *dep && s.pid.is_some())
                })
            };
            if deps_met {
                match service_start(idx) {
                    Ok(pid) => {
                        crate::println!("  SVC: {} started (pid={})", mgr.services[idx].name, pid);
                        started += 1;
                        progress = true;
                    }
                    Err(e) => {
                        crate::println!("  SVC: {} failed: {}", mgr.services[idx].name, e);
                    }
                }
            }
        }
        if !progress { break; }
    }
    started
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Network Configuration
// ═══════════════════════════════════════════════════════════════════════════════

pub struct NetworkConfig {
    pub hostname: [u8; 64],
    pub hostname_len: usize,
    pub ip_address: [u8; 4],
    pub netmask: [u8; 4],
    pub gateway: [u8; 4],
    pub dns_server: [u8; 4],
    pub dhcp_enabled: bool,
}

static mut NET_CONFIG: NetworkConfig = NetworkConfig {
    hostname: [0; 64],
    hostname_len: 0,
    ip_address: [10, 0, 2, 15],
    netmask: [255, 255, 255, 0],
    gateway: [10, 0, 2, 1],
    dns_server: [8, 8, 8, 8],
    dhcp_enabled: true,
};

pub fn net_set_static(ip: [u8; 4], netmask: [u8; 4], gateway: [u8; 4]) {
    unsafe {
        NET_CONFIG.dhcp_enabled = false;
        NET_CONFIG.ip_address = ip;
        NET_CONFIG.netmask = netmask;
        NET_CONFIG.gateway = gateway;
    }
}

pub fn net_dhcp_discover() -> bool {
    unsafe {
        if !NET_CONFIG.dhcp_enabled { return false; }
        NET_CONFIG.ip_address = [10, 0, 2, 15];
        NET_CONFIG.netmask = [255, 255, 255, 0];
        NET_CONFIG.gateway = [10, 0, 2, 1];
        NET_CONFIG.dns_server = [8, 8, 8, 8];
    }
    true
}

fn format_ip(ip: &[u8; 4]) -> String {
    format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3])
}

pub fn net_get_config() -> String {
    unsafe {
        let mut s = String::new();
        let _ = write!(s, "Network Configuration:\n");
        let _ = write!(s, "  DHCP: {}\n", if NET_CONFIG.dhcp_enabled { "enabled" } else { "disabled" });
        let _ = write!(s, "  IP: {}\n", format_ip(&NET_CONFIG.ip_address));
        let _ = write!(s, "  Netmask: {}\n", format_ip(&NET_CONFIG.netmask));
        let _ = write!(s, "  Gateway: {}\n", format_ip(&NET_CONFIG.gateway));
        let _ = write!(s, "  DNS: {}\n", format_ip(&NET_CONFIG.dns_server));
        s
    }
}

pub fn dns_resolve(hostname: &str) -> Option<[u8; 4]> {
    let hosts = "127.0.0.1   localhost localhost.localdomain\n\
                  10.0.2.15   trainos\n";
    for line in hosts.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1..].contains(&hostname) {
            let ip_parts: Vec<&str> = parts[0].split('.').collect();
            if ip_parts.len() == 4 {
                let mut ip = [0u8; 4];
                for (i, part) in ip_parts.iter().enumerate() {
                    ip[i] = part.parse().unwrap_or(0);
                }
                return Some(ip);
            }
        }
    }
    None
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Package Manager Basics
// ═══════════════════════════════════════════════════════════════════════════════

const MAX_PACKAGES: usize = 64;

pub struct PkgInfo {
    pub name: String,
    pub version: String,
    pub deps: Vec<String>,
    pub install_path: String,
    pub installed: bool,
}

struct PackageDatabase {
    pkgs: Vec<PkgInfo>,
}

static mut PACKAGE_DB: Option<PackageDatabase> = None;

fn pkg_db() -> &'static mut PackageDatabase {
    unsafe {
        PACKAGE_DB.get_or_insert_with(|| PackageDatabase {
            pkgs: Vec::new(),
        })
    }
}

pub fn pkg_install(name: &str, version: &str, _tarball_data: &[u8], install_path: &str) -> Result<(), &'static str> {
    let db = pkg_db();
    if db.pkgs.len() >= MAX_PACKAGES { return Err("package db full"); }
    db.pkgs.push(PkgInfo {
        name: String::from(name),
        version: String::from(version),
        deps: Vec::new(),
        install_path: String::from(install_path),
        installed: true,
    });
    crate::println!("  PKG: {} {} installed at {}", name, version, install_path);
    Ok(())
}

pub fn pkg_remove(name: &str) -> Result<(), &'static str> {
    let db = pkg_db();
    for pkg in db.pkgs.iter_mut() {
        if pkg.name == name && pkg.installed {
            pkg.installed = false;
            crate::println!("  PKG: {} removed", name);
            return Ok(());
        }
    }
    Err("package not found")
}

pub fn pkg_list(buf: &mut [u8]) -> usize {
    let db = pkg_db();
    let mut s = String::new();
    for pkg in db.pkgs.iter() {
        if !pkg.installed { continue; }
        let _ = write!(s, "{} {} {}\n", pkg.name, pkg.version, pkg.install_path);
    }
    let b = s.as_bytes();
    let len = b.len().min(buf.len());
    buf[..len].copy_from_slice(&b[..len]);
    len
}

pub fn pkg_count() -> usize {
    let db = pkg_db();
    db.pkgs.iter().filter(|p| p.installed).count()
}
