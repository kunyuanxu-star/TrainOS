//! Syscall task management
//!
//! Implements process/thread-related syscalls

// Re-export the process TaskControlBlock for use in syscalls

/// Clone flags
pub const CLONE_VM: usize = 0x00000100;       // Share virtual memory
pub const CLONE_FS: usize = 0x00000200;       // Share filesystem info
pub const CLONE_FILES: usize = 0x00000400;    // Share file descriptors
pub const CLONE_SIGHAND: usize = 0x00008000;  // Share signal handlers
pub const CLONE_PTRACE: usize = 0x00002000;   // Trace this clone
pub const CLONE_VFORK: usize = 0x00004000;    // VFORK parent sleeps
pub const CLONE_PARENT: usize = 0x00008000;   // Parent is same thread group
pub const CLONE_THREAD: usize = 0x00010000;   // Add to same thread group
pub const CLONE_NEWNS: usize = 0x00020000;    // New mount namespace
pub const CLONE_SYSVSEM: usize = 0x00040000;  // Share SysV semundo
pub const CLONE_SETTLS: usize = 0x00080000;   // Set TLS
pub const CLONE_PARENT_SETTID: usize = 0x00100000; // Set parent TID
pub const CLONE_CHILD_CLEARTID: usize = 0x00200000; // Clear child TID
pub const CLONE_CHILD_SETTID: usize = 0x01000000;  // Set child TID
pub const CLONE_SIGNAL: usize = 0x02000000;        // Signal to deliver

// Fork flags (for compatibility)
pub const SIGCHLD: usize = 17;
