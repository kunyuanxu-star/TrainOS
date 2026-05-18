// Time-related syscalls: nanosleep, gettimeofday, clock_gettime

/// sys_nanosleep(req_ptr, rem_ptr) — sleep for specified nanoseconds.
/// Blocks the current thread for approximately the requested time.
pub fn sys_nanosleep(_req_ptr: usize, _rem_ptr: usize) -> Result<usize, &'static str> {
    // Approximate: each timer tick is 10ms = 10,000,000 ns
    // For simplicity, we yield and let the scheduler handle timing
    // Real implementation would block until timer expiry

    let current_ticks = unsafe { crate::trap::TICK_COUNT };
    let target_ticks = current_ticks + 1; // sleep at least 1 tick (10ms)

    // Spin-wait (not ideal, but works for now)
    loop {
        unsafe {
            if crate::trap::TICK_COUNT >= target_ticks {
                break;
            }
        }
        // Yield to other threads while waiting
        crate::sched::schedule();
    }

    Ok(0)
}

/// sys_gettimeofday(tv_ptr, tz_ptr) — get current time.
/// Simplified: returns uptime in ms as seconds+microseconds.
pub fn sys_gettimeofday(tv_ptr: usize, _tz_ptr: usize) -> Result<usize, &'static str> {
    let ticks = unsafe { crate::trap::TICK_COUNT };
    let ms = ticks * 10; // each tick is 10ms
    let sec = ms / 1000;
    let usec = (ms - sec * 1000) * 1000;

    if tv_ptr != 0 {
        unsafe {
            // struct timeval { tv_sec: i64, tv_usec: i64 }
            let ptr = tv_ptr as *mut u64;
            ptr.write_volatile(sec as u64);
            ptr.add(1).write_volatile(usec as u64);
        }
    }

    Ok(0)
}

/// sys_clock_gettime(clk_id, ts_ptr) — get clock time.
/// clk_id: 0=REALTIME, 1=MONOTONIC
pub fn sys_clock_gettime(clk_id: usize, ts_ptr: usize) -> Result<usize, &'static str> {
    let ticks = unsafe { crate::trap::TICK_COUNT };
    let ms = ticks * 10;

    match clk_id {
        0 => {
            // CLOCK_REALTIME — same as gettimeofday
            let sec = ms / 1000;
            let nsec = (ms - sec * 1000) * 1_000_000;
            if ts_ptr != 0 {
                unsafe {
                    // struct timespec { tv_sec: i64, tv_nsec: i64 }
                    let ptr = ts_ptr as *mut u64;
                    ptr.write_volatile(sec as u64);
                    ptr.add(1).write_volatile(nsec as u64);
                }
            }
            Ok(0)
        }
        1 => {
            // CLOCK_MONOTONIC — uptime
            let sec = ms / 1000;
            let nsec = (ms - sec * 1000) * 1_000_000;
            if ts_ptr != 0 {
                unsafe {
                    let ptr = ts_ptr as *mut u64;
                    ptr.write_volatile(sec as u64);
                    ptr.add(1).write_volatile(nsec as u64);
                }
            }
            Ok(0)
        }
        _ => Err("unsupported clock"),
    }
}
