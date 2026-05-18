// Namespace subsystem — container isolation primitives
//
// Namespace types:
//   CLONE_NEWUTS  (0x04000000) — hostname/domainname isolation
//   CLONE_NEWPID  (0x20000000) — PID namespace isolation
//   CLONE_NEWNS   (0x00020000) — mount namespace (filesystem root)
//   CLONE_NEWNET  (0x40000000) — network namespace
//   CLONE_NEWIPC  (0x08000000) — IPC namespace
//   CLONE_NEWUSER (0x10000000) — user namespace

pub const CLONE_NEWUTS: usize = 0x0400_0000;
pub const CLONE_NEWPID: usize = 0x2000_0000;
pub const CLONE_NEWNS: usize = 0x0002_0000;
pub const CLONE_NEWNET: usize = 0x4000_0000;
pub const CLONE_NEWIPC: usize = 0x0800_0000;
pub const CLONE_NEWUSER: usize = 0x1000_0000;

// ── UTS Namespace ────────────────────────────────────────────────────────────

const UTS_MAX: usize = 16;
const MAX_UTS_NS: usize = 8;

struct UtsNamespace {
    ns_id: u32,
    hostname: [u8; UTS_MAX],
    hostname_len: usize,
    domainname: [u8; UTS_MAX],
    domainname_len: usize,
}

static mut UTS_NAMESPACES: [UtsNamespace; MAX_UTS_NS] = [
    UtsNamespace { ns_id: 0, hostname: [0; UTS_MAX], hostname_len: 0, domainname: [0; UTS_MAX], domainname_len: 0 },
    UtsNamespace { ns_id: 0, hostname: [0; UTS_MAX], hostname_len: 0, domainname: [0; UTS_MAX], domainname_len: 0 },
    UtsNamespace { ns_id: 0, hostname: [0; UTS_MAX], hostname_len: 0, domainname: [0; UTS_MAX], domainname_len: 0 },
    UtsNamespace { ns_id: 0, hostname: [0; UTS_MAX], hostname_len: 0, domainname: [0; UTS_MAX], domainname_len: 0 },
    UtsNamespace { ns_id: 0, hostname: [0; UTS_MAX], hostname_len: 0, domainname: [0; UTS_MAX], domainname_len: 0 },
    UtsNamespace { ns_id: 0, hostname: [0; UTS_MAX], hostname_len: 0, domainname: [0; UTS_MAX], domainname_len: 0 },
    UtsNamespace { ns_id: 0, hostname: [0; UTS_MAX], hostname_len: 0, domainname: [0; UTS_MAX], domainname_len: 0 },
    UtsNamespace { ns_id: 0, hostname: [0; UTS_MAX], hostname_len: 0, domainname: [0; UTS_MAX], domainname_len: 0 },
];
static mut UTS_COUNT: usize = 1; // ns 0 = default
static mut NEXT_UTS_ID: u32 = 1;

// Per-process UTS namespace tracking
static mut PROC_UTS_NS: [(u32, u32); 64] = [(0, 0); 64]; // (pid, uts_ns_id)
static mut PROC_UTS_COUNT: usize = 0;

pub fn init() {
    unsafe {
        // Default UTS namespace (ns_id=0)
        let default_name = b"trainos";
        let ns = &mut UTS_NAMESPACES[0];
        ns.ns_id = 0;
        ns.hostname_len = default_name.len().min(UTS_MAX);
        for i in 0..ns.hostname_len { ns.hostname[i] = default_name[i]; }
    }
}

/// Create a new UTS namespace. Returns ns_id.
pub fn new_uts_ns() -> Option<u32> {
    unsafe {
        if UTS_COUNT >= MAX_UTS_NS { return None; }
        let ns_id = NEXT_UTS_ID;
        NEXT_UTS_ID += 1;

        // Copy from default namespace (ns 0)
        let src = &UTS_NAMESPACES[0];
        let dst = &mut UTS_NAMESPACES[UTS_COUNT];
        dst.ns_id = ns_id;
        dst.hostname_len = src.hostname_len;
        for i in 0..dst.hostname_len { dst.hostname[i] = src.hostname[i]; }
        dst.domainname_len = src.domainname_len;
        for i in 0..dst.domainname_len { dst.domainname[i] = src.domainname[i]; }

        UTS_COUNT += 1;
        Some(ns_id)
    }
}

/// Assign a process to a UTS namespace.
pub fn set_process_uts(pid: u32, ns_id: u32) -> bool {
    unsafe {
        for i in 0..PROC_UTS_COUNT {
            if PROC_UTS_NS[i].0 == pid {
                PROC_UTS_NS[i].1 = ns_id;
                return true;
            }
        }
        if PROC_UTS_COUNT >= 64 { return false; }
        PROC_UTS_NS[PROC_UTS_COUNT] = (pid, ns_id);
        PROC_UTS_COUNT += 1;
        true
    }
}

/// Get the UTS namespace id for a process.
pub fn get_process_uts(pid: u32) -> u32 {
    unsafe {
        for i in 0..PROC_UTS_COUNT {
            if PROC_UTS_NS[i].0 == pid { return PROC_UTS_NS[i].1; }
        }
    }
    0 // default
}

/// Set hostname for a UTS namespace.
pub fn set_hostname(ns_id: u32, name: &[u8]) -> bool {
    unsafe {
        for i in 0..UTS_COUNT {
            if UTS_NAMESPACES[i].ns_id == ns_id {
                let len = name.len().min(UTS_MAX);
                UTS_NAMESPACES[i].hostname_len = len;
                for j in 0..len { UTS_NAMESPACES[i].hostname[j] = name[j]; }
                return true;
            }
        }
    }
    false
}

/// Get hostname for a UTS namespace. Returns length.
pub fn get_hostname(ns_id: u32, buf: &mut [u8]) -> usize {
    unsafe {
        for i in 0..UTS_COUNT {
            if UTS_NAMESPACES[i].ns_id == ns_id {
                let len = UTS_NAMESPACES[i].hostname_len.min(buf.len());
                for j in 0..len { buf[j] = UTS_NAMESPACES[i].hostname[j]; }
                return len;
            }
        }
    }
    0
}

// ── PID Namespace ────────────────────────────────────────────────────────────

const MAX_PID_NS: usize = 8;

struct PidNamespace {
    ns_id: u32,
    pid_offset: u32,    // PID in this namespace = global_pid - offset
    parent_ns: u32,     // parent namespace id (0 = init)
}

static mut PID_NAMESPACES: [PidNamespace; MAX_PID_NS] = [
    PidNamespace { ns_id: 0, pid_offset: 0, parent_ns: 0 },
    PidNamespace { ns_id: 0, pid_offset: 0, parent_ns: 0 },
    PidNamespace { ns_id: 0, pid_offset: 0, parent_ns: 0 },
    PidNamespace { ns_id: 0, pid_offset: 0, parent_ns: 0 },
    PidNamespace { ns_id: 0, pid_offset: 0, parent_ns: 0 },
    PidNamespace { ns_id: 0, pid_offset: 0, parent_ns: 0 },
    PidNamespace { ns_id: 0, pid_offset: 0, parent_ns: 0 },
    PidNamespace { ns_id: 0, pid_offset: 0, parent_ns: 0 },
];
static mut PID_NS_COUNT: usize = 1;
static mut NEXT_PID_NS_ID: u32 = 1;

// Per-process PID namespace assignment
static mut PROC_PID_NS: [(u32, u32); 64] = [(0, 0); 64];
static mut PROC_PID_COUNT: usize = 0;
static mut PID_NS_LAST_INIT: u32 = 1; // PID of "init" in new namespace

pub fn new_pid_ns(parent_pid: u32) -> Option<u32> {
    unsafe {
        if PID_NS_COUNT >= MAX_PID_NS { return None; }
        let ns_id = NEXT_PID_NS_ID;
        NEXT_PID_NS_ID += 1;

        // parent_ns = parent process's namespace
        let parent_ns = get_process_pid_ns(parent_pid);

        PID_NAMESPACES[PID_NS_COUNT] = PidNamespace {
            ns_id,
            pid_offset: PID_NS_LAST_INIT,
            parent_ns,
        };
        PID_NS_COUNT += 1;
        PID_NS_LAST_INIT += 1;

        Some(ns_id)
    }
}

pub fn set_process_pid_ns(pid: u32, ns_id: u32) -> bool {
    unsafe {
        for i in 0..PROC_PID_COUNT {
            if PROC_PID_NS[i].0 == pid {
                PROC_PID_NS[i].1 = ns_id;
                return true;
            }
        }
        if PROC_PID_COUNT >= 64 { return false; }
        PROC_PID_NS[PROC_PID_COUNT] = (pid, ns_id);
        PROC_PID_COUNT += 1;
        true
    }
}

pub fn get_process_pid_ns(pid: u32) -> u32 {
    unsafe {
        for i in 0..PROC_PID_COUNT {
            if PROC_PID_NS[i].0 == pid { return PROC_PID_NS[i].1; }
        }
    }
    0
}

/// Translate global PID to namespace-local PID.
pub fn pid_to_ns(global_pid: u32, ns_id: u32) -> u32 {
    if ns_id == 0 { return global_pid; }
    unsafe {
        for i in 0..PID_NS_COUNT {
            if PID_NAMESPACES[i].ns_id == ns_id {
                return global_pid - PID_NAMESPACES[i].pid_offset;
            }
        }
    }
    global_pid
}

// ── Mount Namespace (stub) ───────────────────────────────────────────────────

pub fn new_mount_ns() -> Option<u32> {
    // Simplified: mount namespaces echo the UTS namespace ID for now
    Some(0)
}

// ── Network/IPC/User Namespace (stubs) ───────────────────────────────────────

pub fn new_net_ns() -> Option<u32> { Some(0) }
pub fn new_ipc_ns() -> Option<u32> { Some(0) }
pub fn new_user_ns() -> Option<u32> { Some(0) }
