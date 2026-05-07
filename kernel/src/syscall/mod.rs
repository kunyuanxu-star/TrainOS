pub mod proc;
pub mod ipc;
pub mod cap;

use crate::trap::TrapFrame;

// Syscall numbers
pub const SYS_EXIT:      usize = 0;
pub const SYS_SPAWN:     usize = 3;
pub const SYS_GETPID:    usize = 5;
pub const SYS_EP_CREATE: usize = 10;
pub const SYS_SEND:      usize = 11;
pub const SYS_RECV:      usize = 12;
pub const SYS_CALL:      usize = 13;
pub const SYS_REPLY:     usize = 14;
pub const SYS_MAP:       usize = 20;
pub const SYS_UNMAP:     usize = 21;
pub const SYS_MINT:      usize = 30;
pub const SYS_COPY:      usize = 31;
pub const SYS_MOVE:      usize = 32;
pub const SYS_DELETE:    usize = 33;
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
        SYS_MINT => cap::sys_mint(arg0, arg1, arg2 as u8),
        SYS_COPY => cap::sys_copy(arg0, arg1, arg2, arg3),
        SYS_MOVE => cap::sys_move(arg0, arg1, arg2, arg3),
        SYS_DELETE => cap::sys_delete(arg0, arg1),
        SYS_EXIT => proc::sys_exit(arg0 as i32),
        SYS_SPAWN => proc::sys_spawn(arg0, arg1),
        SYS_GETPID => Ok(crate::sched::current_thread()
            .map(|t| unsafe { (*t).owner as usize }).unwrap_or(0)),
        _ => Err("unknown syscall"),
    };

    match result {
        Ok(val) => { tf.a0 = val; tf.a1 = 0; }
        Err(_e) => { tf.a0 = usize::MAX; } // error
    }

    tf.sepc += 4;
}
