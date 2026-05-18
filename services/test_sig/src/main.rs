#![no_std]
#![no_main]
use core::panic::PanicInfo;
use tros;

const SIGCHLD: u32 = 17;
const SIG_IGN: usize = 1;

/// Print a label followed by a u32 value
fn report(label: &str, val: u32) {
    tros::print("SIG: ");
    tros::print(label);
    tros::print_uint(val as usize);
    tros::print("\r\n");
}

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("SIG: signal handling test\r\n");

    // Register SIGCHLD handler (ignore it)
    let r = tros::signal(SIGCHLD, SIG_IGN);
    report("signal(SIGCHLD, SIG_IGN)=", r as u32);

    // Fork a child and wait for it
    let child = tros::fork();
    if child == 0 {
        tros::print("SIG: child exiting\r\n");
        tros::exit(0);
    } else if child != usize::MAX {
        report("parent, child=", child as u32);

        // Wait for child: spin until waitpid returns the child pid.
        // The timer interrupt will preempt this thread and schedule
        // the child, which exits quickly.
        let mut status: i32 = 0;
        let waited = loop {
            let w = tros::waitpid(child as i32, &mut status, 0);
            if w == child || w != 0 {
                break w;
            }
        };
        report("waitpid returned pid=", waited as u32);

        if waited == child {
            tros::print("SIG: PASS\r\n");
        } else {
            tros::print("SIG: FAIL\r\n");
        }
    }

    tros::exit(0);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! { loop { unsafe { core::arch::asm!("wfi"); } } }
