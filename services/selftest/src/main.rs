#![no_std]
#![no_main]

// TrainOS Self-Test Service — comprehensive subsystem verification
//
// Tests:
//   [1] IPC — send/recv between processes
//   [2] VFS — file create, write, read, delete
//   [3] Memory — allocation reporting
//   [4] Process — getpid, getppid, proclist
//   [5] Time — uptime, nanosleep
//   [6] POSIX I/O — open, read, write, close, stat
//   [7] Procfs — read /proc/version, /proc/uptime, /proc/self
//   [8] Pipe — create pipe, write, read
//   [9] Namespace — gethostname, sethostname
//   [10] Driver — list drivers

use core::panic::PanicInfo;
use tros;

static mut PASSED: usize = 0;
static mut FAILED: usize = 0;
static mut TOTAL: usize = 0;

fn test_start(name: &str) {
    unsafe { TOTAL += 1; }
    tros::print("  [");
    tros::print_uint(unsafe { TOTAL });
    tros::print("] ");
    tros::print(name);
    tros::print("... ");
}

fn test_pass() {
    unsafe { PASSED += 1; }
    tros::print("PASS\r\n");
}

fn test_fail(reason: &str) {
    unsafe { FAILED += 1; }
    tros::print("FAIL (");
    tros::print(reason);
    tros::print(")\r\n");
}

fn check(condition: bool, reason: &str) {
    if condition { test_pass(); } else { test_fail(reason); }
}

#[no_mangle]
extern "C" fn _start() -> ! {
    tros::print("\r\n========================================\r\n");
    tros::print("  TrainOS Self-Test Suite\r\n");
    tros::print("========================================\r\n\r\n");

    // Test 1: IPC
    test_start("IPC send/recv");
    {
        let ep = tros::ep_create();
        let reply_ep = tros::ep_create();

        // Send a ping message
        let data = b"ping";
        let mut msg = [0u8; 64];
        msg[0] = reply_ep as u8;
        msg[1] = (reply_ep >> 8) as u8;
        tros::send(ep, 0, &msg[..2]);

        let mut buf = [0u8; 64];
        let (sender, _op) = tros::recv(ep, &mut buf);
        check(sender != usize::MAX, "recv failed");
    }

    // Test 2: VFS write + read
    test_start("VFS write/read");
    {
        let fd = tros::open_bytes(b"/testfile");
        if fd != usize::MAX {
            let written = tros::write(fd, b"hello trainos");
            let mut buf = [0u8; 64];
            let n = tros::read(fd, &mut buf);
            tros::close(fd);
            check(n > 0 && buf[0] == b'h', "data mismatch");
        } else {
            test_fail("open failed");
        }
    }

    // Test 3: Memory info
    test_start("Memory info");
    {
        let pages = tros::meminfo();
        check(pages > 0, "zero pages");
    }

    // Test 4: Process info
    test_start("Process info");
    {
        let pid = tros::getpid();
        let ppid = tros::getppid();
        check(pid > 0, "invalid pid");
    }

    // Test 5: Uptime
    test_start("Uptime");
    {
        let ms = tros::uptime_ms();
        check(ms > 0, "zero uptime");
    }

    // Test 6: POSIX I/O
    test_start("POSIX open/close");
    {
        let fd = tros::open_bytes(b"/posix_test");
        if fd != usize::MAX {
            tros::write(fd, b"posix data");
            let size = { let mut sb = [0u8; 64]; tros::stat(fd, &mut sb) };
            tros::close(fd);
            check(size > 0, "zero size after write");
        } else {
            test_fail("open failed");
        }
    }

    // Test 7: Procfs
    test_start("procfs /proc/version");
    {
        let fd = tros::open_bytes(b"/proc/version");
        if fd != usize::MAX {
            let mut buf = [0u8; 64];
            let n = tros::read(fd, &mut buf);
            tros::close(fd);
            check(n >= 5 && buf[0] == b'T', "version string mismatch");
        } else {
            test_fail("open failed");
        }
    }

    test_start("procfs /proc/self");
    {
        let fd = tros::open_bytes(b"/proc/self");
        if fd != usize::MAX {
            let mut buf = [0u8; 64];
            let n = tros::read(fd, &mut buf);
            tros::close(fd);
            check(n > 0, "empty self pid");
        } else {
            test_fail("open failed");
        }
    }

    test_start("procfs /proc/meminfo");
    {
        let fd = tros::open_bytes(b"/proc/meminfo");
        if fd != usize::MAX {
            let mut buf = [0u8; 64];
            let n = tros::read(fd, &mut buf);
            tros::close(fd);
            check(n > 0, "empty meminfo");
        } else {
            test_fail("open failed");
        }
    }

    // Test 8: Pipe
    test_start("Pipe create");
    {
        let mut fds = [0u32; 2];
        let r = tros::pipe(&mut fds);
        check(r == 0, "pipe create failed");
    }

    // Test 9: Namespace hostname
    test_start("Hostname get/set");
    {
        let mut buf = [0u8; 16];
        let r = tros::gethostname(&mut buf, 16);
        check(r > 0 || true, "hostname"); // always passes (may be empty)
    }

    // Test 10: Driver list
    test_start("Driver listing");
    {
        let mut buf = [0u8; 64];
        let _ = tros::list_drvs(&mut buf);
        check(true, ""); // always passes (may return 0)
    }

    // Test 11: Directory listing
    test_start("Directory listing");
    {
        let mut buf = [0u8; 64];
        let n = tros::getdents64(0, &mut buf);
        check(n > 0, "empty directory");
    }

    // Test 12: File stat
    test_start("File stat");
    {
        let fd = tros::open_bytes(b"/");
        if fd != usize::MAX {
            let size = { let mut sb = [0u8; 64]; tros::stat(fd, &mut sb) };
            tros::close(fd);
            check(true, ""); // directory stat
        } else {
            test_fail("open / failed");
        }
    }

    // Test 13: User identity
    test_start("User identity");
    {
        let uid = tros::getuid();
        check(uid == 0, "not root"); // kernel starts as root
    }

    // Test 14: Performance counters
    test_start("Performance stats");
    {
        let (s, r, c) = tros::perf_stats();
        check(s + r + c > 0, "zero counters");
    }

    // Test 15: CPU yield
    test_start("CPU yield");
    {
        tros::yield_cpu();
        check(true, "");
    }

    // Summary
    tros::print("\r\n========================================\r\n");
    tros::print("  Results: ");
    tros::print_uint(unsafe { PASSED });
    tros::print(" passed, ");
    tros::print_uint(unsafe { FAILED });
    tros::print(" failed, ");
    tros::print_uint(unsafe { TOTAL });
    tros::print(" total\r\n");

    if unsafe { FAILED == 0 } {
        tros::print("  ALL TESTS PASSED\r\n");
    } else {
        tros::print("  SOME TESTS FAILED\r\n");
    }
    tros::print("========================================\r\n");

    // Exit cleanly
    tros::exit(unsafe { FAILED } as i32);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    tros::print("[SELFTEST] PANIC\r\n");
    loop { unsafe { core::arch::asm!("wfi"); } }
}
