// V37b — Simple Framebuffer Driver
//
// Provides a memory-backed framebuffer for the GUI subsystem.
// Allocates pages from the buddy allocator and provides them
// to the user-space GUI service via framebuffer-mapping syscall.
//
// The framebuffer is a simple RGBA32 buffer at a known set of
// physical pages that the GUI service can map into its address space.

use crate::mem::{buddy, layout::PAGE_SIZE, sv39};

/// Default framebuffer width in pixels.
pub const FB_DEFAULT_WIDTH: u32 = 1024;

/// Default framebuffer height in pixels.
pub const FB_DEFAULT_HEIGHT: u32 = 768;

/// Bytes per pixel (RGBA32).
pub const FB_BYTES_PER_PIXEL: u32 = 4;

/// Maximum number of physical pages the framebuffer may use.
const FB_MAX_PAGES: usize = 4096; // 16 MB at most

/// Framebuffer driver with double buffering and dirty tracking.
pub struct Framebuffer {
    /// Physical addresses of the framebuffer pages (contiguous virtual view).
    pages: [usize; FB_MAX_PAGES],
    /// Number of physical pages used.
    page_count: usize,
    /// Kernel virtual address (linear mapping of all pages).
    fb_virt: usize,
    /// Width in pixels.
    width: u32,
    /// Height in pixels.
    height: u32,
    /// Bytes per pixel (always 4 for RGBA32).
    bpp: u32,
    /// Stride (bytes per row).
    stride: u32,
    /// Total size in bytes.
    fb_size: usize,
    /// Back buffer physical pages.
    back_pages: [usize; FB_MAX_PAGES],
    /// Number of back buffer pages.
    back_page_count: usize,
    /// Back buffer kernel virtual address.
    back_virt: usize,
    /// Dirty rectangle tracking.
    dirty_min_x: u32,
    dirty_min_y: u32,
    dirty_max_x: u32,
    dirty_max_y: u32,
    /// Whether initialized.
    initialized: bool,
}

impl Framebuffer {
    /// Create a new framebuffer with given dimensions.
    pub fn new(width: u32, height: u32) -> Option<Self> {
        let bpp = FB_BYTES_PER_PIXEL;
        let stride = width * bpp;
        let fb_size = (stride * height) as usize;

        if fb_size == 0 || fb_size > FB_MAX_PAGES * PAGE_SIZE {
            return None;
        }

        let page_count = (fb_size + PAGE_SIZE - 1) / PAGE_SIZE;
        if page_count > FB_MAX_PAGES {
            return None;
        }

        let mut pages = [0usize; FB_MAX_PAGES];
        let mut fb_virt = 0usize;

        // Allocate physical pages and map them linearly into kernel space.
        for i in 0..page_count {
            let phys = buddy::alloc_page()?;
            let kva = sv39::pa_to_kva(phys);
            unsafe {
                core::ptr::write_bytes(kva as *mut u8, 0, PAGE_SIZE);
            }
            if i == 0 {
                fb_virt = kva;
            } else {
                // Ensure pages are virtually contiguous (they are already
                // contiguous in the kernel linear map if allocated sequentially).
                debug_assert!(
                    kva == fb_virt + i * PAGE_SIZE,
                    "framebuffer: non-contiguous pages!"
                );
            }
            pages[i] = phys;
        }

        Some(Framebuffer {
            pages,
            page_count,
            fb_virt,
            width,
            height,
            bpp,
            stride,
            fb_size,
            back_pages: [0usize; FB_MAX_PAGES],
            back_page_count: 0,
            back_virt: 0,
            dirty_min_x: 0,
            dirty_min_y: 0,
            dirty_max_x: width - 1,
            dirty_max_y: height - 1,
            initialized: true,
        })
    }

    /// Create with default dimensions.
    pub fn new_default() -> Option<Self> {
        Self::new(FB_DEFAULT_WIDTH, FB_DEFAULT_HEIGHT)
    }

    // ── Accessors ──────────────────────────────────────────────────────────

    pub fn phys_base(&self) -> usize {
        if self.page_count > 0 { self.pages[0] } else { 0 }
    }

    pub fn pages_slice(&self) -> &[usize] {
        &self.pages[..self.page_count]
    }

    pub fn page_count(&self) -> usize {
        self.page_count
    }

    pub fn virt_base(&self) -> usize {
        self.fb_virt
    }

    pub fn width(&self) -> u32 { self.width }
    pub fn height(&self) -> u32 { self.height }
    pub fn bpp(&self) -> u32 { self.bpp }
    pub fn stride(&self) -> u32 { self.stride }
    pub fn size(&self) -> usize { self.fb_size }
    pub fn initialized(&self) -> bool { self.initialized }

    // ── Pixel Operations ───────────────────────────────────────────────────

    /// Write a single pixel.
    pub fn put_pixel(&mut self, x: u32, y: u32, color: u32) {
        if !self.initialized || x >= self.width || y >= self.height {
            return;
        }
        let offset = (y * self.stride + x * self.bpp) as usize;
        unsafe {
            core::ptr::write_volatile((self.fb_virt + offset) as *mut u32, color);
        }
        self.mark_dirty(x, y, 1, 1);
    }

    /// Read a pixel.
    pub fn get_pixel(&self, x: u32, y: u32) -> u32 {
        if !self.initialized || x >= self.width || y >= self.height {
            return 0;
        }
        let offset = (y * self.stride + x * self.bpp) as usize;
        unsafe {
            core::ptr::read_volatile((self.fb_virt + offset) as *const u32)
        }
    }

    /// Fill a rectangle with a color.
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        if !self.initialized || w == 0 || h == 0 {
            return;
        }
        let x_end = core::cmp::min(x + w, self.width);
        let y_end = core::cmp::min(y + h, self.height);

        for row in y..y_end {
            let offset = (row * self.stride) as usize;
            unsafe {
                let ptr = (self.fb_virt + offset) as *mut u32;
                for col in x..x_end {
                    core::ptr::write_volatile(ptr.add(col as usize), color);
                }
            }
        }
        self.mark_dirty(x, y, x_end - x, y_end - y);
    }

    /// Draw a line using Bresenham's algorithm.
    pub fn draw_line(&mut self, x0: u32, y0: u32, x1: u32, y1: u32, color: u32) {
        if !self.initialized { return; }

        let mut x = x0 as i32;
        let mut y = y0 as i32;
        let dx = (x1 as i32 - x0 as i32).abs();
        let dy = -(y1 as i32 - y0 as i32).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            if x >= 0 && y >= 0 {
                let ux = x as u32;
                let uy = y as u32;
                if ux < self.width && uy < self.height {
                    let offset = (uy * self.stride + ux * self.bpp) as usize;
                    unsafe {
                        core::ptr::write_volatile(
                            (self.fb_virt + offset) as *mut u32, color,
                        );
                    }
                }
            }
            if x == x1 as i32 && y == y1 as i32 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x += sx; }
            if e2 <= dx { err += dx; y += sy; }
        }
    }

    /// Draw an 8x16 bitmap character.
    pub fn draw_char(&mut self, x: u32, y: u32, ch: char, fg: u32, bg: u32, font: &[u8]) {
        if !self.initialized { return; }
        let code = ch as usize;
        if code >= 128 { return; }
        const CHAR_HEIGHT: usize = 16;
        const CHAR_WIDTH: usize = 8;
        let base = code * CHAR_HEIGHT;

        for row in 0..CHAR_HEIGHT {
            if y + row as u32 >= self.height { break; }
            let row_data = font.get(base + row).copied().unwrap_or(0);
            for col in 0..CHAR_WIDTH {
                if x + col as u32 >= self.width { break; }
                let color = if (row_data >> (7 - col)) & 1 != 0 { fg } else { bg };
                let offset = ((y + row as u32) * self.stride + (x + col as u32) * self.bpp) as usize;
                unsafe {
                    core::ptr::write_volatile((self.fb_virt + offset) as *mut u32, color);
                }
            }
        }
        self.mark_dirty(x, y, 8, 16);
    }

    /// Draw a text string.
    pub fn draw_text(&mut self, x: u32, y: u32, text: &str, fg: u32, bg: u32, font: &[u8]) {
        let mut cx = x;
        for ch in text.chars() {
            if cx + 8 > self.width { break; }
            self.draw_char(cx, y, ch, fg, bg, font);
            cx += 8;
        }
    }

    /// Blit pixel data from a buffer.
    pub fn blit(&mut self, x: u32, y: u32, w: u32, h: u32, data: &[u32]) {
        if !self.initialized || w == 0 || h == 0 { return; }
        let x_end = core::cmp::min(x + w, self.width);
        let y_end = core::cmp::min(y + h, self.height);

        for row in 0..(y_end - y) {
            let fb_offset = ((y + row) * self.stride) as usize;
            let data_offset = (row * w) as usize;
            unsafe {
                let fb_ptr = (self.fb_virt + fb_offset) as *mut u32;
                for col in 0..(x_end - x) {
                    if let Some(pixel) = data.get(data_offset + col as usize) {
                        core::ptr::write_volatile(fb_ptr.add((x + col) as usize), *pixel);
                    }
                }
            }
        }
        self.mark_dirty(x, y, x_end - x, y_end - y);
    }

    /// Copy a rectangle (with overlap-safe direction).
    pub fn copy_rect(&mut self, sx: u32, sy: u32, dx: u32, dy: u32, w: u32, h: u32) {
        if !self.initialized || w == 0 || h == 0 { return; }
        let x_end = core::cmp::min(sx + w, self.width);
        let y_end = core::cmp::min(sy + h, self.height);

        let rows: Vec<u32> = if sy < dy {
            (0..(y_end - sy)).rev().collect()
        } else {
            (0..(y_end - sy)).collect()
        };

        for &row in &rows {
            let src_y = sy + row;
            let dst_y = dy + row;
            for col in 0..(x_end - sx) {
                let src_x = sx + col;
                let dst_x = dx + col;
                if src_x < self.width && dst_x < self.width
                    && src_y < self.height && dst_y < self.height
                {
                    let src_off = (src_y * self.stride + src_x * self.bpp) as usize;
                    let dst_off = (dst_y * self.stride + dst_x * self.bpp) as usize;
                    unsafe {
                        let val = core::ptr::read_volatile((self.fb_virt + src_off) as *const u32);
                        core::ptr::write_volatile((self.fb_virt + dst_off) as *mut u32, val);
                    }
                }
            }
        }
        self.mark_dirty(dx, dy, w, h);
    }

    // ── Dirty Tracking ─────────────────────────────────────────────────────

    pub fn mark_dirty(&mut self, x: u32, y: u32, w: u32, h: u32) {
        if w == 0 || h == 0 { return; }
        if x < self.dirty_min_x { self.dirty_min_x = x; }
        if y < self.dirty_min_y { self.dirty_min_y = y; }
        let xe = core::cmp::min(x + w, self.width);
        let ye = core::cmp::min(y + h, self.height);
        if xe > 0 && xe - 1 > self.dirty_max_x { self.dirty_max_x = xe - 1; }
        if ye > 0 && ye - 1 > self.dirty_max_y { self.dirty_max_y = ye - 1; }
    }

    pub fn clear_dirty(&mut self) {
        self.dirty_min_x = self.width;
        self.dirty_min_y = self.height;
        self.dirty_max_x = 0;
        self.dirty_max_y = 0;
    }

    /// Flush dirty region (no-op for memory-backed framebuffer).
    pub fn flush(&mut self) {
        // For a memory-backed fb, volatile writes are already visible.
        self.clear_dirty();
    }

    // ── Double Buffering ───────────────────────────────────────────────────

    /// Create a back buffer for double buffering.
    pub fn create_back_buffer(&mut self) -> bool {
        if self.back_page_count > 0 { return true; }
        let pc = self.page_count;
        let mut back_pages = [0usize; FB_MAX_PAGES];

        for i in 0..pc {
            match buddy::alloc_page() {
                Some(p) => {
                    let kva = sv39::pa_to_kva(p);
                    unsafe { core::ptr::write_bytes(kva as *mut u8, 0, PAGE_SIZE); }
                    back_pages[i] = p;
                    if i == 0 {
                        self.back_virt = kva;
                    }
                }
                None => {
                    // Free already allocated back pages
                    for j in 0..i {
                        buddy::free_page(back_pages[j]);
                    }
                    return false;
                }
            }
        }

        self.back_pages = back_pages;
        self.back_page_count = pc;
        true
    }

    /// Swap buffers (copy back to front).
    pub fn swap_buffers(&mut self) {
        if self.back_page_count == 0 { return; }
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.back_virt as *const u8,
                self.fb_virt as *mut u8,
                self.fb_size,
            );
        }
        self.mark_dirty(0, 0, self.width, self.height);
        self.flush();
    }

    /// Write pixel to back buffer.
    pub fn back_put_pixel(&mut self, x: u32, y: u32, color: u32) {
        if self.back_page_count == 0 || x >= self.width || y >= self.height { return; }
        let offset = (y * self.stride + x * self.bpp) as usize;
        unsafe {
            core::ptr::write_volatile((self.back_virt + offset) as *mut u32, color);
        }
    }

    /// Fill rect in back buffer.
    pub fn back_fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: u32) {
        if self.back_page_count == 0 || w == 0 || h == 0 { return; }
        let x_end = core::cmp::min(x + w, self.width);
        let y_end = core::cmp::min(y + h, self.height);
        for row in y..y_end {
            let offset = (row * self.stride) as usize;
            unsafe {
                let ptr = (self.back_virt + offset) as *mut u32;
                for col in x..x_end {
                    core::ptr::write_volatile(ptr.add(col as usize), color);
                }
            }
        }
    }

    /// Clear entire framebuffer with a color.
    pub fn clear(&mut self, color: u32) {
        if !self.initialized { return; }
        let total = self.fb_size / 4;
        unsafe {
            for i in 0..total {
                core::ptr::write_volatile((self.fb_virt as *mut u32).add(i), color);
            }
        }
        self.mark_dirty(0, 0, self.width, self.height);
    }
}

// ── Global Instance ────────────────────────────────────────────────────────

static mut FB_INSTANCE: Option<Framebuffer> = None;
static mut FB_ALLOC_PAGES: [usize; FB_MAX_PAGES] = [0usize; FB_MAX_PAGES];
static mut FB_ALLOC_COUNT: usize = 0;

/// Initialize the global framebuffer.
pub fn fb_init() -> bool {
    unsafe {
        if FB_INSTANCE.is_some() {
            return true;
        }
    }

    // Allocate pages and store them for mapping into user space
    let fb = Framebuffer::new_default();

    match fb {
        Some(mut f) => {
            // Store the physical pages for later user-space mapping
            unsafe {
                for i in 0..f.page_count {
                    FB_ALLOC_PAGES[i] = f.pages[i];
                }
                FB_ALLOC_COUNT = f.page_count;
                FB_INSTANCE = Some(f);
            }

            crate::println!(
                "  V37b: Framebuffer: {}x{}x{} ({} KiB, {} pages)",
                FB_DEFAULT_WIDTH,
                FB_DEFAULT_HEIGHT,
                FB_BYTES_PER_PIXEL * 8,
                (FB_DEFAULT_WIDTH * FB_DEFAULT_HEIGHT * FB_BYTES_PER_PIXEL) / 1024,
                unsafe { FB_ALLOC_COUNT },
            );
            true
        }
        None => {
            crate::println!("  WARNING: V37b: Framebuffer init failed");
            false
        }
    }
}

/// Get mutable reference to the global framebuffer.
pub fn fb_instance() -> Option<&'static mut Framebuffer> {
    unsafe { FB_INSTANCE.as_mut() }
}

/// Get framebuffer info.
/// Returns (phys_base, phys_pages_ptr, page_count, width, height, bpp, stride).
pub fn fb_get_info() -> (usize, usize, usize, usize, usize, usize, usize) {
    unsafe {
        match FB_INSTANCE.as_ref() {
            Some(fb) => (
                fb.phys_base(),
                FB_ALLOC_PAGES.as_ptr() as usize,
                fb.page_count,
                fb.width as usize,
                fb.height as usize,
                fb.bpp as usize,
                fb.stride as usize,
            ),
            None => (0, 0, 0, 0, 0, 0, 0),
        }
    }
}

/// Get the list of physical pages for mapping into user space.
/// Returns (phys_base, page_count).
pub fn fb_get_pages() -> (usize, usize) {
    unsafe {
        match FB_INSTANCE.as_ref() {
            Some(fb) => (fb.phys_base(), fb.page_count),
            None => (0, 0),
        }
    }
}
