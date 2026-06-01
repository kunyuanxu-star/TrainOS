pub mod cap;
pub mod ipc;
pub mod posix;
pub mod proc;
pub mod memory;
pub mod socket;
pub mod epoll;
pub mod time;
pub mod fs;
pub mod ioflags;

use crate::trap::TrapFrame;

// V21.12: Syscall statistics counters (512 entries for up to syscall 511)
pub static mut SYSCALL_COUNTERS: [u64; 512] = [0u64; 512];

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
pub const SYS_REMOTE_PROBE: usize = 184;
pub const SYS_REMOTE_RECV: usize = 185;
pub const SYS_REMOTE_MEM_ALLOC: usize = 186;
pub const SYS_REMOTE_MEM_FREE: usize = 187;
pub const SYS_REMOTE_PROCLIST: usize = 188;
pub const SYS_REMOTE_MINT: usize = 189;
pub const SYS_REMOTE_MIGRATE_PAGE: usize = 190;
pub const SYS_NODE_ID: usize = 191;


// ASLR/Cheri (V27): 200-205
pub const SYS_ASLR_INIT: usize = 200;
pub const SYS_CHERI_CAP_CREATE: usize = 201;
pub const SYS_CHERI_CAP_CHECK: usize = 202;
pub const SYS_SANDBOX_ADD: usize = 203;
pub const SYS_SANDBOX_CHECK: usize = 204;
pub const SYS_CHERI_CAP_DELETE: usize = 205;
pub const SYS_SANDBOX_NET_ADD: usize = 206;
pub const SYS_SANDBOX_UID_MAP: usize = 207;
pub const SYS_ASLR_ENTROPY: usize = 208;
pub const SYS_CHERI_STATUS: usize = 209;

// WASM (V28): 210-215
pub const SYS_WASM_LOAD: usize = 210;
pub const SYS_WASM_UNLOAD: usize = 211;
pub const SYS_WASM_LIST: usize = 212;
pub const SYS_WASM_EXECUTE: usize = 213;
pub const SYS_WASM_MEM_READ: usize = 214;
pub const SYS_WASM_MEM_WRITE: usize = 215;

// AI/GPU (V29): 220-239
pub const SYS_GPU_REGISTER: usize = 220;
pub const SYS_GPU_LIST: usize = 221;
pub const SYS_AI_SUBMIT: usize = 222;
pub const SYS_AI_NEXT: usize = 223;
pub const SYS_GPU_SUBMIT_CMD: usize = 224;
pub const SYS_GPU_WAIT_FENCE: usize = 225;
pub const SYS_GPU_ALLOC: usize = 226;
pub const SYS_GPU_FREE: usize = 227;
pub const SYS_GPU_UTILIZATION: usize = 228;
pub const SYS_GPU_ACTIVE_WL: usize = 229;
pub const SYS_AI_COMPLETE: usize = 230;
pub const SYS_AI_PREEMPT: usize = 231;
pub const SYS_MODEL_LOAD: usize = 232;
pub const SYS_MODEL_UNLOAD: usize = 233;
pub const SYS_MODEL_LIST: usize = 234;
pub const SYS_INFERENCE_SUBMIT: usize = 235;
pub const SYS_INFERENCE_STATS: usize = 236;

// Linux Compat (V30): 300-302
pub const SYS_COMPAT_INIT: usize = 300;
pub const SYS_COMPAT_TRANSLATE: usize = 301;
pub const SYS_COMPAT_SETUP_AUXV: usize = 302;

// V30 POSIX Compliance: 240-283
// System V IPC — Semaphores
pub const SYS_SEMGET: usize = 240;
pub const SYS_SEMOP: usize = 241;
pub const SYS_SEMCTL: usize = 242;

// System V IPC — Message queues
pub const SYS_MSGGET: usize = 243;
pub const SYS_MSGSND: usize = 244;
pub const SYS_MSGRCV: usize = 245;
pub const SYS_MSGCTL: usize = 246;

// Signals
pub const SYS_SIGACTION: usize = 247;
pub const SYS_SIGPROCMASK: usize = 248;
pub const SYS_SIGRETURN: usize = 249;
pub const SYS_RT_SIGACTION: usize = 250;
pub const SYS_RT_SIGPROCMASK: usize = 251;
pub const SYS_SIGPENDING: usize = 252;

// Filesystem
pub const SYS_SYMLINK: usize = 253;
pub const SYS_READLINK: usize = 254;
pub const SYS_FSYNC: usize = 255;
pub const SYS_FDATASYNC: usize = 256;
pub const SYS_FLOCK: usize = 257;
pub const SYS_FALLOCATE: usize = 258;
pub const SYS_SENDFILE: usize = 259;

// Process
pub const SYS_PRCTL: usize = 260;
pub const SYS_GETPRIORITY: usize = 261;
pub const SYS_SETPRIORITY: usize = 262;
pub const SYS_SCHED_GETPARAM: usize = 263;
pub const SYS_SCHED_SETPARAM: usize = 264;

// Memory
pub const SYS_MADVISE: usize = 265;
pub const SYS_MINCORE: usize = 266;
pub const SYS_MLOCK: usize = 267;
pub const SYS_MUNLOCK: usize = 268;

// Time
pub const SYS_SETTIMEOFDAY: usize = 269;
pub const SYS_TIMER_CREATE: usize = 270;
pub const SYS_TIMER_DELETE: usize = 271;
pub const SYS_TIMER_SETTIME: usize = 272;
pub const SYS_TIMER_GETTIME: usize = 273;

// Socket
pub const SYS_GETSOCKOPT: usize = 274;
pub const SYS_SETSOCKOPT: usize = 275;
pub const SYS_GETPEERNAME: usize = 276;
pub const SYS_GETSOCKNAME: usize = 277;
pub const SYS_SHUTDOWN: usize = 278;

// Poll/Select
pub const SYS_POLL: usize = 279;
pub const SYS_PPOLL: usize = 280;
pub const SYS_PSELECT6: usize = 281;

// V35 — Linux Scheduling & IPC: 296-299
pub const SYS_SCHED_SETPREEMPT: usize = 296;  // Set preemption mode
pub const SYS_SET_SLICE_EXT: usize = 297;      // Enable/disable time slice extension

// V34 — AI-Native Scheduling: 282-295
pub const SYS_PD_SUBMIT: usize = 282;       // Submit P/D workload pair
pub const SYS_PD_NEXT_DECODE: usize = 283;  // Get next decode step
pub const SYS_PD_NEXT_PREFILL: usize = 284; // Get next prefill batch
pub const SYS_PD_PREEMPT: usize = 285;      // Preempt a decode workload
pub const SYS_PD_RESUME: usize = 286;       // Resume a decode workload
pub const SYS_KV_ALLOC: usize = 287;        // Allocate KV-cache pages
pub const SYS_KV_FREE: usize = 288;         // Free KV-cache pages
pub const SYS_KV_SHARE: usize = 289;        // Share KV-cache pages
pub const SYS_KV_STATS: usize = 290;        // Get KV-cache statistics
pub const SYS_GPU_HETERO_SCHED: usize = 291; // GPU-CPU heterogeneous scheduling
pub const SYS_GPU_MIGRATE: usize = 292;     // Migrate workload to different GPU
pub const SYS_GPU_BALANCE: usize = 293;     // Balance GPU load
pub const SYS_AI_SCHED_STATS: usize = 294;  // Get AI scheduling stats
pub const SYS_AI_SCHED_RESET: usize = 295;  // Reset AI scheduling stats

// V35 — Linux Feature Parity (I/O & Filesystem): 305-309
pub const SYS_READV2: usize = 305;           // Extended read with flags (preadv2)
pub const SYS_WRITEV2: usize = 306;          // Extended write with flags (pwritev2)
pub const SYS_CACHESTAT: usize = 307;        // Page cache statistics
pub const SYS_IORING_PROVIDE_BUFFERS: usize = 308;  // Pre-register buffer pool
pub const SYS_IORING_REMOVE_BUFFERS: usize = 309;   // Remove buffer pool

// V35 — Memory & Security (V35a): 301
pub const SYS_MSEAL: usize = 301;                    // Memory sealing (mseal)

// V36a — RVV 1.0 Vector Extension: 310-311
pub const SYS_CAP_VECTOR_ENABLE: usize = 310;        // Grant vector capability
pub const SYS_VECTOR_STATS: usize = 311;             // Read vector statistics

// V37b — GUI syscalls: 350-359
pub const SYS_FB_INFO: usize = 350;                  // Get framebuffer info
pub const SYS_FB_FLUSH: usize = 351;                 // Flush framebuffer
pub const SYS_INPUT_POLL: usize = 352;               // Poll for input events
pub const SYS_INPUT_WAIT: usize = 353;               // Wait (block) for input event
pub const SYS_FB_MAP_PAGE: usize = 354;              // Map a framebuffer page into process
pub const SYS_GUI_REDRAW: usize = 355;               // Redraw all windows
pub const SYS_GUI_CREATE_WINDOW: usize = 356;        // Create a window (kernel-managed)

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
        SYS_BLK_WRITE => proc::sys_blk_write(arg0, arg1, arg2, arg3),
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
        SYS_CHERI_CAP_CHECK => proc::sys_cheri_cap_check(arg0, arg1, arg2 as u16),
        SYS_SANDBOX_ADD => proc::sys_sandbox_add(arg0, arg1),
        SYS_SANDBOX_CHECK => proc::sys_sandbox_check(arg0, arg1),
        SYS_CHERI_CAP_DELETE => proc::sys_cheri_cap_delete(arg0 as u32, arg1 as u8),
        SYS_SANDBOX_NET_ADD => proc::sys_sandbox_net_add(arg0 as u32, arg1 as u16, arg2 as u16, arg3),
        SYS_SANDBOX_UID_MAP => proc::sys_sandbox_uid_map(arg0 as u32, arg1 as u32, arg2 as u32),
        SYS_ASLR_ENTROPY => proc::sys_aslr_entropy(),
        SYS_CHERI_STATUS => proc::sys_cheri_status(arg0, arg1),

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
        SYS_GPU_SUBMIT_CMD => proc::sys_gpu_submit_cmd(arg0 as u32, arg1, arg2),
        SYS_GPU_WAIT_FENCE => proc::sys_gpu_wait_fence(arg0 as u32, arg1 as u64),
        SYS_GPU_ALLOC => proc::sys_gpu_alloc(arg0 as u32, arg1),
        SYS_GPU_FREE => proc::sys_gpu_free(arg0 as u32, arg1),
        SYS_GPU_UTILIZATION => proc::sys_gpu_utilization(arg0 as u32),
        SYS_GPU_ACTIVE_WL => proc::sys_gpu_active_wl(arg0 as u32),
        SYS_AI_COMPLETE => proc::sys_ai_complete(arg0, arg1),
        SYS_AI_PREEMPT => proc::sys_ai_preempt(arg0),
        SYS_MODEL_LOAD => proc::sys_model_load(arg0 as u32, arg1, arg2),
        SYS_MODEL_UNLOAD => proc::sys_model_unload(arg0 as u32),
        SYS_MODEL_LIST => proc::sys_model_list(arg0, arg1),
        SYS_INFERENCE_SUBMIT => proc::sys_inference_submit(arg0 as u32, arg1 as u64, arg2 as u64),
        SYS_INFERENCE_STATS => proc::sys_inference_stats(arg0 as u32, arg1),

        // V30 — System V Semaphores
        SYS_SEMGET => proc::sys_semget(arg0 as u32, arg1, arg2),
        SYS_SEMOP => proc::sys_semop(arg0 as u32, arg1, arg2),
        SYS_SEMCTL => proc::sys_semctl(arg0 as u32, arg1 as u32, arg2, arg3),

        // V30 — System V Message Queues
        SYS_MSGGET => proc::sys_msgget(arg0 as u32, arg1),
        SYS_MSGSND => proc::sys_msgsnd(arg0 as u32, arg1, arg2, arg3),
        SYS_MSGRCV => proc::sys_msgrcv(arg0 as u32, arg1, arg2, arg3 as i64, tf.a4),
        SYS_MSGCTL => proc::sys_msgctl(arg0 as u32, arg1 as u32, arg2),

        // V30 — Signals
        SYS_SIGACTION => proc::sys_sigaction(arg0 as u32, arg1, arg2),
        SYS_SIGPROCMASK => proc::sys_sigprocmask(arg0 as u32, arg1, arg2),
        SYS_SIGRETURN => proc::sys_sigreturn(),
        SYS_RT_SIGACTION => proc::sys_sigaction(arg0 as u32, arg1, arg2),
        SYS_RT_SIGPROCMASK => proc::sys_sigprocmask(arg0 as u32, arg1, arg2),
        SYS_SIGPENDING => proc::sys_sigpending(arg0),

        // V30 — Filesystem
        SYS_SYMLINK => fs::sys_symlink(arg0, arg1),
        SYS_READLINK => fs::sys_readlink(arg0, arg1, arg2),
        SYS_FSYNC => fs::sys_fsync(arg0),
        SYS_FDATASYNC => fs::sys_fdatasync(arg0),
        SYS_FLOCK => fs::sys_flock(arg0, arg1),
        SYS_FALLOCATE => fs::sys_fallocate(arg0, arg1, arg2, arg3),
        SYS_SENDFILE => fs::sys_sendfile(arg0, arg1, arg2, arg3),

        // V30 — Process
        SYS_PRCTL => proc::sys_prctl(arg0, arg1, arg2),
        SYS_GETPRIORITY => proc::sys_getpriority(arg0, arg1),
        SYS_SETPRIORITY => proc::sys_setpriority(arg0, arg1, arg2),
        SYS_SCHED_GETPARAM => proc::sys_sched_getparam(arg0 as u32, arg1),
        SYS_SCHED_SETPARAM => proc::sys_sched_setparam(arg0 as u32, arg1),

        // V30 — Memory
        SYS_MADVISE => memory::sys_madvise(arg0, arg1, arg2),
        SYS_MINCORE => memory::sys_mincore(arg0, arg1, arg2),
        SYS_MLOCK => memory::sys_mlock(arg0, arg1),
        SYS_MUNLOCK => memory::sys_munlock(arg0, arg1),

        // V30 — Time
        SYS_SETTIMEOFDAY => time::sys_settimeofday(arg0, arg1),
        SYS_TIMER_CREATE => time::sys_timer_create(arg0, arg1, arg2),
        SYS_TIMER_DELETE => time::sys_timer_delete(arg0),
        SYS_TIMER_SETTIME => time::sys_timer_settime(arg0, arg1, arg2, arg3),
        SYS_TIMER_GETTIME => time::sys_timer_gettime(arg0, arg1),

        // V30 — Socket
        SYS_GETSOCKOPT => socket::sys_getsockopt(arg0, arg1, arg2, arg3, tf.a4),
        SYS_SETSOCKOPT => socket::sys_setsockopt(arg0, arg1, arg2, arg3, tf.a4),
        SYS_GETPEERNAME => socket::sys_getpeername(arg0, arg1, arg2),
        SYS_GETSOCKNAME => socket::sys_getsockname(arg0, arg1, arg2),
        SYS_SHUTDOWN => socket::sys_shutdown(arg0, arg1),

        // V30 — Poll/Select
        SYS_POLL => proc::sys_poll(arg0, arg1, arg2 as isize),
        SYS_PPOLL => proc::sys_ppoll(arg0, arg1, arg2, arg3),
        SYS_PSELECT6 => proc::sys_pselect6(arg0, arg1, arg2, arg3, tf.a4),

        // V30 — Linux compat
        SYS_COMPAT_INIT => proc::sys_compat_init(),
        SYS_COMPAT_TRANSLATE => proc::sys_compat_translate(arg0),
        SYS_COMPAT_SETUP_AUXV => proc::sys_compat_setup_auxv(arg0, arg1, arg2, arg3, tf.a4 as usize),

        // V26 — Distributed
        SYS_REMOTE_NODE_ADD => proc::sys_remote_node_add(arg0, arg1),
        SYS_REMOTE_EP_PUBLISH => proc::sys_remote_ep_publish(arg0, arg1, arg2),
        SYS_REMOTE_EP_LOOKUP => proc::sys_remote_ep_lookup(arg0, arg1),
        SYS_REMOTE_SEND => proc::sys_remote_send(arg0 as u32, arg1, arg2, arg3),
        SYS_REMOTE_PROBE => proc::sys_remote_probe(arg0 as u32),
        SYS_REMOTE_RECV => proc::sys_remote_recv(arg0, arg1),
        SYS_REMOTE_MEM_ALLOC => proc::sys_remote_mem_alloc(arg0 as u8, arg1),
        SYS_REMOTE_MEM_FREE => proc::sys_remote_mem_free(arg0 as u64),
        SYS_REMOTE_PROCLIST => proc::sys_remote_proclist(arg0 as u32, arg1, arg2),
        SYS_REMOTE_MINT => proc::sys_remote_mint(arg0 as u32, arg1 as u32, arg2 as u32),
        SYS_REMOTE_MIGRATE_PAGE => proc::sys_remote_migrate_page(arg0, arg1 as u8, arg2 as u8),
        SYS_NODE_ID => proc::sys_node_id(),

        // V34 — AI Scheduling
        SYS_PD_SUBMIT => proc::sys_pd_submit(arg0, arg1, arg2 as u32, arg3 as u32),
        SYS_PD_NEXT_DECODE => proc::sys_pd_next_decode(),
        SYS_PD_NEXT_PREFILL => proc::sys_pd_next_prefill(arg0, arg1),
        SYS_PD_PREEMPT => proc::sys_pd_preempt(arg0),
        SYS_PD_RESUME => proc::sys_pd_resume(arg0),
        SYS_KV_ALLOC => proc::sys_kv_alloc(arg0),
        SYS_KV_FREE => proc::sys_kv_free(arg0, arg1),
        SYS_KV_SHARE => proc::sys_kv_share(arg0, arg1),
        SYS_KV_STATS => proc::sys_kv_stats(arg0),
        SYS_GPU_HETERO_SCHED => proc::sys_gpu_hetero_sched(arg0 as u32, arg1),
        SYS_GPU_MIGRATE => proc::sys_gpu_migrate(arg0, arg1 as u32),
        SYS_GPU_BALANCE => proc::sys_gpu_balance(),
        SYS_AI_SCHED_STATS => proc::sys_ai_sched_stats(arg0),
        SYS_AI_SCHED_RESET => proc::sys_ai_sched_reset(),

        // V35 — Scheduling & IPC
        SYS_SCHED_SETPREEMPT => {
            crate::sched::sys_sched_setpreempt(arg0 as u32, arg1)
        }
        SYS_SET_SLICE_EXT => crate::sched::sys_set_slice_ext(arg0 != 0),

        // V35a — Memory & Security
        SYS_MSEAL => memory::sys_mseal(arg0, arg1),

        // V35 — Linux Feature Parity (I/O & Filesystem)
        SYS_READV2 => ioflags::sys_readv2(
            arg0 as u32,
            arg1 as *mut u8,
            arg2,
            arg3 as isize as i64,
            tf.a4 as u32,
        ),
        SYS_WRITEV2 => ioflags::sys_writev2(
            arg0 as u32,
            arg1 as *const u8,
            arg2,
            arg3 as isize as i64,
            tf.a4 as u32,
        ),
        SYS_CACHESTAT => ioflags::sys_cachestat(
            arg0 as u32,
            arg1,
            arg2,
            arg3,
        ),
        SYS_IORING_PROVIDE_BUFFERS => {
            let pages_ptr = arg2;
            let pages_count = tf.a4;
            if pages_ptr == 0 { Err("null pages") }
            else {
                let mut pages = [0usize; 16];
                let count = pages_count.min(16);
                unsafe {
                    for i in 0..count {
                        pages[i] = (pages_ptr as *const u32).add(i).read_volatile() as usize;
                    }
                }
                crate::iouring::provide_buffers(
                    arg0,
                    arg1 as u16,
                    &pages[..count],
                    arg3,
                );
                Ok(0)
            }
        }
        SYS_IORING_REMOVE_BUFFERS => {
            crate::iouring::remove_buffers(arg0, arg1 as u16);
            Ok(0)
        }

        // V21 — Security
        SYS_SECCOMP_ADD => proc::sys_seccomp_add(arg0 as u32, arg1),
        SYS_CAP_AUDIT => proc::sys_cap_audit(arg0, arg1),
        SYS_SYSCALL_STATS => proc::sys_syscall_stats(arg0, arg1),

        // V36a — RVV 1.0 Vector Extension
        SYS_CAP_VECTOR_ENABLE => proc::sys_cap_vector_enable(arg0 as u32),
        SYS_VECTOR_STATS => proc::sys_vector_stats(arg0, arg1),

        // V37b — GUI syscalls
        SYS_FB_INFO => {
            let pid = crate::sched::current_thread()
                .map(|t| unsafe { (*t).owner })
                .unwrap_or(0);
            if pid == 0 { Err("no process") }
            // arg0 = user buffer pointer, arg1 = buffer size
            else if arg0 == 0 { Err("null buffer") }
            else {
                let buf = unsafe { core::slice::from_raw_parts_mut(arg0 as *mut u8, arg1) };
                crate::device::gui::sys_fb_info(buf)
            }
        }
        SYS_FB_FLUSH => crate::device::gui::sys_fb_flush(),
        SYS_INPUT_POLL => {
            if arg0 == 0 { Err("null buffer") }
            else {
                let buf = unsafe { core::slice::from_raw_parts_mut(arg0 as *mut u8, arg1) };
                crate::device::gui::sys_input_poll(buf)
            }
        }
        SYS_INPUT_WAIT => {
            if arg0 == 0 { Err("null buffer") }
            else {
                let buf = unsafe { core::slice::from_raw_parts_mut(arg0 as *mut u8, arg1) };
                crate::device::gui::sys_input_wait(buf)
            }
        }
        SYS_FB_MAP_PAGE => {
            let pid = crate::sched::current_thread()
                .map(|t| unsafe { (*t).owner })
                .unwrap_or(0);
            if pid == 0 { Err("no process") }
            else { crate::device::gui::sys_fb_map_page(pid, arg0 as u32) }
        }
        SYS_GUI_REDRAW => {
            crate::device::gui::gui_redraw();
            Ok(0)
        }
        SYS_GUI_CREATE_WINDOW => {
            // arg0 = title ptr, arg1 = title_len, arg2 = x, arg3 = y, a4 = w, a5 = h
            let pid = crate::sched::current_thread()
                .map(|t| unsafe { (*t).owner })
                .unwrap_or(0);
            if pid == 0 || arg0 == 0 { Err("invalid args") }
            else {
                let title = unsafe {
                    let slice = core::slice::from_raw_parts(arg0 as *const u8, arg1.min(64));
                    core::str::from_utf8_unchecked(slice)
                };
                if let Some(wm) = crate::device::gui::wm() {
                    wm.create_window(title, arg2 as i32, arg3 as i32, tf.a4 as u32, tf.a5 as u32, pid)
                        .map(|id| id)
                        .ok_or("window create failed")
                } else {
                    Err("window manager not initialized")
                }
            }
        }

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

// ── V32: Syscall dispatch for WASM host calls ────────────────────────────
/// Dispatch a syscall from WASM module context.
/// Creates a minimal TrapFrame and delegates to the main dispatch.
pub fn syscall_dispatch_wasm(nr: usize, args: &[usize; 6], _pid: u32) -> isize {
    use crate::trap::TrapFrame;
    let mut tf = TrapFrame {
        a0: args[0],
        a1: args[1],
        a2: args[2],
        a3: args[3],
        a4: args[4],
        a5: args[5],
        a6: 0,
        a7: nr,
        ..unsafe { core::mem::zeroed() }
    };
    syscall_dispatch(&mut tf);
    tf.a0 as isize
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
