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

    /// Find a window index by its ID.
    fn find_by_id(&self, id: usize) -> Option<usize> {
        self.windows.iter().position(|w| w.as_ref().map_or(false, |w| w.id == id))
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
}
