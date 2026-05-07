pub mod asm;

/// TrapFrame: register state saved on trap entry.
/// Layout must match asm.rs offsets exactly.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TrapFrame {
    pub ra:  usize,   // 0*8
    pub gp:  usize,   // 1*8
    pub tp:  usize,   // 2*8
    pub t0:  usize,   // 3*8 (user_sp at save time)
    pub t1:  usize,   // 4*8
    pub t2:  usize,   // 5*8
    pub s0:  usize,   // 6*8
    pub s1:  usize,   // 7*8
    pub a0:  usize,   // 8*8
    pub a1:  usize,   // 9*8
    pub a2:  usize,   // 10*8
    pub a3:  usize,   // 11*8
    pub a4:  usize,   // 12*8
    pub a5:  usize,   // 13*8
    pub a6:  usize,   // 14*8
    pub a7:  usize,   // 15*8
    pub s2:  usize,   // 16*8
    pub s3:  usize,   // 17*8
    pub s4:  usize,   // 18*8
    pub s5:  usize,   // 19*8
    pub s6:  usize,   // 20*8
    pub s7:  usize,   // 21*8
    pub s8:  usize,   // 22*8
    pub s9:  usize,   // 23*8
    pub s10: usize,   // 24*8
    pub s11: usize,   // 25*8
    pub t3:  usize,   // 26*8
    pub t4:  usize,   // 27*8
    pub t5:  usize,   // 28*8
    pub t6:  usize,   // 29*8
    pub sepc:    usize, // 30*8
    pub sstatus: usize, // 31*8
    pub satp:    usize, // 32*8
    pub user_sp: usize, // 33*8
    pub stval:   usize, // 34*8
}

impl Default for TrapFrame {
    fn default() -> Self {
        TrapFrame {
            ra: 0, gp: 0, tp: 0, t0: 0, t1: 0, t2: 0,
            s0: 0, s1: 0, a0: 0, a1: 0, a2: 0, a3: 0,
            a4: 0, a5: 0, a6: 0, a7: 0, s2: 0, s3: 0,
            s4: 0, s5: 0, s6: 0, s7: 0, s8: 0, s9: 0,
            s10: 0, s11: 0, t3: 0, t4: 0, t5: 0, t6: 0,
            sepc: 0, sstatus: 0, satp: 0, user_sp: 0, stval: 0,
        }
    }
}

/// Initialize trap handling: set stvec to __trap_entry.
pub fn init() {
    extern "C" {
        fn __trap_entry();
    }
    unsafe {
        core::arch::asm!("csrw stvec, {}", in(reg) __trap_entry as *const () as usize);
    }
}

/// Enable timer interrupts (sie.STIE).
pub fn enable_timer_interrupt() {
    unsafe {
        core::arch::asm!("csrrs zero, sie, {}", in(reg) 0x20usize); // set STIE bit (bit 5)
    }
}

// CLINT constants
const CLINT_BASE: usize = 0x0200_0000;
const CLINT_MTIMECMP: usize = CLINT_BASE + 0x4000;
const CLINT_MTIME: usize = CLINT_BASE + 0xBFF8;
const TIMEBASE_FREQ: usize = 10_000_000; // 10MHz
const TICK_MS: usize = 10;
const TICK_TICKS: usize = (TICK_MS * TIMEBASE_FREQ) / 1000;

unsafe fn mtime() -> u64 {
    let ptr = CLINT_MTIME as *const u64;
    ptr.read_volatile()
}

unsafe fn set_mtimecmp(val: u64) {
    let ptr = CLINT_MTIMECMP as *mut u64;
    ptr.write_volatile(val);
}

pub fn clint_set_next_timer() {
    unsafe {
        let current = mtime();
        set_mtimecmp(current + TICK_TICKS as u64);
    }
}

pub fn clint_init() {
    clint_set_next_timer();
}

/// Trap dispatch — called from assembly with TrapFrame pointer in a0.
#[no_mangle]
extern "C" fn handle_trap(tf: &mut TrapFrame) {
    let scause: usize;
    unsafe { core::arch::asm!("csrr {}, scause", out(reg) scause); }

    let cause = scause & !(1usize << 63);
    let is_interrupt = (scause >> 63) != 0;

    if is_interrupt {
        match cause {
            5 => timer_interrupt(tf), // Supervisor Timer
            _ => unknown_trap(scause),
        }
    } else {
        match cause {
            8 => syscall(tf),         // Environment call from U-mode
            _ => unknown_trap(scause),
        }
    }
}

fn timer_interrupt(_tf: &mut TrapFrame) {
    clint_set_next_timer();
    crate::sched::schedule();
}

fn syscall(tf: &mut TrapFrame) {
    crate::syscall::syscall_dispatch(tf);
}

fn unknown_trap(scause: usize) {
    let stval: usize;
    unsafe { core::arch::asm!("csrr {}, stval", out(reg) stval); }

    crate::console::puts("Unhandled trap: scause=0x");
    for i in (0..16).rev() {
        let nibble = (scause >> (i * 4)) & 0xF;
        let c = if nibble < 10 { b'0' + nibble as u8 } else { b'a' + (nibble - 10) as u8 };
        unsafe { core::arch::asm!("ecall", in("a7") 1usize, in("a0") c as usize); }
    }
    crate::console::puts(" stval=0x");
    for i in (0..16).rev() {
        let nibble = (stval >> (i * 4)) & 0xF;
        let c = if nibble < 10 { b'0' + nibble as u8 } else { b'a' + (nibble - 10) as u8 };
        unsafe { core::arch::asm!("ecall", in("a7") 1usize, in("a0") c as usize); }
    }
    crate::console::puts("\r\n");
    crate::idle_loop();
}
