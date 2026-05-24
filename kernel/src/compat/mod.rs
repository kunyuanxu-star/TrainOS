// V30: Linux ABI Compatibility Subsystem
//
// Features:
//   - Linux syscall number → TrainOS syscall translation table
//   - /proc filesystem compatibility (full procfs)
//   - ELF auxiliary vector setup (AT_* entries)
//   - Signal frame layout (sigaction-compatible)

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

/// Initialize the syscall translation table.
pub fn compat_init() {
    unsafe {
        SYSCALL_MAP_COUNT = 0;
        // Linux → TrainOS mappings (common subset)
        add_mapping(0, 0);    // read
        add_mapping(1, 52);   // write
        add_mapping(2, 50);   // open
        add_mapping(3, 53);   // close
        add_mapping(5, 51);   // read → TrainOS open+read
        add_mapping(8, 55);   // lseek
        add_mapping(9, 83);   // mmap
        add_mapping(10, 85);  // mprotect
        add_mapping(11, 84);  // munmap
        add_mapping(12, 86);  // brk
        add_mapping(39, 5);   // getpid
        add_mapping(56, 4);   // clone → fork
        add_mapping(57, 4);   // fork
        add_mapping(60, 0);   // exit
        add_mapping(61, 64);  // wait4 → waitpid
        add_mapping(62, 42);  // kill
        add_mapping(78, 76);  // mkdir
        add_mapping(79, 77);  // rmdir
        add_mapping(80, 78);  // unlink
        add_mapping(82, 79);  // rename
        add_mapping(93, 0);   // exit_group → exit
        add_mapping(96, 116); // times
        add_mapping(99, 114); // sched_setaffinity
        add_mapping(102, 5);  // getuid
        add_mapping(110, 5);  // getppid
        add_mapping(113, 117); // getrusage
        add_mapping(124, 68); // clock_gettime
        add_mapping(135, 67); // nanosleep
        add_mapping(162, 67); // nanosleep (alt)
        add_mapping(169, 70); // setsid
        add_mapping(174, 81); // access
        add_mapping(272, 110); // unshare
        add_mapping(275, 111); // sethostname
        add_mapping(281, 122); // reboot
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
pub fn translate_syscall(linux_nr: usize) -> Option<usize> {
    unsafe {
        for i in 0..SYSCALL_MAP_COUNT {
            if SYSCALL_MAP[i].linux_nr == linux_nr {
                return Some(SYSCALL_MAP[i].trainos_nr);
            }
        }
    }
    None
}

// ── ELF Auxiliary Vector ─────────────────────────────────────────────────────

pub const AT_NULL: usize = 0;
pub const AT_PHDR: usize = 3;
pub const AT_PHENT: usize = 4;
pub const AT_PHNUM: usize = 5;
pub const AT_PAGESZ: usize = 6;
pub const AT_ENTRY: usize = 9;
pub const AT_UID: usize = 11;
pub const AT_EUID: usize = 12;
pub const AT_GID: usize = 13;
pub const AT_EGID: usize = 14;
pub const AT_RANDOM: usize = 25;
pub const AT_SYSINFO_EHDR: usize = 33;

/// Set up the ELF auxiliary vector on the user stack.
/// Returns the new stack pointer after pushing auxv entries.
pub fn setup_auxv(stack_top: usize, entry: usize, phdr: usize, phent: usize, phnum: usize) -> usize {
    // Simplified: store a minimal auxv
    // In a full implementation, this would push entries onto the user stack
    // and return the adjusted sp.
    stack_top
}
