// epoll syscalls — I/O event notification
//
// Simplified epoll implementation suitable for microkernel IPC.
// epoll_create: creates an endpoint for event notification
// epoll_ctl: register/unregister fd for events
// epoll_wait: wait for events on registered fds

use crate::ipc::message::Message;

fn current_pid() -> u32 {
    crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0)
}

const MAX_EPOLL_FDS: usize = 16;

// Epoll event structure (packed into u64 for user space)
// struct epoll_event {
//     events: u32,      // EPOLLIN=1, EPOLLOUT=4, EPOLLERR=8
//     data: u64,        // user data
// }

static mut EPOLL_FDS: [(u32, usize); MAX_EPOLL_FDS] = [(0, 0); MAX_EPOLL_FDS];
static mut EPOLL_COUNT: usize = 0;

/// sys_epoll_create(size) — create an epoll instance.
/// Returns epoll fd (endpoint id).
pub fn sys_epoll_create(_size: usize) -> Result<usize, &'static str> {
    let ep = crate::ipc::create_endpoint();
    Ok(ep)
}

/// sys_epoll_ctl(epfd, op, fd, events) — control epoll instance.
/// op: 1=ADD, 2=DEL, 3=MOD
pub fn sys_epoll_ctl(epfd: usize, op: usize, fd: usize, _events: usize) -> Result<usize, &'static str> {
    let pid = current_pid();

    unsafe {
        match op {
            1 => {
                // EPOLL_CTL_ADD
                if EPOLL_COUNT >= MAX_EPOLL_FDS {
                    return Err("epoll table full");
                }
                EPOLL_FDS[EPOLL_COUNT] = (pid, fd);
                EPOLL_COUNT += 1;

                // Notify epfd about the registration
                let mut msg = Message::new(pid, 0);
                msg.payload[0] = 1; // EPOLLIN
                msg.payload_len = 1;

                // Store epfd association
                let _ = epfd; // epfd is the listening endpoint
                Ok(0)
            }
            2 => {
                // EPOLL_CTL_DEL
                for i in 0..EPOLL_COUNT {
                    if EPOLL_FDS[i].0 == pid && EPOLL_FDS[i].1 == fd {
                        EPOLL_FDS[i] = EPOLL_FDS[EPOLL_COUNT - 1];
                        EPOLL_COUNT -= 1;
                        break;
                    }
                }
                Ok(0)
            }
            3 => {
                // EPOLL_CTL_MOD — re-register with new events
                Ok(0)
            }
            _ => Err("invalid epoll op"),
        }
    }
}

/// sys_epoll_wait(epfd, events_ptr, maxevents, timeout) — wait for events.
/// Returns number of events.
pub fn sys_epoll_wait(
    epfd: usize, events_ptr: usize, maxevents: usize, _timeout: isize,
) -> Result<usize, &'static str> {
    let sender_pid = current_pid();

    // Poll: check each registered fd for incoming data
    let mut ready_count: usize = 0;

    for _ in 0..maxevents {
        // Try to receive from epfd (non-blocking check)
        match crate::ipc::endpoint::recv(epfd, sender_pid) {
            Ok(msg) => {
                if events_ptr != 0 && ready_count < maxevents {
                    let event_offset = ready_count * 12; // 4 (events) + 8 (data) = 12 bytes per event
                    unsafe {
                        let buf = events_ptr as *mut u32;
                        buf.add(event_offset / 4).write_volatile(1); // EPOLLIN
                        let data_ptr = events_ptr as *mut u64;
                        data_ptr.add(event_offset / 8 + 1).write_volatile(msg.opcode as u64);
                    }
                }
                ready_count += 1;
            }
            Err(_) => {
                // Would block — schedule away and retry
                if ready_count == 0 {
                    crate::sched::schedule();
                    continue; // retry
                } else {
                    break; // have some events, return them
                }
            }
        }
    }

    Ok(ready_count)
}
