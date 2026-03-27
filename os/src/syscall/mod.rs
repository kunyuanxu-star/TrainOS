//! System call module
//!
//! Implements system call handling and dispatching

/// System call numbers
#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum SyscallId {
    Read = 0,
    Write = 1,
    Open = 2,
    Close = 3,
    Fork = 4,
    Exec = 5,
    Wait = 6,
    Exit = 7,
    Getpid = 8,
    Getppid = 9,
    SchedYield = 10,
}

/// Read register a0
fn get_arg0() -> usize {
    let mut val: usize;
    unsafe {
        core::arch::asm!("mv {}, a0", out(reg) val);
    }
    val
}

/// Read register a1
fn get_arg1() -> usize {
    let mut val: usize;
    unsafe {
        core::arch::asm!("mv {}, a1", out(reg) val);
    }
    val
}

/// Read register a2
fn get_arg2() -> usize {
    let mut val: usize;
    unsafe {
        core::arch::asm!("mv {}, a2", out(reg) val);
    }
    val
}

/// Read register a3
fn get_arg3() -> usize {
    let mut val: usize;
    unsafe {
        core::arch::asm!("mv {}, a3", out(reg) val);
    }
    val
}

/// Set return value in a0
fn set_ret(val: usize) {
    unsafe {
        core::arch::asm!("mv a0, {}", in(reg) val);
    }
}

/// Handle a system call
pub fn handle_syscall() {
    let syscall_id: usize;
    unsafe {
        core::arch::asm!("mv {}, a7", out(reg) syscall_id);
    }

    let result = match syscall_id {
        0 => sys_read(get_arg0(), get_arg1(), get_arg2()),   // read
        1 => sys_write(get_arg0(), get_arg1(), get_arg2()), // write
        4 => sys_fork(),                                    // fork
        7 => sys_exit(get_arg0()),                          // exit
        8 => sys_getpid(),                                  // getpid
        10 => sys_sched_yield(),                            // sched_yield
        _ => {
            -1
        }
    };

    set_ret(result as usize);

    // Advance program counter
    advance_sepc();
}

/// Advance sepc by 4 bytes (skip the ecall instruction)
fn advance_sepc() {
    #[allow(deprecated)]
    let mut sepc = riscv::register::sepc::read();
    sepc += 4;
    #[allow(deprecated)]
    riscv::register::sepc::write(sepc);
}

/// System call: read
fn sys_read(_fd: usize, _buf: usize, _count: usize) -> isize {
    crate::println!("[syscall] read called");
    0
}

/// System call: write
pub fn sys_write(fd: usize, buf: usize, count: usize) -> isize {
    if fd != 1 {
        return -1;
    }
    // Write string to console
    let mut written = 0;
    let mut ptr = buf;
    while written < count {
        let c = unsafe { *(ptr as *const u8) };
        crate::console::sbi_console_putchar(c as usize);
        if c == b'\n' {
            crate::console::sbi_console_putchar(b'\r' as usize);
        }
        ptr += 1;
        written += 1;
    }
    written as isize
}

/// System call: fork
pub fn sys_fork() -> isize {
    0  // Child returns 0
}

/// System call: exit
pub fn sys_exit(_code: usize) -> ! {
    crate::println!("[syscall] exit called");
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/// System call: getpid
pub fn sys_getpid() -> isize {
    1  // Return PID 1 for now
}

/// System call: sched_yield
pub fn sys_sched_yield() -> isize {
    0
}
