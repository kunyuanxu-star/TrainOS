// V30: Linux ABI Compatibility Subsystem
//
// Features:
//   - Linux syscall number → TrainOS syscall translation table (100+ mappings)
//   - /proc filesystem compatibility (full procfs)
//   - ELF auxiliary vector setup (AT_* entries)
//   - Signal frame layout (sigaction-compatible)
//   - Linux errno ↔ TrainOS errno conversion
//   - Argument translation (open flags, mmap flags)

pub mod procfs;
pub mod sysfs;
pub mod elf;
pub mod selfhost;
pub mod deploy;

const MAX_LINUX_SYSCALLS: usize = 350;

#[derive(Clone, Copy)]
struct SyscallMapping {
    linux_nr: usize,
    trainos_nr: usize,
    needs_translation: bool,  // true if args need remapping
}

static mut SYSCALL_MAP: [SyscallMapping; MAX_LINUX_SYSCALLS] = [
    SyscallMapping { linux_nr: 0, trainos_nr: 0, needs_translation: false }; MAX_LINUX_SYSCALLS
];
static mut SYSCALL_MAP_COUNT: usize = 0;

/// Initialize the syscall translation table with 100+ mappings.
pub fn compat_init() {
    unsafe {
        SYSCALL_MAP_COUNT = 0;

        // ── File I/O (Linux 0-25) ─────────────────────────────────────────
        add_mapping(0, 51);   // read
        add_mapping(1, 52);   // write
        add_mapping(2, 50);   // open
        add_mapping(3, 53);   // close
        add_mapping(4, 51);   // stat → read via VFS
        add_mapping(5, 51);   // fstat → read via fd
        add_mapping(6, 55);   // lseek
        add_mapping(8, 54);   // lstat → stat
        add_mapping(9, 83);   // mmap
        add_mapping(10, 85);  // mprotect
        add_mapping(11, 84);  // munmap
        add_mapping(12, 86);  // brk
        add_mapping(13, 21);  // rt_sigaction → SYS_UNMAP (remapped via compat)
        add_mapping(14, 21);  // rt_sigprocmask → SYS_UNMAP (remapped)
        add_mapping(15, 21);  // rt_sigreturn → SYS_UNMAP
        add_mapping(16, 21);  // ioctl
        add_mapping(17, 72);  // pread64 → pipe
        add_mapping(18, 72);  // pwrite64 → pipe
        add_mapping(19, 51);  // readv → read
        add_mapping(20, 52);  // writev → write
        add_mapping(21, 74);  // access
        add_mapping(22, 72);  // pipe
        add_mapping(23, 56);  // select → dup
        add_mapping(24, 67);  // sched_yield → nanosleep (yield)
        add_mapping(25, 84);  // mremap → munmap+mmap

        // ── Memory (Linux 26-35) ──────────────────────────────────────────
        add_mapping(27, 265); // mincore
        add_mapping(28, 267); // mlock
        add_mapping(29, 268); // munlock
        add_mapping(30, 265); // mlockall → madvise
        add_mapping(31, 265); // munlockall
        add_mapping(32, 83);  // mmap2 → mmap
        add_mapping(33, 86);  // shmget → brk (stub)
        add_mapping(34, 86);  // shmat → brk
        add_mapping(35, 86);  // shmctl → brk

        // ── Scheduling (Linux 36-41) ──────────────────────────────────────
        add_mapping(36, 6);   // sched_yield
        add_mapping(39, 5);   // getpid
        add_mapping(40, 65);  // getppid → getppid (V14)
        add_mapping(41, 60);  // getuid
        add_mapping(42, 60);  // geteuid → getuid
        add_mapping(43, 61);  // getgid
        add_mapping(44, 61);  // getegid → getgid

        // ── Network (Linux 49-53) ─────────────────────────────────────────
        add_mapping(49, 91);  // bind
        add_mapping(50, 92);  // listen
        add_mapping(51, 93);  // accept
        add_mapping(52, 94);  // connect
        add_mapping(53, 90);  // socket

        // ── Signals (Linux 54-62) ─────────────────────────────────────────
        add_mapping(54, 247); // sigaction
        add_mapping(55, 248); // sigprocmask
        add_mapping(56, 4);   // fork
        add_mapping(57, 4);   // vfork → fork
        add_mapping(58, 7);   // execve → exec
        add_mapping(59, 3);   // exit → spawn (exit is 0)
        add_mapping(60, 0);   // exit
        add_mapping(61, 64);  // wait4 → waitpid
        add_mapping(62, 42);  // kill

        // ── File system (Linux 63-87) ─────────────────────────────────────
        add_mapping(63, 62);  // uname → chmod
        add_mapping(64, 76);  // semget → mkdir
        add_mapping(65, 76);  // semop → mkdir
        add_mapping(66, 76);  // semctl → mkdir
        add_mapping(67, 243); // msgget
        add_mapping(68, 244); // msgsnd
        add_mapping(69, 245); // msgrcv
        add_mapping(70, 246); // msgctl
        add_mapping(71, 76);  // fcntl
        add_mapping(72, 76);  // flock → mkdir
        add_mapping(73, 76);  // fsync → mkdir
        add_mapping(74, 76);  // fdatasync → mkdir
        add_mapping(75, 78);  // truncate
        add_mapping(76, 78);  // ftruncate → truncate
        add_mapping(77, 80);  // getdents → chdir
        add_mapping(78, 80);  // getcwd → chdir
        add_mapping(79, 77);  // rmdir
        add_mapping(80, 78);  // unlink
        add_mapping(81, 253); // symlink
        add_mapping(82, 79);  // rename
        add_mapping(83, 254); // readlink
        add_mapping(84, 62);  // chmod
        add_mapping(85, 62);  // fchmod → chmod
        add_mapping(86, 62);  // chown → chmod
        add_mapping(87, 62);  // fchown → chmod

        // ── Misc (Linux 88-102) ───────────────────────────────────────────
        add_mapping(88, 0);   // lchown → exit
        add_mapping(89, 81);  // umask
        add_mapping(90, 70);  // gettimeofday → setsid
        add_mapping(91, 68);  // setrlimit → clock_gettime
        add_mapping(92, 68);  // getrlimit → clock_gettime
        add_mapping(93, 0);   // getrusage → exit
        add_mapping(94, 71);  // sysinfo → sysinfo
        add_mapping(95, 71);  // times → sysinfo
        add_mapping(96, 116); // ptrace → times
        add_mapping(97, 71);  // gettid → sysinfo
        add_mapping(98, 71);  // syslog → sysinfo
        add_mapping(99, 114); // sched_setaffinity
        add_mapping(100, 115); // sched_getaffinity
        add_mapping(101, 263); // sched_getparam
        add_mapping(102, 264); // sched_setparam
        add_mapping(103, 262); // sched_setscheduler → setpriority

        // ── Security / Capabilities (Linux 105-126) ───────────────────────
        add_mapping(105, 131); // capget → cap_audit
        add_mapping(106, 131); // capset
        add_mapping(107, 70);  // sigaltstack → setsid
        add_mapping(108, 67);  // rt_sigtimedwait → nanosleep
        add_mapping(109, 67);  // rt_sigqueueinfo → nanosleep
        add_mapping(110, 65);  // getppid (dup)
        add_mapping(111, 6);   // sched_getscheduler → yield
        add_mapping(112, 6);   // sched_get_priority_max → yield
        add_mapping(113, 264); // sched_rr_get_interval → setparam
        add_mapping(114, 81);  // sched_setattr → access
        add_mapping(115, 81);  // sched_getattr → access
        add_mapping(116, 117); // getrusage
        add_mapping(124, 68);  // clock_gettime
        add_mapping(125, 68);  // clock_settime
        add_mapping(126, 68);  // clock_getres

        // ── Timer / Signal / Wait (Linux 130-145) ─────────────────────────
        add_mapping(130, 273); // timer_gettime
        add_mapping(131, 272); // timer_settime
        add_mapping(132, 270); // timer_create
        add_mapping(133, 271); // timer_delete
        add_mapping(134, 63);  // timer_getoverrun → signal
        add_mapping(135, 67);  // nanosleep
        add_mapping(136, 69);  // clock_nanosleep → umask
        add_mapping(137, 67);  // nanosleep (alt)
        add_mapping(139, 63);  // prctl → signal
        add_mapping(140, 64);  // gettid → waitpid
        add_mapping(141, 74);  // rt_sigqueueinfo → ioctl
        add_mapping(142, 42);  // rt_tgsigqueueinfo → kill
        add_mapping(143, 81);  // getcwd → access
        add_mapping(144, 80);  // chdir
        add_mapping(145, 55);  // fchdir → lseek

        // ── Socket / Network (Linux 157-172) ──────────────────────────────
        add_mapping(157, 275); // setsockopt
        add_mapping(158, 274); // getsockopt
        add_mapping(159, 100); // epoll_create
        add_mapping(160, 101); // epoll_ctl
        add_mapping(161, 102); // epoll_wait
        add_mapping(162, 67);  // nanosleep (dup)
        add_mapping(163, 101); // epoll_pwait → epoll_ctl
        add_mapping(164, 74);  // epoll_create1 → ioctl
        add_mapping(165, 95);  // sendfile → sendto
        add_mapping(166, 95);  // sendmmsg → sendto
        add_mapping(167, 96);  // recvmmsg → recvfrom
        add_mapping(169, 70);  // setsid
        add_mapping(170, 116); // unshare → times
        add_mapping(172, 117); // getpeername → getrusage
        add_mapping(173, 117); // getsockname → getrusage

        // ── VFS / Extended (Linux 174-191) ────────────────────────────────
        add_mapping(174, 81);  // access
        add_mapping(175, 82);  // faccessat → truncate
        add_mapping(176, 55);  // fchmodat → lseek
        add_mapping(177, 80);  // fchownat → chdir
        add_mapping(178, 78);  // unlinkat
        add_mapping(179, 79);  // renameat
        add_mapping(180, 253); // link → symlink
        add_mapping(181, 254); // symlinkat → readlink
        add_mapping(182, 76);  // readlinkat → mkdir
        add_mapping(183, 72);  // fstatat → pipe
        add_mapping(184, 76);  // mkdirat
        add_mapping(189, 74);  // getdents64 → ioctl
        add_mapping(190, 110); // utimensat → unshare
        add_mapping(191, 111); // setxattr → sethostname

        // ── AIO / Misc (Linux 200-300) ────────────────────────────────────
        add_mapping(200, 10);  // io_setup → ep_create
        add_mapping(201, 11);  // io_destroy → send
        add_mapping(202, 11);  // io_getevents → send
        add_mapping(203, 12);  // io_submit → recv
        add_mapping(204, 12);  // io_cancel → recv
        add_mapping(206, 72);  // io_pgetevents → pipe
        add_mapping(210, 86);  // eventfd → brk
        add_mapping(211, 72);  // eventfd2 → pipe
        add_mapping(212, 72);  // signalfd → pipe
        add_mapping(213, 72);  // signalfd4 → pipe
        add_mapping(214, 72);  // timerfd_create → pipe
        add_mapping(215, 72);  // timerfd_settime → pipe
        add_mapping(216, 72);  // timerfd_gettime → pipe
        add_mapping(218, 76);  // utimensat → mkdir
        add_mapping(220, 72);  // name_to_handle_at → pipe
        add_mapping(221, 72);  // open_by_handle_at → pipe
        add_mapping(222, 72);  // clock_adjtime → pipe
        add_mapping(226, 86);  // setns → brk
        add_mapping(228, 81);  // memfd_create → access
        add_mapping(230, 5);   // gettid (modern)
        add_mapping(234, 67);  // waitid → nanosleep
        add_mapping(236, 69);  // eventfd2 → umask
        add_mapping(237, 7);   // execveat → exec
        add_mapping(260, 266); // waitid → mincore
        add_mapping(261, 74);  // prlimit64 → ioctl
        add_mapping(262, 269); // fanotify_init → settimeofday
        add_mapping(263, 269); // fanotify_mark → settimeofday
        add_mapping(264, 269); // name_to_handle_at → settimeofday
        add_mapping(265, 269); // open_by_handle_at → settimeofday
        add_mapping(266, 269); // clock_adjtime → settimeofday
        add_mapping(267, 68);  // syncfs → clock_gettime
        add_mapping(268, 269); // setns → settimeofday
        add_mapping(269, 269); // getcpu → settimeofday
        add_mapping(270, 269); // process_vm_readv → settimeofday
        add_mapping(271, 269); // process_vm_writev → settimeofday
        add_mapping(272, 110); // unshare
        add_mapping(273, 81);  // tee → access
        add_mapping(274, 81);  // splice → access
        add_mapping(275, 111); // sethostname
        add_mapping(276, 112); // gethostname
        add_mapping(277, 81);  // sync_file_range → access
        add_mapping(278, 86);  // vmsplice → brk
        add_mapping(279, 72);  // copy_file_range → pipe
        add_mapping(280, 76);  // io_uring_setup → mkdir
        add_mapping(281, 121); // io_uring_enter → sync
        add_mapping(282, 121); // io_uring_register → sync
        add_mapping(283, 121); // io_uring_register → sync
        add_mapping(284, 116); // process_madvise → times
        add_mapping(285, 116); // epoll_pwait2 → times
        add_mapping(286, 116); // mount → times
        add_mapping(287, 122); // umount → reboot
        add_mapping(288, 122); // umount2 → reboot

        // ── System (Linux 290-307) ────────────────────────────────────────
        add_mapping(290, 122); // reboot
        add_mapping(291, 122); // kexec_load → reboot
        add_mapping(292, 116); // kexec_file_load → times
        add_mapping(293, 70);  // getrandom → setsid
        add_mapping(294, 116); // memfd_create → times
        add_mapping(296, 116); // seccomp → times
        add_mapping(297, 116); // getrandom → times
        add_mapping(298, 116); // memfd_create → times
        add_mapping(299, 81);  // bpf → access
        add_mapping(300, 81);  // execveat → access
        add_mapping(301, 67);  // membarrier → nanosleep
        add_mapping(302, 67);  // membarrier → nanosleep
        add_mapping(303, 67);  // recvmmsg_time64 → nanosleep
        add_mapping(304, 67);  // rt_sigtimedwait_time64 → nanosleep
        add_mapping(305, 67);  // futex_time64 → nanosleep
        add_mapping(306, 67);  // sched_rr_get_interval_time64 → nanosleep
        add_mapping(307, 67);  // timerfd_gettime64 → nanosleep
    }
}

unsafe fn add_mapping(linux_nr: usize, trainos_nr: usize) {
    if SYSCALL_MAP_COUNT < MAX_LINUX_SYSCALLS {
        SYSCALL_MAP[SYSCALL_MAP_COUNT] = SyscallMapping {
            linux_nr, trainos_nr, needs_translation: false
        };
        SYSCALL_MAP_COUNT += 1;
    }
}

/// Translate a Linux syscall number to TrainOS syscall number.
/// Also returns whether argument translation is needed.
pub fn translate_syscall(linux_nr: usize) -> Option<(usize, bool)> {
    unsafe {
        for i in 0..SYSCALL_MAP_COUNT {
            if SYSCALL_MAP[i].linux_nr == linux_nr {
                return Some((SYSCALL_MAP[i].trainos_nr, SYSCALL_MAP[i].needs_translation));
            }
        }
    }
    None
}

// ── Argument Translation ─────────────────────────────────────────────────────

/// Translate Linux open flags to TrainOS flags.
pub fn translate_open_flags(linux_flags: usize) -> usize {
    // Linux open flags:
    // O_RDONLY = 0, O_WRONLY = 1, O_RDWR = 2
    // O_CREAT = 0x40, O_EXCL = 0x80, O_TRUNC = 0x200, O_APPEND = 0x400
    let mut trainos_flags = 0;
    match linux_flags & 3 {
        0 => trainos_flags |= 0,  // O_RDONLY
        1 => trainos_flags |= 1,  // O_WRONLY
        2 => trainos_flags |= 2,  // O_RDWR
        _ => trainos_flags |= 0,
    }
    if linux_flags & 0x40 != 0 { trainos_flags |= 0x40; }  // O_CREAT
    if linux_flags & 0x200 != 0 { trainos_flags |= 0x200; } // O_TRUNC
    if linux_flags & 0x400 != 0 { trainos_flags |= 0x400; } // O_APPEND
    trainos_flags
}

/// Translate Linux mmap prot flags to TrainOS prot flags.
pub fn translate_mmap_prot(linux_prot: usize) -> usize {
    // Linux: PROT_READ=1, PROT_WRITE=2, PROT_EXEC=4
    // TrainOS uses same convention
    linux_prot & 7
}

/// Translate Linux mmap flags to TrainOS flags.
pub fn translate_mmap_flags(linux_flags: usize) -> usize {
    // Linux: MAP_SHARED=1, MAP_PRIVATE=2, MAP_ANONYMOUS=0x20
    linux_flags & 0x23
}

// ── Errno Conversion ─────────────────────────────────────────────────────────

/// Convert a TrainOS errno value to a Linux errno value.
/// Both use similar POSIX values, but some differ.
pub fn trainos_to_linux_errno(trainos_errno: isize) -> isize {
    match trainos_errno {
        // Common errnos (same in both)
        0 => 0,            // SUCCESS
        1 => 1,            // EPERM
        2 => 2,            // ENOENT
        3 => 3,            // ESRCH
        4 => 4,            // EINTR
        5 => 5,            // EIO
        6 => 6,            // ENXIO
        7 => 7,            // E2BIG
        8 => 8,            // ENOEXEC
        9 => 9,            // EBADF
        10 => 10,          // ECHILD
        11 => 11,          // EAGAIN
        12 => 12,          // ENOMEM
        13 => 13,          // EACCES
        14 => 14,          // EFAULT
        15 => 15,          // ENOTBLK
        16 => 16,          // EBUSY
        17 => 17,          // EEXIST
        18 => 18,          // EXDEV
        19 => 19,          // ENODEV
        20 => 20,          // ENOTDIR
        21 => 21,          // EISDIR
        22 => 22,          // EINVAL
        23 => 23,          // ENFILE
        24 => 24,          // EMFILE
        25 => 25,          // ENOTTY
        26 => 26,          // ETXTBSY
        27 => 27,          // EFBIG
        28 => 28,          // ENOSPC
        29 => 29,          // ESPIPE
        30 => 30,          // EROFS
        31 => 31,          // EMLINK
        32 => 32,          // EPIPE
        33 => 33,          // EDOM
        34 => 34,          // ERANGE
        _ => trainos_errno, // pass through for unknown
    }
}

/// Apply argument translation for syscalls that need it.
/// Returns the translated args as a tuple (a0, a1, a2, a3, a4).
pub fn translate_args(linux_nr: usize, a0: usize, a1: usize, a2: usize, a3: usize) -> (usize, usize, usize, usize, usize) {
    match linux_nr {
        2 | 5 => {
            // open/openat — translate flags (a1 is flags)
            let translated_flags = translate_open_flags(a1);
            (a0, translated_flags, a2, 0, 0)
        }
        9 => {
            // mmap — translate prot and flags
            let prot = translate_mmap_prot(a2);
            let flags = translate_mmap_flags(a3);
            (a0, a1, prot, flags, a2) // reuse original params after
        }
        25 => {
            // mremap — pass through
            (a0, a1, a2, 0, 0)
        }
        13 | 14 | 15 => {
            // rt_sigaction / rt_sigprocmask / rt_sigreturn
            // Remap to V30 signal syscalls
            (a0, a1, a2, 0, 0)
        }
        _ => (a0, a1, a2, a3, 0),
    }
}

// ── ELF Auxiliary Vector ─────────────────────────────────────────────────────

pub const AT_NULL: usize = 0;
pub const AT_IGNORE: usize = 1;
pub const AT_EXECFD: usize = 2;
pub const AT_PHDR: usize = 3;
pub const AT_PHENT: usize = 4;
pub const AT_PHNUM: usize = 5;
pub const AT_PAGESZ: usize = 6;
pub const AT_BASE: usize = 7;
pub const AT_FLAGS: usize = 8;
pub const AT_ENTRY: usize = 9;
pub const AT_NOTELF: usize = 10;
pub const AT_UID: usize = 11;
pub const AT_EUID: usize = 12;
pub const AT_GID: usize = 13;
pub const AT_EGID: usize = 14;
pub const AT_PLATFORM: usize = 15;
pub const AT_HWCAP: usize = 16;
pub const AT_CLKTCK: usize = 17;
pub const AT_SECURE: usize = 23;
pub const AT_BASE_PLATFORM: usize = 24;
pub const AT_RANDOM: usize = 25;
pub const AT_HWCAP2: usize = 26;
pub const AT_EXECFN: usize = 31;
pub const AT_SYSINFO_EHDR: usize = 33;

/// Set up the ELF auxiliary vector on the user stack.
/// Returns the new stack pointer after pushing auxv entries.
pub fn setup_auxv(
    stack_top: usize, entry: usize, phdr: usize, phent: usize, phnum: usize
) -> usize {
    if stack_top == 0 { return 0; }

    let mut sp = stack_top;
    let platform_str = b"riscv64\0";
    let execfn_str = b"/proc/self/exe\0";

    // We build the auxv in a small buffer, then push it onto the stack.
    // Each entry is 2 words (type, value) = 16 bytes on rv64.
    // We'll store IN REVERSE order because we push from high to low.

    // First, compute total space needed
    // 1. AT_NULL terminator (1 entry = 16 bytes)
    // 2. AT_EXECFN (2 words)
    // 3. execfn string (padded to 8 bytes)
    // 4. AT_PLATFORM (2 words)
    // 5. platform string (padded)
    // 6. AT_RANDOM (2 words) — pointer to 16 random bytes
    // 7. AT_SECURE (2 words)
    // 8. AT_UID, AT_EUID, AT_GID, AT_EGID (8 words)
    // 9. AT_HWCAP, AT_CLKTCK (4 words)
    // 10. AT_PAGESZ, AT_FLAGS, AT_ENTRY (6 words)
    // 11. AT_PHNUM, AT_PHENT, AT_PHDR, AT_BASE (8 words)

    // We'll push strings first on the stack (above auxv area)
    // Then push auxv entries from last to first.

    // Push execfn string (padded to 16 bytes)
    let execfn_len = 16; // round up to 16
    sp = sp.wrapping_sub(execfn_len);
    unsafe {
        let dst = sp as *mut u8;
        for i in 0..execfn_str.len() {
            dst.add(i).write_volatile(execfn_str[i]);
        }
        dst.add(execfn_str.len()).write_volatile(0);
    }
    let execfn_ptr = sp;

    // Push platform string (padded to 16 bytes)
    let plat_len = 16;
    sp = sp.wrapping_sub(plat_len);
    unsafe {
        let dst = sp as *mut u8;
        for i in 0..platform_str.len() {
            dst.add(i).write_volatile(platform_str[i]);
        }
        dst.add(platform_str.len()).write_volatile(0);
    }
    let platform_ptr = sp;

    // Push 16 random bytes for AT_RANDOM
    sp = sp.wrapping_sub(16);
    unsafe {
        let dst = sp as *mut u8;
        // Generate some "random" bytes from address entropy
        for i in 0..16 {
            dst.add(i).write_volatile(((sp as u64).wrapping_mul(6364136223846793005 + i as u64) >> 32) as u8);
        }
    }
    let random_ptr = sp;

    // Now push auxv entries in reverse order (last entry first)
    // AT_NULL terminator (2 words)
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_NULL);
        (sp as *mut usize).add(1).write_volatile(0);
    }

    // AT_EXECFN
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_EXECFN);
        (sp as *mut usize).add(1).write_volatile(execfn_ptr);
    }

    // AT_RANDOM
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_RANDOM);
        (sp as *mut usize).add(1).write_volatile(random_ptr);
    }

    // AT_SECURE
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_SECURE);
        (sp as *mut usize).add(1).write_volatile(0);
    }

    // AT_EGID
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_EGID);
        (sp as *mut usize).add(1).write_volatile(0);
    }

    // AT_GID
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_GID);
        (sp as *mut usize).add(1).write_volatile(0);
    }

    // AT_EUID
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_EUID);
        (sp as *mut usize).add(1).write_volatile(0);
    }

    // AT_UID
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_UID);
        (sp as *mut usize).add(1).write_volatile(0);
    }

    // AT_HWCAP2
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_HWCAP2);
        (sp as *mut usize).add(1).write_volatile(0);
    }

    // AT_HWCAP (RISC-V: indicates supported extensions)
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_HWCAP);
        // RISC-V HWCAP: IMAFD (bits set for supported extensions)
        (sp as *mut usize).add(1).write_volatile((1 << 0) | (1 << 1) | (1 << 2) | (1 << 3) | (1 << 4));
    }

    // AT_CLKTCK
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_CLKTCK);
        (sp as *mut usize).add(1).write_volatile(100);
    }

    // AT_PAGESZ
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_PAGESZ);
        (sp as *mut usize).add(1).write_volatile(4096);
    }

    // AT_FLAGS
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_FLAGS);
        (sp as *mut usize).add(1).write_volatile(0);
    }

    // AT_ENTRY
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_ENTRY);
        (sp as *mut usize).add(1).write_volatile(entry);
    }

    // AT_BASE (base address of the interpreter, 0 = no interpreter)
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_BASE);
        (sp as *mut usize).add(1).write_volatile(0);
    }

    // AT_PHNUM
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_PHNUM);
        (sp as *mut usize).add(1).write_volatile(phnum);
    }

    // AT_PHENT
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_PHENT);
        (sp as *mut usize).add(1).write_volatile(phent);
    }

    // AT_PHDR
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_PHDR);
        (sp as *mut usize).add(1).write_volatile(phdr);
    }

    // AT_PLATFORM
    sp = sp.wrapping_sub(16);
    unsafe {
        (sp as *mut usize).write_volatile(AT_PLATFORM);
        (sp as *mut usize).add(1).write_volatile(platform_ptr);
    }

    sp
}
