# syscall.S - System call wrappers for TrainOS
# RISC-V 64-bit

.section .text

# Syscall numbers
.equ SYS_EXIT, 93
.equ SYS_READ, 63
.equ SYS_WRITE, 64
.equ SYS_OPENAT, 56
.equ SYS_CLOSE, 57
.equ SYS_BRK, 214
.equ SYS_GETPID, 172
.equ SYS_GETTID, 178
.equ SYS_GETUID, 174
.equ SYS_GETGID, 176
.equ SYS_GETEUID, 175
.equ SYS_GETEGID, 177
.equ SYS_SCHED_YIELD, 124
.equ SYS_clone, 220
.equ SYS_execve, 221
.equ SYS_wait4, 260
.equ SYS_YIELD, 124

# File descriptors
.equ STDIN, 0
.equ STDOUT, 1
.equ STDERR, 2

# Macros
.macro syscall0 num
    li a7, \num
    ecall
.endm

.macro syscall1 num a0
    li a7, \num
    mv a0, \a0
    ecall
.endm

.macro syscall2 num a0 a1
    li a7, \num
    mv a0, \a0
    mv a1, \a1
    ecall
.endm

.macro syscall3 num a0 a1 a2
    li a7, \num
    mv a0, \a0
    mv a1, \a1
    mv a2, \a2
    ecall
.endm

.macro syscall4 num a0 a1 a2 a3
    li a7, \num
    mv a0, \a0
    mv a1, \a1
    mv a2, \a2
    mv a3, \a3
    ecall
.endm

# _exit - Exit the program
.global _exit
.type _exit, @function
_exit:
    syscall1 SYS_EXIT, a0
    # Should not return
    tail .

# _read - Read from file descriptor
.global _read
.type _read, @function
_read:
    syscall3 SYS_READ, a0, a1, a2
    ret

# _write - Write to file descriptor
.global _write
.type _write, @function
_write:
    syscall3 SYS_WRITE, a0, a1, a2
    ret

# _openat - Open file
.global _openat
.type _openat, @function
_openat:
    # a0 = dirfd, a1 = pathname, a2 = flags, a3 = mode
    syscall4 SYS_OPENAT, a0, a1, a2, a3
    ret

# _close - Close file descriptor
.global _close
.type _close, @function
_close:
    syscall1 SYS_CLOSE, a0
    ret

# _brk - Change data segment size
.global _brk
.type _brk, @function
_brk:
    syscall1 SYS_BRK, a0
    ret

# _sbrk - BSD compatibility wrapper for brk
.global _sbrk
.type _sbrk, @function
_sbrk:
    # Save current brk
    mv t0, a0

    # Get current program break
    syscall0 SYS_BRK
    mv t1, a0

    # If increment is 0, just return current break
    bnez a0, 1f
    mv a0, t1
    ret

1:
    # Add increment to current break
    add a0, t1, t0
    # Set new break
    syscall1 SYS_BRK, a0
    # Return old break on success, -1 on failure
    mv a0, t1
    ret

# _getpid - Get process ID
.global _getpid
.type _getpid, @function
_getpid:
    syscall0 SYS_GETPID
    ret

# _gettid - Get thread ID
.global _gettid
.type _gettid, @function
_gettid:
    syscall0 SYS_GETTID
    ret

# _getuid - Get user ID
.global _getuid
.type _getuid, @function
_getuid:
    syscall0 SYS_GETUID
    ret

# _getgid - Get group ID
.global _getgid
.type _getgid, @function
_getgid:
    syscall0 SYS_GETGID
    ret

# _geteuid - Get effective user ID
.global _geteuid
.type _geteuid, @function
_geteuid:
    syscall0 SYS_GETEUID
    ret

# _getegid - Get effective group ID
.global _getegid
.type _getegid, @function
_getegid:
    syscall0 SYS_GETEGID
    ret

# _sched_yield - Yield the processor
.global _sched_yield
.type _sched_yield, @function
_sched_yield:
    syscall0 SYS_SCHED_YIELD
    ret

# _yield - Alias for sched_yield
.global _yield
.type _yield, @function
_yield:
    syscall0 SYS_YIELD
    ret

# _wait4 - Wait for process termination
.global _wait4
.type _wait4, @function
_wait4:
    # a0 = pid, a1 = status, a2 = options, a3 = rusage
    mv t0, a0
    mv a0, a1  # reorder for syscall
    mv a1, t0
    syscall3 SYS_wait4, a0, a1, a2
    ret

# _clone - Create a new process/thread
.global _clone
.type _clone, @function
_clone:
    syscall1 SYS_clone, a0
    ret

# _execve - Execute program
.global _execve
.type _execve, @function
_execve:
    syscall3 SYS_execve, a0, a1, a2
    ret

# _putchar - Put a character (for stdio)
.global _putchar
.type _putchar, @function
_putchar:
    # a0 = character to print
    # Write to stdout
    addi sp, sp, -16
    sd a0, 0(sp)
    mv a1, sp
    li a0, STDOUT
    li a2, 1
    syscall3 SYS_WRITE, a0, a1, a2
    ld a0, 0(sp)
    addi sp, sp, 16
    ret

# _getchar - Get a character (for stdio)
.global _getchar
.type _getchar, @function
_getchar:
    addi sp, sp, -16
    mv a1, sp
    li a0, STDIN
    li a2, 1
    syscall3 SYS_READ, a0, a1, a2
    ld a0, 0(sp)
    addi sp, sp, 16
    # Return EOF (-1) if no input
    beqz a0, 1f
    lb a0, 0(sp)
    ret
1:
    li a0, -1
    ret

# _isatty - Check if fd is a terminal
.global _isatty
.type _isatty, @function
_isatty:
    # Assume all fds are terminals for now
    li a0, 1
    ret

# _times - Get process times
.global _times
.type _times, @function
_times:
    # Return 0 for all times (simplified)
    xor a0, a0, a0
    ret

# _gettimeofday - Get time of day
.global _gettimeofday
.type _gettimeofday, @function
_gettimeofday:
    # Return 0 for now (simplified)
    xor a0, a0, a0
    ret

# _sigsuspend - Suspend until signal
.global _sigsuspend
.type _sigsuspend, @function
_sigsuspend:
    # Simplified - just return 0
    xor a0, a0, a0
    ret

# _kill - Send signal to process
.global _kill
.type _kill, @function
_kill:
    li a0, -1  # Return error
    ret

# _link - Create hard link
.global _link
.type _link, @function
_link:
    li a0, -1  # Not supported
    ret

# _unlink - Remove file
.global _unlink
.type _unlink, @function
_unlink:
    li a0, -1  # Not supported
    ret

# _mkdir - Create directory
.global _mkdir
.type _mkdir, @function
_mkdir:
    li a0, -1  # Not supported
    ret

# _chdir - Change directory
.global _chdir
.type _chdir, @function
_chdir:
    # Always succeed
    xor a0, a0, a0
    ret

# _getcwd - Get current working directory
.global _getcwd
.type _getcwd, @function
_getcwd:
    # Return "/"
    li t0, 47  # '/'
    sb t0, 0(a0)
    sb zero, 1(a0)
    li a0, 1
    ret

# _fstat - Get file status
.global _fstat
.type _fstat, @function
_fstat:
    # Return dummy stat
    xor a0, a0, a0
    ret

# _stat - Get file status by path
.global _stat
.type _stat, @function
_stat:
    # Return dummy stat
    xor a0, a0, a0
    ret

# _lseek - Seek in file
.global _lseek
.type _lseek, @function
_lseek:
    xor a0, a0, a0
    ret

# _access - Check file access permissions
.global _access
.type _access, @function
_access:
    li a0, -1
    ret

# _sysconf - Get configuration value
.global _sysconf
.type _sysconf, @function
_sysconf:
    # Return default values
    li a0, -1
    ret

# _getpagesize - Get system page size
.global _getpagesize
.type _getpagesize, @function
_getpagesize:
    li a0, 4096
    ret

# _exit_group - Exit all threads
.global _exit_group
.type _exit_group, @function
_exit_group:
    syscall1 SYS_EXIT, a0
    tail .

# _newlib initialization hooks
.global _init
.type _init, @function
_init:
    ret

.global _fini
.type _fini, @function
_fini:
    ret

# Stack canary (for stack protection)
.section .note.GNU-stack,"",@progbits
