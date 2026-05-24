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

// ── V30 Time syscalls ────────────────────────────────────────────────────────

static mut WALL_CLOCK_SECONDS: u64 = 0;
static mut WALL_CLOCK_MICROSECONDS: u64 = 0;

/// sys_settimeofday(tv_ptr, tz_ptr) — set wall clock time.
pub fn sys_settimeofday(tv_ptr: usize, _tz_ptr: usize) -> Result<usize, &'static str> {
    if tv_ptr == 0 { return Err("null tv"); }
    unsafe {
        let sec = (tv_ptr as *const u64).read_volatile();
        let usec = (tv_ptr as *const u64).add(1).read_volatile();
        WALL_CLOCK_SECONDS = sec;
        WALL_CLOCK_MICROSECONDS = usec;
    }
    Ok(0)
}

/// POSIX timer infrastructure
const MAX_TIMERS: usize = 16;

#[derive(Clone, Copy)]
struct PosixTimer {
    id: usize,
    pid: u32,
    clock_id: usize,
    interval_sec: u64,
    interval_nsec: u64,
    value_sec: u64,
    value_nsec: u64,
    active: bool,
}

static mut POSIX_TIMERS: [PosixTimer; MAX_TIMERS] = [
    PosixTimer { id: 0, pid: 0, clock_id: 0, interval_sec: 0, interval_nsec: 0,
                 value_sec: 0, value_nsec: 0, active: false };
    MAX_TIMERS
];
static mut POSIX_TIMER_COUNT: usize = 0;

/// sys_timer_create(clock_id, sevp_ptr, timer_id_ptr) — create a POSIX timer.
pub fn sys_timer_create(clock_id: usize, _sevp_ptr: usize, timer_id_ptr: usize) -> Result<usize, &'static str> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner }).unwrap_or(0);
    unsafe {
        if POSIX_TIMER_COUNT >= MAX_TIMERS { return Err("timer table full"); }
        let id = POSIX_TIMER_COUNT;
        POSIX_TIMERS[id] = PosixTimer {
            id, pid, clock_id,
            interval_sec: 0, interval_nsec: 0,
            value_sec: 0, value_nsec: 0,
            active: false,
        };
        POSIX_TIMER_COUNT += 1;
        if timer_id_ptr != 0 {
            (timer_id_ptr as *mut usize).write_volatile(id);
        }
        Ok(0)
    }
}

/// sys_timer_delete(timer_id) — delete a POSIX timer.
pub fn sys_timer_delete(timer_id: usize) -> Result<usize, &'static str> {
    unsafe {
        if timer_id >= POSIX_TIMER_COUNT { return Err("bad timer id"); }
        POSIX_TIMERS[timer_id].active = false;
        Ok(0)
    }
}

/// sys_timer_settime(timer_id, flags, new_value_ptr, old_value_ptr) — arm/disarm a timer.
pub fn sys_timer_settime(timer_id: usize, _flags: usize, new_value_ptr: usize, old_value_ptr: usize) -> Result<usize, &'static str> {
    unsafe {
        if timer_id >= POSIX_TIMER_COUNT { return Err("bad timer id"); }
        // Return old value
        if old_value_ptr != 0 {
            let old_ptr = old_value_ptr as *mut u64;
            old_ptr.write_volatile(POSIX_TIMERS[timer_id].value_sec);
            old_ptr.add(1).write_volatile(POSIX_TIMERS[timer_id].value_nsec);
        }
        // Set new value
        if new_value_ptr != 0 {
            let new_ptr = new_value_ptr as *const u64;
            POSIX_TIMERS[timer_id].value_sec = new_ptr.read_volatile();
            POSIX_TIMERS[timer_id].value_nsec = new_ptr.add(1).read_volatile();
            let interval_ptr = new_value_ptr as *const u64;
            POSIX_TIMERS[timer_id].interval_sec = interval_ptr.add(2).read_volatile();
            POSIX_TIMERS[timer_id].interval_nsec = interval_ptr.add(3).read_volatile();
            POSIX_TIMERS[timer_id].active = true;
        }
        Ok(0)
    }
}

/// sys_timer_gettime(timer_id, curr_value_ptr) — get timer status.
pub fn sys_timer_gettime(timer_id: usize, curr_value_ptr: usize) -> Result<usize, &'static str> {
    unsafe {
        if timer_id >= POSIX_TIMER_COUNT { return Err("bad timer id"); }
        if curr_value_ptr != 0 {
            let ptr = curr_value_ptr as *mut u64;
            ptr.write_volatile(POSIX_TIMERS[timer_id].value_sec);
            ptr.add(1).write_volatile(POSIX_TIMERS[timer_id].value_nsec);
        }
        Ok(0)
    }
}
