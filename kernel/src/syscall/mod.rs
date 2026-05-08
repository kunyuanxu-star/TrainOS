pub mod proc;
pub mod ipc;
pub mod cap;
pub mod posix;

use crate::trap::TrapFrame;

// Syscall numbers
pub const SYS_EXIT:      usize = 0;
pub const SYS_SPAWN:     usize = 3;
pub const SYS_FORK:      usize = 4;
pub const SYS_GETPID:    usize = 5;
pub const SYS_EP_CREATE: usize = 10;
pub const SYS_SEND:      usize = 11;
pub const SYS_RECV:      usize = 12;
pub const SYS_CALL:      usize = 13;
pub const SYS_REPLY:     usize = 14;
pub const SYS_MMIO_MAP:    usize = 20;
pub const SYS_UNMAP:       usize = 21;
pub const SYS_MAP_MMIO:    usize = 22;
pub const SYS_MMIO_READ32: usize = 23;
pub const SYS_MMIO_WRITE32: usize = 24;
pub const SYS_MINT:      usize = 30;
pub const SYS_COPY:      usize = 31;
pub const SYS_MOVE:      usize = 32;
pub const SYS_DELETE:    usize = 33;
pub const SYS_CAP_STATS: usize = 34;
pub const SYS_BLK_READ:  usize = 40;
pub const SYS_BLK_WRITE: usize = 45;
pub const SYS_PROCLIST:  usize = 41;
pub const SYS_KILL:      usize = 42;
// POSIX compatibility syscalls
pub const SYS_OPEN:      usize = 50;
pub const SYS_READ:      usize = 51;
pub const SYS_WRITE:     usize = 52;
pub const SYS_CLOSE:     usize = 53;
// SBI forwarding (note: SYS_SPAWN and SYS_PUTCHAR both use nr=1, differentiated by context)
pub const SYS_PUTCHAR:   usize = 1;
pub const SYS_GETCHAR:   usize = 2;

pub fn syscall_dispatch(tf: &mut TrapFrame) {
    let nr = tf.a7;
    let arg0 = tf.a0;
    let arg1 = tf.a1;
    let arg2 = tf.a2;
    let arg3 = tf.a3;

    let result = match nr {
        SYS_PUTCHAR => {
            // Forward SBI console putchar to M-mode
            unsafe {
                core::arch::asm!("ecall", in("a7") 1usize, in("a0") tf.a0);
            }
            Ok(0)
        }
        SYS_GETCHAR => {
            // Forward SBI console getchar to M-mode
            let c: usize;
            unsafe {
                core::arch::asm!(
                    "ecall",
                    in("a7") 2usize,
                    lateout("a0") c,
                );
            }
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
        SYS_FORK => proc::sys_fork(tf.sepc),
        SYS_GETPID => Ok(crate::sched::current_thread()
            .map(|t| unsafe { (*t).owner as usize }).unwrap_or(0)),
        SYS_OPEN  => posix::sys_open(arg0, arg1, arg2),
        SYS_READ  => posix::sys_read(arg0, arg1, arg2),
        SYS_WRITE => posix::sys_write(arg0, arg1, arg2),
        SYS_CLOSE => posix::sys_close(arg0),
        SYS_MMIO_READ32  => sys_mmio_read32(arg0),
        SYS_MMIO_WRITE32 => sys_mmio_write32(arg0, arg1),
        SYS_BLK_READ => proc::sys_blk_read(arg0, arg1, arg2),
        SYS_BLK_WRITE => proc::sys_blk_write(arg0, arg1, arg2),
        SYS_PROCLIST => proc::sys_proclist(arg0, arg1),
        SYS_KILL     => proc::sys_kill(arg0 as u32),
        _ => Err("unknown syscall"),
    };

    match result {
        Ok(val) => { tf.a0 = val; }
        Err(_e) => { tf.a0 = usize::MAX; } // error
    }

    tf.sepc += 4;
}

fn sys_map_mmio(phys: usize, size: usize) -> Result<usize, &'static str> {
    let thread = crate::sched::current_thread().ok_or("no thread")?;
    let pid = unsafe { (*thread).owner };
    crate::console::puts("  MMIO: pid=");
    let mut n = pid as usize;
    let mut buf = [0u8; 10];
    let mut i = 10;
    loop { i -= 1; buf[i] = b'0' + (n % 10) as u8; n /= 10; if n == 0 { break; } }
    for j in i..10 { unsafe { core::arch::asm!("ecall", in("a7") 1usize, in("a0") buf[j] as usize); } }
    crate::console::puts("\r\n");

    let procs = crate::proc::PROCESSES.lock();
    let mut root_pt = 0;
    for proc in procs.iter() {
        if proc.pid == pid {
            root_pt = proc.page_table_root;
            break;
        }
    }
    if root_pt == 0 {
        crate::console::puts("  MMIO: process not found!\r\n");
        return Err("process not found");
    }
    drop(procs);

    crate::console::puts("  MMIO: root_pt=0x");
    for i in (0..8).rev() {
        let nibble = (root_pt >> (i * 4)) & 0xF;
        let c = if nibble < 10 { b'0' + nibble as u8 } else { b'a' + (nibble - 10) as u8 };
        unsafe { core::arch::asm!("ecall", in("a7") 1usize, in("a0") c as usize); }
    }
    crate::console::puts("\r\n");

    let va = crate::proc::elf::map_phys_to_user(root_pt, phys, size);
    crate::console::puts("  MMIO: mapped at va=0x");
    for i in (0..8).rev() {
        let nibble = (va >> (i * 4)) & 0xF;
        let c = if nibble < 10 { b'0' + nibble as u8 } else { b'a' + (nibble - 10) as u8 };
        unsafe { core::arch::asm!("ecall", in("a7") 1usize, in("a0") c as usize); }
    }
    crate::console::puts("\r\n");
    Ok(va)
}

/// Read a 32-bit value from a physical MMIO address.
/// The kernel (S-mode) accesses the address directly via the identity mapping
/// set up in setup_kernel_mapping().
///
/// Uses a temporary fault handler to catch load access faults on non-existent
/// physical addresses, returning an error instead of crashing.
fn sys_mmio_read32(phys: usize) -> Result<usize, &'static str> {
    if phys & 0x3 != 0 {
        return Err("unaligned mmio read");
    }

    unsafe {
        // Reset fault flag (defined in global_asm below)
        extern "C" {
            static __mmio_fault_happened: core::cell::UnsafeCell<usize>;
            fn __mmio_fault_recover();
        }
        core::ptr::write_volatile((&__mmio_fault_happened).get(), 0);

        // Install temporary fault handler
        let old_stvec: usize;
        core::arch::asm!("csrr {}, stvec", out(reg) old_stvec);
        core::arch::asm!("csrw stvec, {}", in(reg) __mmio_fault_recover as *const () as usize);

        // Do the volatile read
        let val = (phys as *const u32).read_volatile();

        // Restore original handler
        core::arch::asm!("csrw stvec, {}", in(reg) old_stvec);

        if core::ptr::read_volatile((&__mmio_fault_happened).get()) != 0 {
            Err("mmio load access fault")
        } else {
            Ok(val as usize)
        }
    }
}

/// Temporary fault handler for sys_mmio_read32 / sys_mmio_write32.
/// On a load/store access fault:
///   1. Sets __mmio_fault_happened = 1
///   2. Skips past the faulting instruction (assumes 4-byte lw/sw)
///   3. Returns via sret
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

/// Write a 32-bit value to a physical MMIO address.
fn sys_mmio_write32(phys: usize, val: usize) -> Result<usize, &'static str> {
    if phys & 0x3 != 0 {
        return Err("unaligned mmio write");
    }
    unsafe {
        (phys as *mut u32).write_volatile(val as u32);
    }
    Ok(0)
}
