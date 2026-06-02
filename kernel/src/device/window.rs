// V37b — Simple Window Manager
//
// Provides a minimal window manager for the GUI subsystem.
// Handles window creation, destruction, Z-ordering, focus,
// title bars, and compositing to the framebuffer.

use super::framebuffer::Framebuffer;
use super::graphics::{
    self, draw_border, draw_gradient, draw_text_centered, draw_text_wrapped,
    font_8x16, Color, DESKTOP_BG, DARK_GRAY, GRAY, LIGHT_GRAY, TITLE_BG, WHITE,
    Rect,
};

/// Maximum number of windows.
const MAX_WINDOWS: usize = 32;

/// Maximum title length.
const MAX_TITLE_LEN: usize = 64;

/// Title bar height in pixels.
const TITLE_BAR_HEIGHT: u32 = 24;

/// Border width in pixels.
const WINDOW_BORDER: u32 = 2;

/// Resize handle size.
const RESIZE_HANDLE: u32 = 8;

// ── Wallpaper ──────────────────────────────────────────────────────────────

/// A full-screen backdrop image for the desktop.
pub struct Wallpaper {
    /// Pixel data (RGBA32).
    pub data: [Color; 1024 * 768],
    /// Number of valid pixels (width * height).
    pub data_len: usize,
    /// Image width.
    pub width: u32,
    /// Image height.
    pub height: u32,
    /// Whether to tile the image.
    pub tile: bool,
}

impl Wallpaper {
    /// Create a new wallpaper with a solid color.
    pub fn solid(color: Color) -> Self {
        Wallpaper {
            data: [color; 1024 * 768],
            data_len: 1,
            width: 1,
            height: 1,
            tile: true,
        }
    }

    /// Create a checkerboard wallpaper pattern.
    pub fn checkerboard() -> Self {
        let mut data = [DESKTOP_BG; 1024 * 768];
        for y in 0..768 {
            for x in 0..1024 {
                if ((x / 32) + (y / 32)) % 2 == 0 {
                    data[(y * 1024 + x) as usize] = 0xFF3B4252;
                }
            }
        }
        Wallpaper {
            data,
            data_len: (1024 * 768) as usize,
            width: 1024,
            height: 768,
            tile: false,
        }
    }
}

// ── Window ─────────────────────────────────────────────────────────────────

/// A single window managed by the window manager.
pub struct Window {
    /// Unique window identifier.
    pub id: usize,
    /// Window title (UTF-8 bytes).
    pub title: [u8; MAX_TITLE_LEN],
    /// Length of the title string.
    pub title_len: usize,
    /// Window rectangle (screen coordinates).
    pub rect: Rect,
    /// Client area rectangle (excluding title bar and borders).
    pub client_area: Rect,
    /// Whether the window is visible.
    pub is_visible: bool,
    /// Whether the window has keyboard focus.
    pub is_focused: bool,
    /// Whether the window is minimized.
    pub is_minimized: bool,
    /// Whether the window is maximized.
    pub is_maximized: bool,
    /// Saved rectangle for restore from maximized.
    pub saved_rect: Rect,
    /// Whether the window has a close button.
    pub has_close_button: bool,
    /// Whether the window has a minimize button.
    pub has_minimize_button: bool,
    /// Title bar background color.
    pub title_bar_color: Color,
    /// Window fill color (client area background).
    pub fill_color: Color,
    /// PID of the process that owns this window.
    pub owner_pid: u32,
    /// Whether the window needs redrawing.
    pub dirty: bool,
    /// Whether the window is being dragged.
    pub is_dragging: bool,
    /// X-offset within window where drag started.
    pub drag_offset_x: i32,
    /// Y-offset within window where drag started.
    pub drag_offset_y: i32,
}

impl Window {
    /// Create a new window with given properties.
    pub fn new(id: usize, title: &str, x: i32, y: i32, w: u32, h: u32, owner: u32) -> Self {
        let mut title_buf = [0u8; MAX_TITLE_LEN];
        let tlen = core::cmp::min(title.len(), MAX_TITLE_LEN);
        for (i, b) in title.bytes().enumerate().take(tlen) {
            title_buf[i] = b;
        }

        let rect = Rect::new(x, y, w, h);
        let client_area = rect.inset(
            WINDOW_BORDER as i32,
            (WINDOW_BORDER + TITLE_BAR_HEIGHT) as i32,
        );

        Window {
            id,
            title: title_buf,
            title_len: tlen,
            rect,
            client_area,
            is_visible: true,
            is_focused: false,
            is_minimized: false,
            is_maximized: false,
            saved_rect: rect,
            has_close_button: true,
            has_minimize_button: true,
            title_bar_color: TITLE_BG,
            fill_color: 0xFFF0F0F0,
            owner_pid: owner,
            dirty: true,
            is_dragging: false,
            drag_offset_x: 0,
            drag_offset_y: 0,
        }
    }

    /// Get the title as a string slice.
    pub fn title_str(&self) -> &str {
        core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")
    }

    /// Update client area after a resize.
    fn update_client_area(&mut self) {
        self.client_area = self.rect.inset(
            WINDOW_BORDER as i32,
            (WINDOW_BORDER + TITLE_BAR_HEIGHT) as i32,
        );
    }
}

// ── Window Manager ─────────────────────────────────────────────────────────

/// The window manager manages all windows on the desktop.
pub struct WindowManager {
    /// Array of managed windows.
    windows: [Option<Window>; MAX_WINDOWS],
    /// Number of active windows.
    window_count: usize,
    /// ID of the next window to create.
    next_id: usize,
    /// Index of the active (focused) window, if any.
    active_idx: Option<usize>,
    /// Desktop dimensions.
    root_width: u32,
    /// Desktop dimensions.
    root_height: u32,
    /// Z-order (front-to-back: indices into windows[]).
    z_order: [usize; MAX_WINDOWS],
    /// Number of entries in z_order.
    z_count: usize,
    /// Background color for the desktop.
    bg_color: Color,
    /// Optional wallpaper.
    wallpaper: Wallpaper,
}

impl WindowManager {
    /// Create a new window manager for a desktop of given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        WindowManager {
            windows: [const { None }; MAX_WINDOWS],
            window_count: 0,
            next_id: 1,
            active_idx: None,
            root_width: width,
            root_height: height,
            z_order: [0; MAX_WINDOWS],
            z_count: 0,
            bg_color: DESKTOP_BG,
            wallpaper: Wallpaper::checkerboard(),
        }
    }

    /// Set a custom wallpaper.
    pub fn set_wallpaper(&mut self, wallpaper: Wallpaper) {
        self.wallpaper = wallpaper;
    }

    // ── Window Lifecycle ─────────────────────────────────────────────────

    /// Create a new window and return its ID.
    pub fn create_window(
        &mut self,
        title: &str,
        x: i32,
        y: i32,
        w: u32,
        h: u32,
        owner: u32,
    ) -> Option<usize> {
        // Find a free slot
        let idx = self.windows.iter().position(|w| w.is_none())?;
        let id = self.next_id;
        self.next_id += 1;

        let win = Window::new(id, title, x, y, w, h, owner);
        self.windows[idx] = Some(win);
        self.window_count += 1;

        // Add to front of Z-order
        self.z_order[self.z_count] = idx;
        self.z_count += 1;
        self.active_idx = Some(idx);

        Some(id)
    }

    /// Destroy a window by ID.
    pub fn destroy_window(&mut self, id: usize) -> bool {
        if let Some(idx) = self.find_by_id(id) {
            self.windows[idx] = None;
            self.window_count -= 1;

            // Remove from Z-order
            let mut found = false;
            let mut new_z = [0usize; MAX_WINDOWS];
            let mut new_zc = 0;
            for i in 0..self.z_count {
                if self.z_order[i] == idx {
                    found = true;
                } else {
                    new_z[new_zc] = self.z_order[i];
                    new_zc += 1;
                }
            }
            self.z_order = new_z;
            self.z_count = new_zc;

            // Clear active if it was this window
            if self.active_idx == Some(idx) {
                self.active_idx = if self.z_count > 0 {
                    Some(self.z_order[0])
                } else {
                    None
                };
            }
            true
        } else {
            false
        }
    }

    /// Move a window to a new position.
    pub fn move_window(&mut self, id: usize, x: i32, y: i32) {
        if let Some(idx) = self.find_by_id(id) {
            if let Some(ref mut win) = self.windows[idx] {
                win.rect.x = x;
                win.rect.y = y;
                win.update_client_area();
                win.dirty = true;
            }
        }
    }

    /// Resize a window.
    pub fn resize_window(&mut self, id: usize, w: u32, h: u32) {
        if let Some(idx) = self.find_by_id(id) {
            if let Some(ref mut win) = self.windows[idx] {
                win.rect.width = w;
                win.rect.height = h;
                win.update_client_area();
                win.dirty = true;
            }
        }
    }

    /// Bring a window to the front and give it focus.
    pub fn focus_window(&mut self, id: usize) {
        if let Some(idx) = self.find_by_id(id) {
            // Remove from current position in Z-order
            let mut new_z = [0usize; MAX_WINDOWS];
            let mut new_zc = 0;
            for i in 0..self.z_count {
                if self.z_order[i] != idx {
                    new_z[new_zc] = self.z_order[i];
                    new_zc += 1;
                }
            }
            // Add to front
            new_z[new_zc] = idx;
            new_zc += 1;
            self.z_order = new_z;
            self.z_count = new_zc;

            // Update focus
            for i in 0..MAX_WINDOWS {
                if let Some(ref mut win) = self.windows[i] {
                    win.is_focused = i == idx;
                }
            }
            self.active_idx = Some(idx);
        }
    }

    /// Toggle minimize state of a window.
    pub fn toggle_minimize(&mut self, id: usize) {
        if let Some(idx) = self.find_by_id(id) {
            if let Some(ref mut win) = self.windows[idx] {
                if win.is_minimized {
                    win.is_minimized = false;
                    win.rect = win.saved_rect;
                } else {
                    win.saved_rect = win.rect;
                    win.is_minimized = true;
                    // Move off-screen (or hide)
                    win.rect = Rect::new(-32000, -32000, 0, 0);
                }
                win.update_client_area();
                win.dirty = true;
            }
        }
    }

    /// Toggle maximize state of a window.
    pub fn toggle_maximize(&mut self, id: usize) {
        if let Some(idx) = self.find_by_id(id) {
            if let Some(ref mut win) = self.windows[idx] {
                if win.is_maximized {
                    win.is_maximized = false;
                    win.rect = win.saved_rect;
                } else {
                    win.saved_rect = win.rect;
                    win.is_maximized = true;
                    win.rect = Rect::new(0, 0, self.root_width, self.root_height);
                }
                win.update_client_area();
                win.dirty = true;
            }
        }
    }

    // ── Hit Testing ──────────────────────────────────────────────────────

    /// Find the topmost window at the given screen point.
    pub fn window_at(&self, x: i32, y: i32) -> Option<(usize, bool)> {
        // Search Z-order front-to-back
        for i in 0..self.z_count {
            let idx = self.z_order[i];
            if let Some(ref win) = self.windows[idx] {
                if !win.is_visible || win.is_minimized {
                    continue;
                }
                if win.rect.contains(&graphics::Point::new(x, y)) {
                    // Check if it's on the title bar
                    let title_rect = Rect::new(
                        win.rect.x + WINDOW_BORDER as i32,
                        win.rect.y + WINDOW_BORDER as i32,
                        win.rect.width - 2 * WINDOW_BORDER,
                        TITLE_BAR_HEIGHT,
                    );
                    let on_title = title_rect.contains(&graphics::Point::new(x, y));
                    return Some((idx, on_title));
                }
            }
        }
        None
    }

    // ── Event Handling ───────────────────────────────────────────────────

    /// Handle a mouse click at the given position.
    pub fn handle_click(&mut self, x: i32, y: i32, _button: u8) {
        if let Some((idx, on_title)) = self.window_at(x, y) {
            self.focus_window(self.windows[idx].as_ref().unwrap().id);

            if on_title {
                // Start dragging
                if let Some(ref mut win) = self.windows[idx] {
                    win.is_dragging = true;
                    win.drag_offset_x = x - win.rect.x;
                    win.drag_offset_y = y - win.rect.y;
                }
            }
        }
    }

    /// Handle mouse button release.
    pub fn handle_release(&mut self, _x: i32, _y: i32) {
        for i in 0..MAX_WINDOWS {
            if let Some(ref mut win) = self.windows[i] {
                win.is_dragging = false;
            }
        }
    }

    /// Handle mouse motion (for dragging).
    pub fn handle_motion(&mut self, x: i32, y: i32) {
        for i in 0..MAX_WINDOWS {
            if let Some(ref mut win) = self.windows[i] {
                if win.is_dragging && !win.is_maximized {
                    let new_x = x - win.drag_offset_x;
                    let new_y = y - win.drag_offset_y;
                    win.rect.x = new_x;
                    win.rect.y = new_y;
                    win.update_client_area();
                    win.dirty = true;
                }
            }
        }
    }

    /// Handle keyboard input (delegate to focused window).
    pub fn handle_key(&mut self, _keycode: u8, _modifier: u8) -> Option<usize> {
        self.active_idx
    }

    // ── Rendering ────────────────────────────────────────────────────────

    /// Redraw a single window to the framebuffer.
    pub fn redraw_window(&self, fb: &mut Framebuffer, idx: usize) {
        let Some(ref win) = self.windows[idx] else { return };
        if !win.is_visible || win.is_minimized {
            return;
        }

        let font = font_8x16();

        // Draw drop shadow
        let shadow_rect = Rect::new(
            win.rect.x + 3,
            win.rect.y + 3,
            win.rect.width,
            win.rect.height,
        );
        graphics::draw_shadow(fb, &shadow_rect, 4, 40);

        // Draw window border
        draw_border(fb, &win.rect, WINDOW_BORDER, DARK_GRAY);

        // Draw window fill
        fb.fill_rect(
            (win.rect.x + WINDOW_BORDER as i32) as u32,
            (win.rect.y + WINDOW_BORDER as i32) as u32,
            win.rect.width - 2 * WINDOW_BORDER,
            win.rect.height - 2 * WINDOW_BORDER,
            win.fill_color,
        );

        // Draw title bar background
        let title_bg = if win.is_focused {
            win.title_bar_color
        } else {
            DARK_GRAY
        };
        fb.fill_rect(
            (win.rect.x + WINDOW_BORDER as i32) as u32,
            (win.rect.y + WINDOW_BORDER as i32) as u32,
            win.rect.width - 2 * WINDOW_BORDER,
            TITLE_BAR_HEIGHT,
            title_bg,
        );

        // Draw title text
        let title_str = win.title_str();
        let tx = win.rect.x + WINDOW_BORDER as i32 + 4;
        let ty = win.rect.y + WINDOW_BORDER as i32 + 4;
        draw_text_wrapped(
            fb,
            tx as u32,
            ty as u32,
            win.rect.width - 60,
            title_str,
            WHITE,
            title_bg,
        );

        // Draw close button (X)
        if win.has_close_button {
            let cb_x = win.rect.right() - WINDOW_BORDER as i32 - 20;
            let cb_y = win.rect.y + WINDOW_BORDER as i32 + 4;
            fb.fill_rect(cb_x as u32, cb_y as u32, 16, 16, 0xFFCC3333);
            // Draw X inside button
            let cx = cb_x + 8;
            let cy = cb_y + 8;
            fb.draw_line(
                (cx - 3) as u32, (cy - 3) as u32,
                (cx + 3) as u32, (cy + 3) as u32,
                WHITE,
            );
            fb.draw_line(
                (cx + 3) as u32, (cy - 3) as u32,
                (cx - 3) as u32, (cy + 3) as u32,
                WHITE,
            );
        }

        // Draw minimize button
        if win.has_minimize_button {
            let mb_x = win.rect.right() - WINDOW_BORDER as i32 - 40;
            let mb_y = win.rect.y + WINDOW_BORDER as i32 + 4;
            fb.fill_rect(mb_x as u32, mb_y as u32, 16, 16, 0xFF336633);
            // Draw _ (underscore)
            fb.fill_rect(
                mb_x as u32, (mb_y + 12) as u32,
                16, 2,
                WHITE,
            );
        }
    }

    /// Redraw all windows (background + windows in Z-order).
    pub fn redraw_all(&self, fb: &mut Framebuffer) {
        // Draw background
        if self.wallpaper.tile {
            let w = self.wallpaper.width;
            let h = self.wallpaper.height;
            for ty in (0..self.root_height).step_by(h as usize) {
                for tx in (0..self.root_width).step_by(w as usize) {
                    fb.blit(
                        tx, ty,
                        core::cmp::min(w, self.root_width - tx),
                        core::cmp::min(h, self.root_height - ty),
                        &self.wallpaper.data[..core::cmp::min(
                            (self.root_width * self.root_height) as usize,
                            self.wallpaper.data_len,
                        )],
                    );
                }
            }
        } else {
            // Fill background with desktop color
            fb.fill_rect(0, 0, self.root_width, self.root_height, DESKTOP_BG);
        }

        // Draw windows back-to-front (reverse Z-order)
        for i in (0..self.z_count).rev() {
            let idx = self.z_order[i];
            self.redraw_window(fb, idx);
        }
    }

    /// Mark a window as needing redraw.
    pub fn mark_dirty(&mut self, id: usize) {
        if let Some(idx) = self.find_by_id(id) {
            if let Some(ref mut win) = self.windows[idx] {
                win.dirty = true;
            }
        }
    }

    /// Access a window by index.
    pub fn window_by_idx(&self, idx: usize) -> Option<&Window> {
        self.windows[idx].as_ref()
    }

    /// Access a window mutably by index.
    pub fn window_by_idx_mut(&mut self, idx: usize) -> Option<&mut Window> {
        self.windows[idx].as_mut()
    }

    /// Access a window by ID.
    pub fn window_by_id(&self, id: usize) -> Option<&Window> {
        let idx = self.find_by_id(id)?;
        self.windows[idx].as_ref()
    }

    /// Access a window mutably by ID.
    pub fn window_by_id_mut(&mut self, id: usize) -> Option<&mut Window> {
        let idx = self.find_by_id(id)?;
        self.windows[idx].as_mut()
    }

    // ── Accessors for Compositor ─────────────────────────────────────────────

    pub fn bg_color(&self) -> Color {
        self.bg_color
    }

    pub fn z_count(&self) -> usize {
        self.z_count
    }

    pub fn z_order_at(&self, i: usize) -> usize {
        if i < MAX_WINDOWS { self.z_order[i] } else { 0 }
    }

    pub fn find_by_id(&self, id: usize) -> Option<usize> {
        self.windows.iter().position(|w| w.as_ref().map_or(false, |w| w.id == id))
    }
}

// ── Software Compositor ───────────────────────────────────────────────────────
//
// V39a — Z-ordered composition with alpha blending, drop shadows,
//        background blur, and window animations.

/// Maximum tracked dirty rectangles.
const MAX_DIRTY: usize = 64;

/// Software compositor — manages off-screen composition with alpha blending
/// and visual effects.
pub struct Compositor {
    /// Off-screen composition buffer (RGBA32).
    composition_buffer: *mut u32,
    buffer_width: u32,
    buffer_height: u32,
    /// Global alpha for desktop translucency.
    global_alpha: u8,
    /// Visual effect flags.
    enable_shadows: bool,
    enable_animations: bool,
    enable_blur: bool,
    /// Damage tracking.
    dirty_regions: [Rect; MAX_DIRTY],
    dirty_count: usize,
    /// Frame timing.
    last_frame_time: u64,
    target_fps: u32,
    /// Animation state.
    animating: bool,
    anim_window_id: usize,
    anim_frame: u32,
    anim_total_frames: u32,
    anim_type: AnimType,
}

/// Animation type.
enum AnimType {
    None,
    WindowOpen,
    WindowClose,
    Minimize,
}

impl Compositor {
    /// Create a new compositor for the given resolution.
    pub fn new(width: u32, height: u32) -> Self {
        Compositor {
            composition_buffer: core::ptr::null_mut(),
            buffer_width: width,
            buffer_height: height,
            global_alpha: 255,
            enable_shadows: true,
            enable_animations: true,
            enable_blur: false,
            dirty_regions: [Rect::new(0, 0, 0, 0); MAX_DIRTY],
            dirty_count: 0,
            last_frame_time: 0,
            target_fps: 30,
            animating: false,
            anim_window_id: 0,
            anim_frame: 0,
            anim_total_frames: 15,
            anim_type: AnimType::None,
        }
    }

    /// Initialize the composition buffer from the framebuffer pages.
    pub fn init_from_fb(&mut self, fb: &mut Framebuffer) {
        self.composition_buffer = fb.virt_base() as *mut u32;
        self.buffer_width = fb.width();
        self.buffer_height = fb.height();
    }

    /// Allocate a dedicated composition buffer (off-screen).
    pub fn alloc_buffer(&mut self, fb: &Framebuffer) -> bool {
        // Use the framebuffer's back buffer if available, else allocate.
        if !fb.initialized() {
            return false;
        }
        // Reuse framebuffer's virtual address as composition target.
        // For proper off-screen compositing we'd allocate separate pages;
        // for this implementation we composite directly to the framebuffer.
        self.composition_buffer = fb.virt_base() as *mut u32;
        self.buffer_width = fb.width();
        self.buffer_height = fb.height();
        true
    }

    /// Composite all visible windows into the composition buffer.
    /// Returns a reference to the composed buffer.
    pub fn composite(&mut self, wm: &WindowManager) -> &[u32] {
        let total_pixels = (self.buffer_width * self.buffer_height) as usize;

        // Draw background first
        if self.buffer_width > 0 && self.buffer_height > 0 {
            unsafe {
                let buf = core::slice::from_raw_parts_mut(
                    self.composition_buffer, total_pixels,
                );
                // Fill with desktop background color
                for pixel in buf.iter_mut() {
                    *pixel = wm.bg_color();
                }
            }
        }

        // Composite windows back-to-front (bottom of Z-order first)
        for i in (0..wm.z_count()).rev() {
            let idx = wm.z_order_at(i);
            if let Some(win) = wm.window_by_idx(idx) {
                if !win.is_visible || win.is_minimized {
                    continue;
                }
                self.composite_window(win);
            }
        }

        unsafe {
            core::slice::from_raw_parts(self.composition_buffer, total_pixels)
        }
    }

    /// Composite a single window into the composition buffer.
    fn composite_window(&self, win: &Window) {
        let buf = unsafe {
            core::slice::from_raw_parts_mut(
                self.composition_buffer,
                (self.buffer_width * self.buffer_height) as usize,
            )
        };

        let wx = win.rect.x.max(0) as u32;
        let wy = win.rect.y.max(0) as u32;
        let ww = core::cmp::min(win.rect.width, self.buffer_width - wx);
        let wh = core::cmp::min(win.rect.height, self.buffer_height - wy);

        // Apply drop shadow
        if self.enable_shadows {
            let shadow_rect = Rect::new(
                win.rect.x + 3,
                win.rect.y + 3,
                win.rect.width,
                win.rect.height,
            );
            self.render_shadow(&shadow_rect, 4, 40);
        }

        // Draw window background (semi-transparent)
        for row in 0..wh {
            let buf_row = (wy + row) as usize * self.buffer_width as usize;
            for col in 0..ww {
                let buf_idx = buf_row + (wx + col) as usize;
                if buf_idx >= buf.len() { continue; }

                // Determine pixel color based on position within window
                let local_x = col;
                let local_y = row;
                let in_titlebar = local_y < (win.rect.height - win.client_area.height);

                let color = if in_titlebar {
                    win.title_bar_color
                } else {
                    win.fill_color
                };

                // Alpha blend with background
                let sa = self.global_alpha as u32;
                if sa < 255 {
                    let src = color;
                    let dst = buf[buf_idx];
                    let out_a = sa + graphics::alpha(dst) as u32 * (255 - sa) / 255;
                    let out_r = (graphics::red(src) as u32 * sa
                        + graphics::red(dst) as u32 * (255 - sa)) / 255;
                    let out_g = (graphics::green(src) as u32 * sa
                        + graphics::green(dst) as u32 * (255 - sa)) / 255;
                    let out_b = (graphics::blue(src) as u32 * sa
                        + graphics::blue(dst) as u32 * (255 - sa)) / 255;
                    buf[buf_idx] = graphics::rgba(
                        core::cmp::min(out_r, 255) as u8,
                        core::cmp::min(out_g, 255) as u8,
                        core::cmp::min(out_b, 255) as u8,
                        core::cmp::min(out_a, 255) as u8,
                    );
                } else {
                    buf[buf_idx] = color;
                }
            }
        }

        // Render title bar and controls
        // (Title bar is already part of the window fill above,
        // but we re-draw it for proper compositing)

        // Draw title text
        if win.title_len > 0 {
            let title_str = win.title_str();
            let tx = wx + 6;
            let ty = wy + 2;
            // Use bitmap font for title text in composition
            let font = graphics::font_8x16();
            let mut cx = tx;
            for &ch in title_str.as_bytes() {
                let ch = ch as char;
                if cx + 8 > wx + ww { break; }
                let code = ch as usize;
                if code >= 128 { continue; }
                const CHAR_HEIGHT: usize = 16;
                const CHAR_WIDTH: usize = 8;
                let base = code * CHAR_HEIGHT;

                for row in 0..CHAR_HEIGHT {
                    if ty + row as u32 >= self.buffer_height { break; }
                    let row_data = font.get(base + row).copied().unwrap_or(0);
                    for col in 0..CHAR_WIDTH {
                        if cx + col as u32 >= self.buffer_width { break; }
                        if (row_data >> (7 - col)) & 1 != 0 {
                            let idx = ((ty + row as u32) * self.buffer_width + cx + col as u32) as usize;
                            if idx < buf.len() {
                                buf[idx] = graphics::WHITE;
                            }
                        }
                    }
                }
                cx += 8;
            }
        }

        // Draw close button
        if win.has_close_button {
            let cb_x = win.rect.right() - 20;
            let cb_y = win.rect.y + 4;
            for row in 0..16 {
                for col in 0..16 {
                    let bx = (cb_x + col as i32) as u32;
                    let by = (cb_y + row as i32) as u32;
                    if bx < self.buffer_width && by < self.buffer_height {
                        let idx = (by * self.buffer_width + bx) as usize;
                        if idx < buf.len() {
                            buf[idx] = 0xFFCC3333;
                        }
                    }
                }
            }
            // Draw X
            let cx2 = cb_x + 8;
            let cy2 = cb_y + 8;
            for i in 0..7 {
                for d in 0..3 {
                    let xp = (cx2 as i32 - 3 + i as i32) as u32;
                    let yp = (cy2 as i32 - 3 + i as i32) as u32;
                    if xp < self.buffer_width && yp < self.buffer_height {
                        let idx = (yp * self.buffer_width + xp) as usize;
                        if idx < buf.len() { buf[idx] = graphics::WHITE; }
                    }
                    let xp2 = (cx2 as i32 + 3 - i as i32) as u32;
                    let yp2 = (cy2 as i32 - 3 + i as i32) as u32;
                    if xp2 < self.buffer_width && yp2 < self.buffer_height {
                        let idx2 = (yp2 * self.buffer_width + xp2) as usize;
                        if idx2 < buf.len() { buf[idx2] = graphics::WHITE; }
                    }
                }
            }
        }

        // Draw minimize button
        if win.has_minimize_button {
            let mb_x = win.rect.right() - 40;
            let mb_y = win.rect.y + 4;
            for row in 0..16 {
                for col in 0..16 {
                    let bx = (mb_x + col as i32) as u32;
                    let by = (mb_y + row as i32) as u32;
                    if bx < self.buffer_width && by < self.buffer_height {
                        let idx = (by * self.buffer_width + bx) as usize;
                        if idx < buf.len() {
                            buf[idx] = 0xFF336633;
                        }
                    }
                }
            }
            // Draw underscore
            for col in 0..16 {
                let bx = (mb_x + col as i32) as u32;
                let by = (mb_y + 12) as u32;
                if bx < self.buffer_width && by < self.buffer_height {
                    let idx = (by * self.buffer_width + bx) as usize;
                    if idx < buf.len() { buf[idx] = graphics::WHITE; }
                }
            }
        }
    }

    /// Alpha-blend two RGBA pixels (source OVER destination).
    pub fn alpha_blend(src: u32, dst: u32, alpha: u8) -> u32 {
        let sa = alpha as u32;
        if sa >= 255 { return src; }
        if sa == 0 { return dst; }
        let sr = graphics::red(src) as u32;
        let sg = graphics::green(src) as u32;
        let sb = graphics::blue(src) as u32;
        let dr = graphics::red(dst) as u32;
        let dg = graphics::green(dst) as u32;
        let db = graphics::blue(dst) as u32;

        let out_r = (sr * sa + dr * (255 - sa)) / 255;
        let out_g = (sg * sa + dg * (255 - sa)) / 255;
        let out_b = (sb * sa + db * (255 - sa)) / 255;
        graphics::rgba(out_r as u8, out_g as u8, out_b as u8, 255)
    }

    /// Apply a drop shadow behind a window.
    fn render_shadow(&self, rect: &Rect, radius: u32, alpha: u8) {
        let buf = unsafe {
            core::slice::from_raw_parts_mut(
                self.composition_buffer,
                (self.buffer_width * self.buffer_height) as usize,
            )
        };

        let blur = core::cmp::min(radius, 16u32);
        for layer in 0..blur {
            let a = (alpha as u32) * (blur - layer) / blur;
            let blur_color = graphics::rgba(0, 0, 0, a as u8);
            let inset = layer as i32;

            let sx = (rect.x + inset).max(0) as u32;
            let sy = (rect.y + inset).max(0) as u32;
            let sw = rect.width.wrapping_sub((2 * inset) as u32);
            let sh = rect.height.wrapping_sub((2 * inset) as u32);

            if sw == 0 || sh == 0 { continue; }

            for row in sy..core::cmp::min(sy + sh, self.buffer_height) {
                for col in sx..core::cmp::min(sx + sw, self.buffer_width) {
                    let idx = (row * self.buffer_width + col) as usize;
                    if idx >= buf.len() { continue; }
                    let dst = buf[idx];
                    let blended = Self::alpha_blend(blur_color, dst, a as u8);
                    buf[idx] = blended;
                }
            }
        }
    }

    /// Apply background blur behind a window (simple box blur approximation).
    fn render_blur(&self, rect: &Rect, radius: u32) {
        let buf = unsafe {
            core::slice::from_raw_parts_mut(
                self.composition_buffer,
                (self.buffer_width * self.buffer_height) as usize,
            )
        };

        let r = core::cmp::min(radius, 8u32) as usize;
        let bx = rect.x.max(0) as usize;
        let by = rect.y.max(0) as usize;
        let bw = core::cmp::min(rect.width as usize, self.buffer_width as usize - bx);
        let bh = core::cmp::min(rect.height as usize, self.buffer_height as usize - by);

        if bw < 2 || bh < 2 { return; }

        // Simple box blur: average of surrounding pixels
        let mut temp = [0u32; 1024]; // Temporary row buffer
        for row in 0..bh {
            let y = by + row;
            for col in 0..bw {
                let x = bx + col;
                let mut sum_r = 0u32;
                let mut sum_g = 0u32;
                let mut sum_b = 0u32;
                let mut count = 0u32;

                let ky_min = if row > r { row - r } else { 0 };
                let ky_max = core::cmp::min(row + r, bh - 1);
                let kx_min = if col > r { col - r } else { 0 };
                let kx_max = core::cmp::min(col + r, bw - 1);

                for ky in ky_min..=ky_max {
                    for kx in kx_min..=kx_max {
                        let idx = (by + ky) * self.buffer_width as usize + (bx + kx);
                        if idx < buf.len() {
                            let p = buf[idx];
                            sum_r += graphics::red(p) as u32;
                            sum_g += graphics::green(p) as u32;
                            sum_b += graphics::blue(p) as u32;
                            count += 1;
                        }
                    }
                }

                if count > 0 {
                    temp[col] = graphics::rgba(
                        (sum_r / count) as u8,
                        (sum_g / count) as u8,
                        (sum_b / count) as u8,
                        255,
                    );
                } else {
                    temp[col] = buf[y * self.buffer_width as usize + x];
                }
            }

            for col in 0..bw {
                let idx = y * self.buffer_width as usize + (bx + col);
                if idx < buf.len() {
                    buf[idx] = temp[col];
                }
            }
        }
    }

    /// Fade-in + scale-up animation for window opening.
    pub fn animate_window_open(&mut self, window_id: usize) {
        if !self.enable_animations { return; }
        self.animating = true;
        self.anim_window_id = window_id;
        self.anim_frame = 0;
        self.anim_total_frames = 15;
        self.anim_type = AnimType::WindowOpen;
    }

    /// Fade-out animation for window closing.
    pub fn animate_window_close(&mut self, window_id: usize) {
        if !self.enable_animations { return; }
        self.animating = true;
        self.anim_window_id = window_id;
        self.anim_frame = 0;
        self.anim_total_frames = 10;
        self.anim_type = AnimType::WindowClose;
    }

    /// Slide-to-taskbar animation for minimize.
    pub fn animate_minimize(&mut self, window_id: usize, _taskbar_y: u32) {
        if !self.enable_animations { return; }
        self.animating = true;
        self.anim_window_id = window_id;
        self.anim_frame = 0;
        self.anim_total_frames = 12;
        self.anim_type = AnimType::Minimize;
    }

    /// Advance the current animation by one frame.
    /// Returns true if animation is still running.
    pub fn tick_animation(&mut self, wm: &mut WindowManager) -> bool {
        if !self.animating { return false; }

        self.anim_frame += 1;
        if self.anim_frame >= self.anim_total_frames {
            self.animating = false;
            self.anim_type = AnimType::None;
            return false;
        }

        let progress = self.anim_frame as f32 / self.anim_total_frames as f32;

        match self.anim_type {
            AnimType::WindowOpen => {
                // Scale from 0.5 to 1.0 and fade in
                let scale = 0.5 + 0.5 * progress;
                let alpha = (progress * 255.0) as u8;
                if let Some(idx) = wm.find_by_id(self.anim_window_id) {
                    if let Some(ref mut win) = wm.window_by_idx_mut(idx) {
                        let orig_w = win.rect.width;
                        let orig_h = win.rect.height;
                        let scaled_w = (orig_w as f32 * scale) as u32;
                        let scaled_h = (orig_h as f32 * scale) as u32;
                        win.rect.width = scaled_w;
                        win.rect.height = scaled_h;
                        // Center the scaled window
                        win.rect.x += (orig_w - scaled_w) as i32 / 2;
                        win.rect.y += (orig_h - scaled_h) as i32 / 2;
                        win.update_client_area();
                        win.dirty = true;
                    }
                }
                self.global_alpha = alpha;
            }
            AnimType::WindowClose => {
                // Fade out
                let alpha = ((1.0 - progress) * 255.0) as u8;
                self.global_alpha = alpha;
            }
            AnimType::Minimize => {
                // Shrink toward taskbar
                let scale = 1.0 - progress * 0.7;
                if let Some(idx) = wm.find_by_id(self.anim_window_id) {
                    if let Some(ref mut win) = wm.window_by_idx_mut(idx) {
                        let orig_w = win.rect.width;
                        let orig_h = win.rect.height;
                        win.rect.width = (orig_w as f32 * scale) as u32;
                        win.rect.height = (orig_h as f32 * scale) as u32;
                        win.rect.y += (orig_h - win.rect.height) as i32;
                        win.update_client_area();
                        win.dirty = true;
                    }
                }
            }
            AnimType::None => {}
        }
        true
    }

    /// Flush only the damaged regions from the composition buffer to the
    /// hardware framebuffer.
    pub fn flush_to_fb(&mut self, fb: &mut Framebuffer) {
        if self.composition_buffer.is_null() {
            return;
        }
        let total = (self.buffer_width * self.buffer_height) as usize;
        let comp_buf = unsafe {
            core::slice::from_raw_parts(self.composition_buffer, total)
        };

        // If we have dirty regions, only copy those; else full update.
        if self.dirty_count > 0 {
            for i in 0..self.dirty_count {
                let r = &self.dirty_regions[i];
                if r.width == 0 || r.height == 0 { continue; }
                let x = r.x.max(0) as u32;
                let y = r.y.max(0) as u32;
                let w = core::cmp::min(r.width, self.buffer_width - x);
                let h = core::cmp::min(r.height, self.buffer_height - y);
                if w == 0 || h == 0 { continue; }

                for row in 0..h {
                    let src_row = (y + row) as usize;
                    let dst_row = (y + row) as usize;
                    let src_off = src_row * self.buffer_width as usize + x as usize;
                    let dst_off = dst_row * fb.stride() as usize / 4 + x as usize;
                    for col in 0..w {
                        if (src_off + col as usize) < total {
                            let pixel = comp_buf[src_off + col as usize];
                            unsafe {
                                core::ptr::write_volatile(
                                    (fb.virt_base() as *mut u32).add(dst_off + col as usize),
                                    pixel,
                                );
                            }
                        }
                    }
                }
            }
        } else {
            // Full update
            for i in 0..total {
                unsafe {
                    core::ptr::write_volatile(
                        (fb.virt_base() as *mut u32).add(i),
                        comp_buf[i],
                    );
                }
            }
        }

        self.dirty_count = 0;
    }

    /// Set the target FPS.
    pub fn set_fps(&mut self, fps: u32) {
        self.target_fps = core::cmp::max(1, core::cmp::min(fps, 120));
    }

    /// Enable or disable drop shadows.
    pub fn set_shadows(&mut self, enabled: bool) {
        self.enable_shadows = enabled;
    }

    /// Enable or disable animations.
    pub fn set_animations(&mut self, enabled: bool) {
        self.enable_animations = enabled;
    }

    /// Enable or disable background blur.
    pub fn set_blur(&mut self, enabled: bool) {
        self.enable_blur = enabled;
    }

    /// Set global alpha for desktop translucency.
    pub fn set_global_alpha(&mut self, alpha: u8) {
        self.global_alpha = alpha;
    }

    /// Get the frame time in milliseconds since last composite.
    pub fn frame_time_ms(&self) -> u64 {
        // In a real system this would read a timer CSR.
        // For now, returns a rough estimate based on target FPS.
        if self.target_fps > 0 {
            1000 / self.target_fps as u64
        } else {
            33
        }
    }

    /// Mark a region as dirty.
    pub fn mark_dirty(&mut self, x: u32, y: u32, w: u32, h: u32) {
        if self.dirty_count < MAX_DIRTY {
            self.dirty_regions[self.dirty_count] = Rect::new(x as i32, y as i32, w, h);
            self.dirty_count += 1;
        }
    }

    /// Check if compositor has a buffer allocated.
    pub fn has_buffer(&self) -> bool {
        !self.composition_buffer.is_null()
    }
}
