// V32: WASM Syscall-as-Host-Function Interface (WABI-inspired)
//
// Registers TrainOS syscalls as WASM-importable host functions.
// WASM modules can import functions like:
//   (import "tros:io" "fd_write" (func ...))
// which dispatches directly to the TrainOS syscall handler.
//
// Architecture:
//   - WasmSyscallTable maps (module_name, function_name) -> syscall_nr
//   - Auto-register ~50 commonly used syscalls in register_default_set()
//   - Interpreter falls through to this table when no explicit host function matches

use crate::syscall;

// ── Constants ─────────────────────────────────────────────────────────────

const MAX_SYSCALL_ENTRIES: usize = 256;
const NAME_LEN: usize = 32;

// ── Entry Structure ──────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct WasmSyscallEntry {
    module_name: [u8; NAME_LEN],
    function_name: [u8; NAME_LEN],
    syscall_nr: usize,
}

impl WasmSyscallEntry {
    const fn empty() -> Self {
        WasmSyscallEntry {
            module_name: [0u8; NAME_LEN],
            function_name: [0u8; NAME_LEN],
            syscall_nr: 0,
        }
    }
}

// ── Syscall Table ────────────────────────────────────────────────────────

pub struct WasmSyscallTable {
    entries: [WasmSyscallEntry; MAX_SYSCALL_ENTRIES],
    count: usize,
}

impl WasmSyscallTable {
    pub const fn new() -> Self {
        WasmSyscallTable {
            entries: [WasmSyscallEntry::empty(); MAX_SYSCALL_ENTRIES],
            count: 0,
        }
    }

    pub fn register(&mut self, module: &str, func: &str, syscall_nr: usize) {
        if self.count >= MAX_SYSCALL_ENTRIES {
            return;
        }
        let entry = &mut self.entries[self.count];
        let mlen = module.len().min(NAME_LEN - 1);
        let mbytes = module.as_bytes();
        for i in 0..mlen {
            entry.module_name[i] = mbytes[i];
        }
        entry.module_name[mlen] = 0;
        let flen = func.len().min(NAME_LEN - 1);
        let fbytes = func.as_bytes();
        for i in 0..flen {
            entry.function_name[i] = fbytes[i];
        }
        entry.function_name[flen] = 0;
        entry.syscall_nr = syscall_nr;
        self.count += 1;
    }

    pub fn lookup(&self, module: &[u8], func: &[u8]) -> Option<usize> {
        for i in 0..self.count {
            let entry = &self.entries[i];
            if name_matches(&entry.module_name, module)
                && name_matches(&entry.function_name, func)
            {
                return Some(entry.syscall_nr);
            }
        }
        None
    }

    pub fn lookup_by_func(&self, func: &[u8]) -> Option<usize> {
        for i in 0..self.count {
            let entry = &self.entries[i];
            if name_matches(&entry.function_name, func) {
                return Some(entry.syscall_nr);
            }
        }
        None
    }

    pub fn register_default_set(&mut self) {
        // File I/O
        self.register("tros:io", "fd_read",     syscall::SYS_READ);
        self.register("tros:io", "fd_write",    syscall::SYS_WRITE);
        self.register("tros:io", "fd_close",    syscall::SYS_CLOSE);
        self.register("tros:io", "fd_seek",     syscall::SYS_LSEEK);
        self.register("tros:io", "fd_open",     syscall::SYS_OPEN);
        self.register("tros:io", "fd_stat",     syscall::SYS_STAT);
        self.register("tros:io", "fd_dup",      syscall::SYS_DUP);
        self.register("tros:io", "fd_sync",     syscall::SYS_SYNC);
        self.register("tros:io", "fd_truncate", syscall::SYS_TRUNCATE);
        self.register("tros:io", "fd_ioctl",    syscall::SYS_IOCTL);
        self.register("tros:io", "fd_fcntl",    syscall::SYS_FCNTL);
        // Directory / Path
        self.register("tros:fs", "getcwd",      syscall::SYS_GETCWD);
        self.register("tros:fs", "chdir",       syscall::SYS_CHDIR);
        self.register("tros:fs", "mkdir",       syscall::SYS_MKDIR);
        self.register("tros:fs", "rmdir",       syscall::SYS_RMDIR);
        self.register("tros:fs", "unlink",      syscall::SYS_UNLINK);
        self.register("tros:fs", "rename",      syscall::SYS_RENAME);
        self.register("tros:fs", "access",      syscall::SYS_ACCESS);
        self.register("tros:fs", "getdents",    syscall::SYS_GETDENTS64);
        self.register("tros:fs", "chmod",       syscall::SYS_CHMOD);
        // Process
        self.register("tros:proc", "exit",      syscall::SYS_EXIT);
        self.register("tros:proc", "getpid",    syscall::SYS_GETPID);
        self.register("tros:proc", "getppid",   syscall::SYS_GETPPID);
        self.register("tros:proc", "gettid",    syscall::SYS_GETTID);
        self.register("tros:proc", "spawn",     syscall::SYS_SPAWN);
        self.register("tros:proc", "fork",      syscall::SYS_FORK);
        self.register("tros:proc", "exec",      syscall::SYS_EXEC);
        self.register("tros:proc", "kill",      syscall::SYS_KILL);
        self.register("tros:proc", "waitpid",   syscall::SYS_WAITPID);
        self.register("tros:proc", "signal",    syscall::SYS_SIGNAL);
        self.register("tros:proc", "yield",     syscall::SYS_YIELD);
        // Memory
        self.register("tros:mem", "mmap",       syscall::SYS_MMAP);
        self.register("tros:mem", "munmap",     syscall::SYS_MUNMAP);
        self.register("tros:mem", "mprotect",   syscall::SYS_MPROTECT);
        self.register("tros:mem", "brk",        syscall::SYS_BRK);
        self.register("tros:mem", "shm_map",    syscall::SYS_SHM_MAP);
        // Time
        self.register("tros:clock", "time_get",  syscall::SYS_CLOCK_GETTIME);
        self.register("tros:clock", "nanosleep", syscall::SYS_NANOSLEEP);
        self.register("tros:clock", "uptime",    syscall::SYS_UPTIME);
        // Socket
        self.register("tros:net", "socket",     syscall::SYS_SOCKET);
        self.register("tros:net", "bind",       syscall::SYS_BIND);
        self.register("tros:net", "listen",     syscall::SYS_LISTEN);
        self.register("tros:net", "accept",     syscall::SYS_ACCEPT);
        self.register("tros:net", "connect",    syscall::SYS_CONNECT);
        self.register("tros:net", "sendto",     syscall::SYS_SENDTO);
        self.register("tros:net", "recvfrom",   syscall::SYS_RECVFROM);
        // IPC
        self.register("tros:ipc", "ep_create",  syscall::SYS_EP_CREATE);
        self.register("tros:ipc", "send",       syscall::SYS_SEND);
        self.register("tros:ipc", "recv",       syscall::SYS_RECV);
        self.register("tros:ipc", "call",       syscall::SYS_CALL);
        self.register("tros:ipc", "reply",      syscall::SYS_REPLY);
        // Capabilities
        self.register("tros:cap", "mint",       syscall::SYS_MINT);
        self.register("tros:cap", "copy_cap",   syscall::SYS_COPY);
        self.register("tros:cap", "move_cap",   syscall::SYS_MOVE);
        self.register("tros:cap", "delete_cap", syscall::SYS_DELETE);
        // System
        self.register("tros:sys", "sysinfo",    syscall::SYS_SYSINFO);
        self.register("tros:sys", "getuid",     syscall::SYS_GETUID);
        self.register("tros:sys", "setuid",     syscall::SYS_SETUID);
        self.register("tros:sys", "umask",      syscall::SYS_UMASK);
        self.register("tros:sys", "reboot",     syscall::SYS_REBOOT);
        self.register("tros:sys", "proclist",   syscall::SYS_PROCLIST);
        self.register("tros:sys", "meminfo",    syscall::SYS_MEMINFO);
        // Resource / Namespace
        self.register("tros:res", "times",      syscall::SYS_TIMES);
        self.register("tros:res", "getrusage",  syscall::SYS_GETRUSAGE);
        self.register("tros:ns", "unshare",     syscall::SYS_UNSHARE);
        self.register("tros:ns", "sethostname", syscall::SYS_SETHOSTNAME);
        self.register("tros:ns", "gethostname", syscall::SYS_GETHOSTNAME);
        // Advanced I/O
        self.register("tros:aio", "pipe",       syscall::SYS_PIPE);
        self.register("tros:aio", "epoll_create", syscall::SYS_EPOLL_CREATE);
        self.register("tros:aio", "epoll_ctl",  syscall::SYS_EPOLL_CTL);
        self.register("tros:aio", "epoll_wait", syscall::SYS_EPOLL_WAIT);
        self.register("tros:aio", "io_uring_setup", syscall::SYS_IO_URING_SETUP);
    }

    pub fn count(&self) -> usize { self.count }
}

// ── Name Comparison Helper ────────────────────────────────────────────────

fn name_matches(stored: &[u8; NAME_LEN], input: &[u8]) -> bool {
    if input.is_empty() || input[0] == 0 {
        return stored[0] == 0;
    }
    let mut i = 0;
    while i < input.len() && i < NAME_LEN {
        if stored[i] != input[i] { return false; }
        if stored[i] == 0 { return false; }
        i += 1;
    }
    true
}

// ── Global State ──────────────────────────────────────────────────────────

static mut SYSCALL_TABLE: WasmSyscallTable = WasmSyscallTable::new();

// ── Public API ────────────────────────────────────────────────────────────

/// Initialize the default syscall table. Called once during kernel boot.
pub fn init_wasm_syscall_table() {
    unsafe {
        SYSCALL_TABLE.register_default_set();
        crate::println!("  V32: WASM host-call table: {} syscalls registered",
            SYSCALL_TABLE.count());
    }
}

/// Look up a syscall number by (module, function) name pair.
pub fn lookup_syscall(module: &[u8], func: &[u8]) -> Option<usize> {
    unsafe { SYSCALL_TABLE.lookup(module, func) }
}

/// Look up a syscall number by function name only (module-agnostic).
pub fn lookup_syscall_by_func(func: &[u8]) -> Option<usize> {
    unsafe { SYSCALL_TABLE.lookup_by_func(func) }
}

/// Dispatch a TrainOS syscall from a WASM module context.
pub fn dispatch_syscall(nr: usize, wasm_args: &[i64], pid: u32) -> i64 {
    let mut args = [0usize; 6];
    let n = wasm_args.len().min(6);
    for i in 0..n {
        args[i] = wasm_args[i] as usize;
    }
    let result = crate::syscall::syscall_dispatch_wasm(nr, &args, pid);
    result as i64
}

/// Get the total number of registered syscall aliases.
pub fn syscall_table_count() -> usize {
    unsafe { SYSCALL_TABLE.count() }
}

/// Convert a null-terminated [u8; 32] name to a &[u8] slice.
pub fn name_to_slice(name: &[u8; 32]) -> &[u8] {
    let mut len = 0;
    while len < 32 && name[len] != 0 {
        len += 1;
    }
    unsafe {
        core::slice::from_raw_parts(name.as_ptr(), len)
    }
}
