// V37b — GUI Service IPC Protocol
//
// Defines the IPC protocol between the user-space GUI service (EP 9)
// and the kernel's framebuffer/window manager.
//
// The GUI service is a user-space process that manages windows,
// renders widgets, and dispatches input events to applications.
// It communicates with the kernel framebuffer via syscalls and
// with other services via IPC.

use super::framebuffer;
use super::graphics::{self, font_8x16, Color, Rect};
use super::input;
use super::window::WindowManager;
use crate::ipc::message::Message;

/// Well-known endpoint for the GUI service.
pub const GUI_EP: usize = 9;

/// Default GUI service priority.
pub const GUI_PRIORITY: u8 = 47;

// ── GUI Service IPC Opcodes ───────────────────────────────────────────────
//
// The GUI service listens on EP 9.  Clients send messages with
// an opcode byte at payload[0].  Each opcode has a fixed parameter
// layout described below.

/// Opcode: Create a new window.
/// Payload: [op:1][title:?][x:4][y:4][w:4][h:4]
/// Reply: [window_id:4] (0 = failure)
pub const WINDOW_CREATE: u8 = 1;

/// Opcode: Destroy a window.
/// Payload: [op:1][window_id:4]
pub const WINDOW_DESTROY: u8 = 2;

/// Opcode: Put text in a window.
/// Payload: [op:1][window_id:4][x:4][y:4][color:4][text:?]
pub const PUT_TEXT: u8 = 3;

/// Opcode: Fill a rectangle in a window.
/// Payload: [op:1][window_id:4][x:4][y:4][w:4][h:4][color:4]
pub const FILL_RECT: u8 = 4;

/// Opcode: Draw a line in a window.
/// Payload: [op:1][window_id:4][x0:4][y0:4][x1:4][y1:4][color:4]
pub const DRAW_LINE: u8 = 5;

/// Opcode: Get next input event (blocking).
/// Payload: [op:1]
/// Reply: [event_type:1][data:?]
pub const GET_INPUT: u8 = 6;

/// Opcode: Refresh (flush) a window.
/// Payload: [op:1][window_id:4]
pub const REFRESH: u8 = 7;

/// Opcode: Get framebuffer info.
/// Payload: [op:1]
/// Reply: [phys_base:4][width:4][height:4][bpp:4][stride:4][size:4]
pub const GET_FB_INFO: u8 = 8;

/// Opcode: Move a window.
/// Payload: [op:1][window_id:4][x:4][y:4]
pub const WINDOW_MOVE: u8 = 9;

/// Opcode: Resize a window.
/// Payload: [op:1][window_id:4][w:4][h:4]
pub const WINDOW_RESIZE: u8 = 10;

/// Opcode: Redraw all windows.
/// Payload: [op:1]
pub const REDRAW_ALL: u8 = 11;

/// Opcode: Draw pixel.
/// Payload: [op:1][window_id:4][x:4][y:4][color:4]
pub const DRAW_PIXEL: u8 = 12;

/// Opcode: Invalidate/refresh entire screen.
/// Payload: [op:1]
pub const SCREEN_REFRESH: u8 = 13;

// ── Message Helpers ───────────────────────────────────────────────────────

/// Read a u32 from a byte slice at a given offset (little-endian).
fn read_u32(buf: &[u8], offset: usize) -> u32 {
    if offset + 4 > buf.len() {
        return 0;
    }
    (buf[offset] as u32)
        | ((buf[offset + 1] as u32) << 8)
        | ((buf[offset + 2] as u32) << 16)
        | ((buf[offset + 3] as u32) << 24)
}

/// Write a u32 to a byte slice at a given offset (little-endian).
fn write_u32(buf: &mut [u8], offset: usize, val: u32) {
    if offset + 4 > buf.len() {
        return;
    }
    buf[offset] = val as u8;
    buf[offset + 1] = (val >> 8) as u8;
    buf[offset + 2] = (val >> 16) as u8;
    buf[offset + 3] = (val >> 24) as u8;
}

/// Read a u16 from a byte slice at a given offset (little-endian).
fn read_u16(buf: &[u8], offset: usize) -> u16 {
    if offset + 2 > buf.len() {
        return 0;
    }
    (buf[offset] as u16) | ((buf[offset + 1] as u16) << 8)
}

// ── Syscall Handlers (kernel-side) ────────────────────────────────────────

/// Handle a framebuffer info request from user space.
/// Returns (phys_base, width, height, bpp, stride, size) packed into a usize slice.
pub fn sys_fb_info(buf: &mut [u8]) -> Result<usize, &'static str> {
    let (phys, _pages_ptr, pages_count, w, h, bpp, stride) = framebuffer::fb_get_info();
    if phys == 0 {
        return Err("framebuffer not initialized");
    }
    let fb_size = (w * h * bpp) as usize;

    let info = [
        phys,
        w,
        h,
        bpp,
        stride,
        fb_size,
        pages_count,
    ];

    // Write to user buffer
    let bytes = core::cmp::min(buf.len(), 28);
    for (i, &val) in info.iter().enumerate() {
        if i * 4 + 4 > bytes { break; }
        write_u32(buf, i * 4, val as u32);
    }

    Ok(bytes)
}

/// Handle a framebuffer flush request.
pub fn sys_fb_flush() -> Result<usize, &'static str> {
    if let Some(fb) = framebuffer::fb_instance() {
        fb.flush();
        Ok(0)
    } else {
        Err("framebuffer not initialized")
    }
}

/// Handle an input poll request.
/// Returns the serialized event size, or 0 if no events.
pub fn sys_input_poll(buf: &mut [u8]) -> Result<usize, &'static str> {
    if let Some(event) = input::input_pop() {
        let mut event_buf = [0u8; 16];
        let len = event.serialize(&mut event_buf);
        let copy_len = core::cmp::min(len, buf.len());
        for i in 0..copy_len {
            buf[i] = event_buf[i];
        }
        Ok(len)
    } else {
        Ok(0) // No event available
    }
}

/// Handle block on input (called when GUI service waits for input).
/// Returns the serialized event size.
pub fn sys_input_wait(buf: &mut [u8]) -> Result<usize, &'static str> {
    // Spin until an event is available (the scheduler will yield)
    loop {
        if let Some(event) = input::input_pop() {
            let mut event_buf = [0u8; 16];
            let len = event.serialize(&mut event_buf);
            let copy_len = core::cmp::min(len, buf.len());
            for i in 0..copy_len {
                buf[i] = event_buf[i];
            }
            return Ok(len);
        }
        // Yield to other processes
        crate::sched::schedule();
    }
}

/// Map a physical page into the calling process's address space.
/// Wrapper around the existing sys_map_mmio logic using fb physical addresses.
pub fn sys_fb_map_page(pid: u32, page_index: u32) -> Result<usize, &'static str> {
    let (phys_base, page_count) = framebuffer::fb_get_pages();
    if phys_base == 0 {
        return Err("framebuffer not initialized");
    }
    if page_index as usize >= page_count {
        return Err("page index out of range");
    }

    // Each page is at phys_base + page_index * PAGE_SIZE
    let page_phys = phys_base + (page_index as usize) * crate::mem::layout::PAGE_SIZE;

    // Find process page table
    let procs = crate::proc::PROCESSES.lock();
    let mut root_pt = 0;
    for proc in procs.iter() {
        if proc.pid == pid {
            root_pt = proc.page_table_root;
            break;
        }
    }
    drop(procs);

    if root_pt == 0 {
        return Err("process not found");
    }

    let va = crate::proc::elf::map_phys_to_user(root_pt, page_phys, crate::mem::layout::PAGE_SIZE);
    Ok(va)
}

// ── Global Window Manager Instance ─────────────────────────────────────────

static mut WM: Option<WindowManager> = None;

/// Initialize the window manager.
pub fn gui_init() {
    unsafe {
        if WM.is_none() {
            WM = Some(WindowManager::new(
                framebuffer::FB_DEFAULT_WIDTH,
                framebuffer::FB_DEFAULT_HEIGHT,
            ));
            crate::println!("  V37b: Window manager initialized ({}x{})",
                framebuffer::FB_DEFAULT_WIDTH,
                framebuffer::FB_DEFAULT_HEIGHT,
            );
        }
    }
}

/// Access the window manager.
pub fn wm() -> Option<&'static mut WindowManager> {
    unsafe { WM.as_mut() }
}

/// Redraw all windows.
pub fn gui_redraw() {
    if let Some(fb) = framebuffer::fb_instance() {
        if let Some(wm) = unsafe { WM.as_mut() } {
            wm.redraw_all(fb);
        }
    }
}

/// Redraw a specific window.
pub fn gui_redraw_window(win_id: usize) {
    if let Some(fb) = framebuffer::fb_instance() {
        if let Some(wm) = unsafe { WM.as_mut() } {
            for i in 0..32 {
                if wm.window_by_idx(i).map_or(false, |w| w.id == win_id) {
                    wm.redraw_window(fb, i);
                    break;
                }
            }
        }
    }
}
