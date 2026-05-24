pub mod cap;
pub mod ipc;
pub mod posix;
pub mod proc;
pub mod memory;
pub mod socket;
pub mod epoll;
pub mod time;
pub mod fs;

use crate::trap::TrapFrame;

// V21.12: Syscall statistics counters
pub static mut SYSCALL_COUNTERS: [u64; 256] = [0u64; 256];

// ── Syscall Number Assignments ───────────────────────────────────────────────

// Core: 0-7
pub const SYS_EXIT: usize = 0;
pub const SYS_PUTCHAR: usize = 1;
pub const SYS_GETCHAR: usize = 2;
pub const SYS_SPAWN: usize = 3;
pub const SYS_FORK: usize = 4;
pub const SYS_GETPID: usize = 5;
pub const SYS_YIELD: usize = 6;
pub const SYS_EXEC: usize = 7;

// IPC: 10-14
pub const SYS_EP_CREATE: usize = 10;
pub const SYS_SEND: usize = 11;
pub const SYS_RECV: usize = 12;
pub const SYS_CALL: usize = 13;
pub const SYS_REPLY: usize = 14;

// MMIO / Memory: 20-25
pub const SYS_MMIO_MAP: usize = 20;
pub const SYS_UNMAP: usize = 21;
pub const SYS_MAP_MMIO: usize = 22;
pub const SYS_MMIO_READ32: usize = 23;
pub const SYS_MMIO_WRITE32: usize = 24;
pub const SYS_SHM_MAP: usize = 25;

// Capability: 30-34
pub const SYS_MINT: usize = 30;
pub const SYS_COPY: usize = 31;
pub const SYS_MOVE: usize = 32;
pub const SYS_DELETE: usize = 33;
pub const SYS_CAP_STATS: usize = 34;

// Block I/O + System info: 40-46
pub const SYS_BLK_READ: usize = 40;
pub const SYS_PROCLIST: usize = 41;
pub const SYS_KILL: usize = 42;
pub const SYS_MEMINFO: usize = 43;
pub const SYS_PERF_STATS: usize = 44;
pub const SYS_BLK_WRITE: usize = 45;
pub const SYS_UPTIME: usize = 46;

// POSIX I/O: 50-57
pub const SYS_OPEN: usize = 50;
pub const SYS_READ: usize = 51;
pub const SYS_WRITE: usize = 52;
pub const SYS_CLOSE: usize = 53;
pub const SYS_STAT: usize = 54;
pub const SYS_LSEEK: usize = 55;
pub const SYS_DUP: usize = 56;
pub const SYS_GETCWD: usize = 57;

// User/Permissions: 60-64
pub const SYS_GETUID: usize = 60;
pub const SYS_SETUID: usize = 61;
pub const SYS_CHMOD: usize = 62;
pub const SYS_SIGNAL: usize = 63;
pub const SYS_WAITPID: usize = 64;

// Process (V14.0): 65-71
pub const SYS_GETPPID: usize = 65;
pub const SYS_GETTID: usize = 66;
pub const SYS_NANOSLEEP: usize = 67;
pub const SYS_CLOCK_GETTIME: usize = 68;
pub const SYS_UMASK: usize = 69;
pub const SYS_SETSID: usize = 70;
pub const SYS_SYSINFO: usize = 71;

// Filesystem (V14.0): 72-82
pub const SYS_PIPE: usize = 72;
pub const SYS_FCNTL: usize = 73;
pub const SYS_IOCTL: usize = 74;
pub const SYS_GETDENTS64: usize = 75;
pub const SYS_MKDIR: usize = 76;
pub const SYS_RMDIR: usize = 77;
pub const SYS_UNLINK: usize = 78;
pub const SYS_RENAME: usize = 79;
pub const SYS_CHDIR: usize = 80;
pub const SYS_ACCESS: usize = 81;
pub const SYS_TRUNCATE: usize = 82;

// Memory management (V14.0): 83-86
pub const SYS_MMAP: usize = 83;
pub const SYS_MUNMAP: usize = 84;
pub const SYS_MPROTECT: usize = 85;
pub const SYS_BRK: usize = 86;

// Socket (V14.0): 90-96
pub const SYS_SOCKET: usize = 90;
pub const SYS_BIND: usize = 91;
pub const SYS_LISTEN: usize = 92;
pub const SYS_ACCEPT: usize = 93;
pub const SYS_CONNECT: usize = 94;
pub const SYS_SENDTO: usize = 95;
pub const SYS_RECVFROM: usize = 96;

// epoll (V14.0): 100-102
pub const SYS_EPOLL_CREATE: usize = 100;
pub const SYS_EPOLL_CTL: usize = 101;
pub const SYS_EPOLL_WAIT: usize = 102;

// Namespace (V15.0): 110-113
pub const SYS_UNSHARE: usize = 110;
pub const SYS_SETHOSTNAME: usize = 111;
pub const SYS_GETHOSTNAME: usize = 112;
pub const SYS_SETNS: usize = 113;

// CPU affinity (V15.0): 114-115
pub const SYS_SCHED_SETAFFINITY: usize = 114;
pub const SYS_SCHED_GETAFFINITY: usize = 115;

// Resource usage (V15.0): 116-117
pub const SYS_TIMES: usize = 116;
pub const SYS_GETRUSAGE: usize = 117;

// Device driver (V15.0): 118-120
pub const SYS_REGISTER_DRV: usize = 118;
pub const SYS_UNREGISTER_DRV: usize = 119;
pub const SYS_LIST_DRVS: usize = 120;

// System (V15.0): 121-122
pub const SYS_SYNC: usize = 121;
pub const SYS_REBOOT: usize = 122;

// Security (V21): 130-132
pub const SYS_SECCOMP_ADD: usize = 130;
pub const SYS_CAP_AUDIT: usize = 131;
pub const SYS_SYSCALL_STATS: usize = 132;


// io_uring (V22): 140-143
pub const SYS_IO_URING_SETUP: usize = 140;
pub const SYS_IO_URING_ENTER: usize = 141;
pub const SYS_IO_URING_REGISTER: usize = 142;

// Virtualization (V23): 150-155
pub const SYS_VM_CREATE: usize = 150;
pub const SYS_VM_DESTROY: usize = 151;
pub const SYS_VM_START: usize = 152;
pub const SYS_VM_LIST: usize = 153;
pub const SYS_VM_PAUSE: usize = 154;
pub const SYS_VM_RESUME: usize = 155;

// Kernel extensions (V24): 160-162
pub const SYS_EXT_REGISTER: usize = 160;
pub const SYS_EXT_UNREGISTER: usize = 161;
pub const SYS_EXT_LIST: usize = 162;

// NUMA (V25): 170-174
pub const SYS_NUMA_NODES: usize = 170;
pub const SYS_NUMA_ALLOC: usize = 171;
pub const SYS_NUMA_MIGRATE: usize = 172;
pub const SYS_NUMA_INFO: usize = 173;
pub const SYS_NUMA_BALANCE: usize = 174;

// Distributed (V26): 180-183
pub const SYS_REMOTE_NODE_ADD: usize = 180;
pub const SYS_REMOTE_EP_PUBLISH: usize = 181;
pub const SYS_REMOTE_EP_LOOKUP: usize = 182;
pub const SYS_REMOTE_SEND: usize = 183;


// ASLR/Cheri (V27): 200-205
pub const SYS_ASLR_INIT: usize = 200;
pub const SYS_CHERI_CAP_CREATE: usize = 201;
pub const SYS_CHERI_CAP_CHECK: usize = 202;
pub const SYS_SANDBOX_ADD: usize = 203;
pub const SYS_SANDBOX_CHECK: usize = 204;

// WASM (V28): 210-215
pub const SYS_WASM_LOAD: usize = 210;
pub const SYS_WASM_UNLOAD: usize = 211;
pub const SYS_WASM_LIST: usize = 212;
pub const SYS_WASM_EXECUTE: usize = 213;
pub const SYS_WASM_MEM_READ: usize = 214;
pub const SYS_WASM_MEM_WRITE: usize = 215;

// AI/GPU (V29): 220-223
pub const SYS_GPU_REGISTER: usize = 220;
pub const SYS_GPU_LIST: usize = 221;
pub const SYS_AI_SUBMIT: usize = 222;
pub const SYS_AI_NEXT: usize = 223;

// Linux Compat (V30): 300-302
pub const SYS_COMPAT_INIT: usize = 300;
pub const SYS_COMPAT_TRANSLATE: usize = 301;
pub const SYS_COMPAT_SETUP_AUXV: usize = 302;

// ── Dispatch ─────────────────────────────────────────────────────────────────

pub fn syscall_dispatch(tf: &mut TrapFrame) {
    let nr = tf.a7;

    // V21: Seccomp check before syscall execution
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    {
        let (allowed, should_kill) = crate::security::seccomp_check(pid, nr);
        if should_kill {
            crate::println!("seccomp: killing pid={} for syscall nr={}", pid, nr);
            crate::trap::kill_process_impl(pid);
        }
        if !allowed { tf.a0 = usize::MAX; return; }
    }

    // V24: SYSCALL_ENTER hook — fires before syscall dispatch
    crate::extension::run_hook(crate::extension::HOOK_SYSCALL_ENTER, nr as u64, pid as u64);

    crate::syscall::proc::account_stime();
    let arg0 = tf.a0;
    let arg1 = tf.a1;
    let arg2 = tf.a2;
    let arg3 = tf.a3;

    // V21.12: Increment syscall stats counter
    unsafe { SYSCALL_COUNTERS[nr] += 1; }

    let result = match nr {
        // Core
        SYS_PUTCHAR => {
            unsafe { core::arch::asm!("ecall", in("a7") 1usize, in("a0") tf.a0); }
            Ok(0)
        }
        SYS_GETCHAR => {
            let c: usize;
            unsafe { core::arch::asm!("ecall", in("a7") 2usize, lateout("a0") c); }
            Ok(c)
        }
        SYS_EP_CREATE => ipc::sys_ep_create(),
        SYS_SEND => ipc::sys_send(arg0, arg1 as u16, arg2, arg3),
        SYS_RECV => ipc::sys_recv(arg0, arg1, arg2),
        SYS_MINT => cap::sys_mint(arg0, arg1 as u8),
        SYS_COPY => cap::sys_copy(arg0, arg1 as u32, arg2),
        SYS_MOVE => cap::sys_move(arg0, arg1 as u32, arg2),
        SYS_DELETE => cap::sys_delete(arg0),
        SYS_CAP_STATS => cap::sys_cap_stats(),
        SYS_MAP_MMIO => {
            let phys = arg0;
            let size = arg1;
            if phys == 0 || size == 0 || size > 0x1000 {
                Err("invalid mmio args")
            } else {
                sys_map_mmio(phys, size)
            }
        }
        SYS_MMIO_MAP => proc::sys_mmio_map(arg0, arg1),
        SYS_EXIT => proc::sys_exit(arg0 as i32),
        SYS_SPAWN => proc::sys_spawn(arg0, arg1),
        SYS_EXEC => proc::sys_exec(arg0),
        SYS_FORK => proc::sys_fork(tf.sepc),
        SYS_GETPID => Ok(crate::sched::current_thread()
            .map(|t| unsafe { (*t).owner as usize })
            .unwrap_or(0)),
        SYS_YIELD => { crate::sched::schedule(); Ok(0) }
        SYS_OPEN => posix::sys_open(arg0, arg1, arg2),
        SYS_READ => posix::sys_read(arg0, arg1, arg2),
        SYS_WRITE => posix::sys_write(arg0, arg1, arg2),
        SYS_CLOSE => posix::sys_close(arg0),
        SYS_STAT => posix::sys_stat(arg0, arg1),
        SYS_LSEEK => posix::sys_lseek(arg0, arg1 as isize, arg2),
        SYS_DUP => posix::sys_dup(arg0),
        SYS_GETCWD => posix::sys_getcwd(arg0, arg1),
        SYS_MMIO_READ32 => sys_mmio_read32(arg0),
        SYS_MMIO_WRITE32 => sys_mmio_write32(arg0, arg1),
        SYS_BLK_READ => proc::sys_blk_read(arg0, arg1, arg2),
        SYS_BLK_WRITE => proc::sys_blk_write(arg0, arg1, arg2),
        SYS_PROCLIST => proc::sys_proclist(arg0, arg1),
        SYS_KILL => {
            let pid = crate::sched::current_thread()
                .map(|t| unsafe { (*t).owner })
                .unwrap_or(0);
            crate::security::cap_audit_log(pid, 10, arg0 as u32 as usize);
            proc::sys_kill(arg0 as u32)
        }
        SYS_MEMINFO => Ok(crate::mem::buddy::allocated_pages()),
        SYS_PERF_STATS => {
            let sends = crate::ipc::endpoint::SEND_COUNT.load(core::sync::atomic::Ordering::Relaxed);
            let recvs = crate::ipc::endpoint::RECV_COUNT.load(core::sync::atomic::Ordering::Relaxed);
            let ctx = crate::sched::CTX_SWITCH_COUNT.load(core::sync::atomic::Ordering::Relaxed);
            Ok(((sends & 0xFFFFF) | ((recvs & 0xFFFFF) << 20) | ((ctx & 0xFFFFFF) << 40)) as usize)
        }
        SYS_UPTIME => Ok(unsafe { crate::trap::TICK_COUNT }),
        SYS_SHM_MAP => proc::sys_shm_map(arg0 as u32, arg1),
        SYS_GETUID => proc::sys_getuid(),
        SYS_SETUID => proc::sys_setuid(arg0 as u32),
        SYS_CHMOD => proc::sys_chmod(arg0, arg1 as u16),
        SYS_SIGNAL => proc::sys_signal(arg0 as u32, arg1),
        SYS_WAITPID => proc::sys_waitpid(arg0 as i32, arg1, arg2),

        // V14.0 — Process
        SYS_GETPPID => proc::sys_getppid(),
        SYS_GETTID => proc::sys_gettid(),
        SYS_NANOSLEEP => time::sys_nanosleep(arg0, arg1),
        SYS_CLOCK_GETTIME => time::sys_clock_gettime(arg0, arg1),
        SYS_UMASK => proc::sys_umask(arg0 as u16),
        SYS_SETSID => proc::sys_setsid(),
        SYS_SYSINFO => proc::sys_sysinfo(arg0),

        // V14.0 — Filesystem
        SYS_PIPE => fs::sys_pipe(arg0),
        SYS_FCNTL => fs::sys_fcntl(arg0, arg1, arg2),
        SYS_IOCTL => fs::sys_ioctl(arg0, arg1, arg2),
        SYS_GETDENTS64 => fs::sys_getdents64(arg0, arg1, arg2),
        SYS_MKDIR => fs::sys_mkdir(arg0, arg1),
        SYS_RMDIR => fs::sys_rmdir(arg0),
        SYS_UNLINK => fs::sys_unlink(arg0),
        SYS_RENAME => fs::sys_rename(arg0, arg1),
        SYS_CHDIR => fs::sys_chdir(arg0),
        SYS_ACCESS => fs::sys_access(arg0, arg1),
        SYS_TRUNCATE => fs::sys_truncate(arg0, arg1),

        // V14.0 — Memory
        SYS_MMAP => {
            let pid = crate::sched::current_thread()
                .map(|t| unsafe { (*t).owner })
                .unwrap_or(0);
            let a4 = tf.a4 as isize;
            let a5 = tf.a5 as isize;
            crate::security::cap_audit_log(pid, 11, arg1);
            memory::sys_mmap(arg0, arg1, arg2, arg3, a4, a5)
        }
        SYS_MUNMAP => {
            let pid = crate::sched::current_thread()
                .map(|t| unsafe { (*t).owner })
                .unwrap_or(0);
            crate::security::cap_audit_log(pid, 12, arg0);
            memory::sys_munmap(arg0, arg1)
        }
        SYS_MPROTECT => {
            let pid = crate::sched::current_thread()
                .map(|t| unsafe { (*t).owner })
                .unwrap_or(0);
            crate::security::cap_audit_log(pid, 13, arg0);
            memory::sys_mprotect(arg0, arg1, arg2)
        }
        SYS_BRK => memory::sys_brk(arg0),

        // V14.0 — Socket
        SYS_SOCKET => socket::sys_socket(arg0, arg1, arg2),
        SYS_BIND => socket::sys_bind(arg0, arg1, arg2),
        SYS_LISTEN => socket::sys_listen(arg0, arg1),
        SYS_ACCEPT => socket::sys_accept(arg0),
        SYS_CONNECT => socket::sys_connect(arg0, arg1, arg2),
        SYS_SENDTO => socket::sys_sendto(arg0, arg1, arg2, arg3, tf.a4, tf.a5),
        SYS_RECVFROM => socket::sys_recvfrom(arg0, arg1, arg2, arg3, tf.a4, tf.a5),

        // V14.0 — epoll
        SYS_EPOLL_CREATE => epoll::sys_epoll_create(arg0),
        SYS_EPOLL_CTL => epoll::sys_epoll_ctl(arg0, arg1, arg2, arg3),
        SYS_EPOLL_WAIT => epoll::sys_epoll_wait(arg0, arg1, arg2, arg3 as isize),

        // V15.0 — Namespace
        SYS_UNSHARE => proc::sys_unshare(arg0),
        SYS_SETHOSTNAME => proc::sys_sethostname(arg0, arg1),
        SYS_GETHOSTNAME => proc::sys_gethostname(arg0, arg1),
        SYS_SETNS => proc::sys_setns(arg0, arg1),

        // V15.0 — CPU affinity
        SYS_SCHED_SETAFFINITY => proc::sys_sched_setaffinity(arg0, arg1, arg2),
        SYS_SCHED_GETAFFINITY => proc::sys_sched_getaffinity(arg0, arg1, arg2),

        // V15.0 — Resource usage
        SYS_TIMES => proc::sys_times(arg0),
        SYS_GETRUSAGE => proc::sys_getrusage(arg0, arg1),

        // V15.0 — Device driver
        SYS_REGISTER_DRV => proc::sys_register_drv(arg0, arg1, arg2),
        SYS_UNREGISTER_DRV => proc::sys_unregister_drv(arg0),
        SYS_LIST_DRVS => proc::sys_list_drvs(arg0, arg1),

        // V15.0 — System
        SYS_SYNC => proc::sys_sync(),
        SYS_REBOOT => proc::sys_reboot(arg0, arg1),


        // V22 — io_uring
        SYS_IO_URING_SETUP => proc::sys_io_uring_setup(arg0),
        SYS_IO_URING_ENTER => proc::sys_io_uring_enter(arg0, arg1, arg2),
        SYS_IO_URING_REGISTER => proc::sys_io_uring_register(arg0, arg1, arg2),

        // V23 — Virtualization
        SYS_VM_CREATE => proc::sys_vm_create(arg0),
        SYS_VM_DESTROY => proc::sys_vm_destroy(arg0 as u32),
        SYS_VM_START => proc::sys_vm_start(arg0 as u32, arg1),
        SYS_VM_LIST => proc::sys_vm_list(arg0, arg1),
        SYS_VM_PAUSE => proc::sys_vm_pause(arg0 as u32),
        SYS_VM_RESUME => proc::sys_vm_resume(arg0 as u32),

        // V24 — Kernel extensions
        SYS_EXT_REGISTER => proc::sys_ext_register(arg0, arg1, arg2, arg3),
        SYS_EXT_UNREGISTER => proc::sys_ext_unregister(arg0),
        SYS_EXT_LIST => proc::sys_ext_list(arg0, arg1),

        // V25 — NUMA
        SYS_NUMA_NODES => proc::sys_numa_nodes(arg0, arg1),
        SYS_NUMA_ALLOC => proc::sys_numa_alloc(arg0 as u8),
        SYS_NUMA_MIGRATE => proc::sys_numa_migrate(arg0, arg1 as u8, arg2 as u8),
        SYS_NUMA_INFO => proc::sys_numa_info(arg0, arg1),
        SYS_NUMA_BALANCE => proc::sys_numa_balance(),


        // V27 — ASLR/Cheri/Sandbox
        SYS_ASLR_INIT => proc::sys_aslr_init(),
        SYS_CHERI_CAP_CREATE => proc::sys_cheri_cap_create(arg0, arg1, arg2 as u16),
        SYS_CHERI_CAP_CHECK => proc::sys_cheri_cap_check(arg0),
        SYS_SANDBOX_ADD => proc::sys_sandbox_add(arg0, arg1),
        SYS_SANDBOX_CHECK => proc::sys_sandbox_check(arg0, arg1),

        // V28 — WASM
        SYS_WASM_LOAD => proc::sys_wasm_load(arg0, arg1),
        SYS_WASM_UNLOAD => proc::sys_wasm_unload(arg0),
        SYS_WASM_LIST => proc::sys_wasm_list(arg0, arg1),
        SYS_WASM_EXECUTE => proc::sys_wasm_execute(arg0, arg1),
        SYS_WASM_MEM_READ => proc::sys_wasm_mem_read(arg0, arg1, arg2),
        SYS_WASM_MEM_WRITE => proc::sys_wasm_mem_write(arg0, arg1, arg2, arg3),

        // V29 — AI/GPU
        SYS_GPU_REGISTER => proc::sys_gpu_register(arg0, arg1, arg2),
        SYS_GPU_LIST => proc::sys_gpu_list(arg0, arg1),
        SYS_AI_SUBMIT => proc::sys_ai_submit(arg0 as u32, arg1, arg2 as u8, arg3),
        SYS_AI_NEXT => proc::sys_ai_next(arg0, arg1),

        // V30 — Linux compat
        SYS_COMPAT_INIT => proc::sys_compat_init(),
        SYS_COMPAT_TRANSLATE => proc::sys_compat_translate(arg0),
        SYS_COMPAT_SETUP_AUXV => proc::sys_compat_setup_auxv(arg0, arg1, arg2, arg3, tf.a4 as usize),

        // V26 — Distributed
        SYS_REMOTE_NODE_ADD => proc::sys_remote_node_add(arg0, arg1),
        SYS_REMOTE_EP_PUBLISH => proc::sys_remote_ep_publish(arg0, arg1, arg2),
        SYS_REMOTE_EP_LOOKUP => proc::sys_remote_ep_lookup(arg0, arg1),
        SYS_REMOTE_SEND => proc::sys_remote_send(arg0 as u32, arg1, arg2, arg3),

        // V21 — Security
        SYS_SECCOMP_ADD => proc::sys_seccomp_add(arg0 as u32, arg1),
        SYS_CAP_AUDIT => proc::sys_cap_audit(arg0, arg1),
        SYS_SYSCALL_STATS => proc::sys_syscall_stats(arg0, arg1),

        _ => Err("unknown syscall"),
    };

    match result {
        Ok(val) => tf.a0 = val,
        Err(e) => {
            // Log failed syscalls for debugging (every 64th to avoid spam)
            static mut ERR_COUNT: usize = 0;
            unsafe { ERR_COUNT += 1; }
            if unsafe { ERR_COUNT & 63 == 0 } {
                crate::println!("  syscall nr={} failed: {} (count={})", nr, e, unsafe { ERR_COUNT });
            }
            tf.a0 = usize::MAX;
        }
    }

    // V24: SYSCALL_EXIT hook — fires after syscall completes
    crate::extension::run_hook(crate::extension::HOOK_SYSCALL_EXIT, nr as u64, pid as u64);

    tf.sepc += 4;
}

// ── V21.12: Syscall Stats Read ─────────────────────────────────────────────────
/// Serialize syscall counters into a binary buffer.
/// Format per entry: [nr:2][count:8] = 10 bytes each (only non-zero entries).
pub fn syscall_stats_read(buf: &mut [u8]) -> usize {
    let mut pos = 0usize;
    unsafe {
        for nr in 0..256 {
            let count = SYSCALL_COUNTERS[nr];
            if count == 0 { continue; }
            if pos + 10 > buf.len() { break; }
            buf[pos] = nr as u8;
            buf[pos+1] = (nr >> 8) as u8;
            for b in 0..8 {
                buf[pos+2+b] = (count >> (b*8)) as u8;
            }
            pos += 10;
        }
    }
    pos
}

// ── MMIO helpers ─────────────────────────────────────────────────────────────

fn sys_map_mmio(phys: usize, size: usize) -> Result<usize, &'static str> {
    let thread = crate::sched::current_thread().ok_or("no thread")?;
    let pid = unsafe { (*thread).owner };

    let procs = crate::proc::PROCESSES.lock();
    let mut root_pt = 0;
    for proc in procs.iter() {
        if proc.pid == pid { root_pt = proc.page_table_root; break; }
    }
    drop(procs);

    if root_pt == 0 {
        crate::println!("  MMIO: process pid={} not found!", pid);
        return Err("process not found");
    }

    crate::println!("  MMIO: pid={} root_pt=0x{:x}", pid, root_pt);
    let va = crate::proc::elf::map_phys_to_user(root_pt, phys, size);
    crate::println!("  MMIO: mapped at va=0x{:x}", va);
    Ok(va)
}

fn sys_mmio_read32(phys: usize) -> Result<usize, &'static str> {
    if phys & 0x3 != 0 { return Err("unaligned mmio read"); }
    unsafe {
        extern "C" {
            static __mmio_fault_happened: core::cell::UnsafeCell<usize>;
            fn __mmio_fault_recover();
        }
        core::ptr::write_volatile(__mmio_fault_happened.get(), 0);
        let old_stvec: usize;
        core::arch::asm!("csrr {}, stvec", out(reg) old_stvec);
        core::arch::asm!("csrw stvec, {}", in(reg) __mmio_fault_recover as *const () as usize);
        let val = (phys as *const u32).read_volatile();
        core::arch::asm!("csrw stvec, {}", in(reg) old_stvec);
        if core::ptr::read_volatile(__mmio_fault_happened.get()) != 0 {
            Err("mmio load access fault")
        } else {
            Ok(val as usize)
        }
    }
}

fn sys_mmio_write32(phys: usize, val: usize) -> Result<usize, &'static str> {
    if phys & 0x3 != 0 { return Err("unaligned mmio write"); }
    unsafe { (phys as *mut u32).write_volatile(val as u32); }
    Ok(0)
}

// ── MMIO fault recovery ──────────────────────────────────────────────────────

core::arch::global_asm!(
    ".data",
    ".align 3",
    ".globl __mmio_fault_happened",
    "__mmio_fault_happened:",
    "    .quad 0",
    ".text",
    ".globl __mmio_fault_recover",
    ".align 2",
    "__mmio_fault_recover:",
    "    li t0, 1",
    "    la t1, __mmio_fault_happened",
    "    sd t0, 0(t1)",
    "    csrr t0, sepc",
    "    addi t0, t0, 4",
    "    csrw sepc, t0",
    "    sret",
);
