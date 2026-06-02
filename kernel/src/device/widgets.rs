// V37b — GUI Widget Toolkit
//
// Provides basic widget types and rendering functions for
// building graphical user interfaces.

use super::framebuffer::Framebuffer;
use super::graphics::{
    self, draw_border, draw_text_centered, draw_text_wrapped,
    font_8x16, Color, DARK_GRAY, GRAY, LIGHT_GRAY, WHITE,
    Rect,
};

// ── Text Alignment ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

// ── Widget Types ───────────────────────────────────────────────────────────

/// A clickable button widget.
pub struct ButtonWidget {
    pub rect: Rect,
    pub text: [u8; 32],
    pub text_len: usize,
    pub is_pressed: bool,
    pub is_hovered: bool,
    pub color: Color,
    pub hover_color: Color,
    pub text_color: Color,
    pub border_radius: u32,
}

impl ButtonWidget {
    pub fn new(x: i32, y: i32, w: u32, h: u32, text: &str) -> Self {
        let mut text_buf = [0u8; 32];
        let tlen = core::cmp::min(text.len(), 32);
        for (i, b) in text.bytes().enumerate().take(tlen) {
            text_buf[i] = b;
        }
        ButtonWidget {
            rect: Rect::new(x, y, w, h),
            text: text_buf,
            text_len: tlen,
            is_pressed: false,
            is_hovered: false,
            color: 0xFF4A90D9,
            hover_color: 0xFF5BA0E9,
            text_color: WHITE,
            border_radius: 4,
        }
    }

    pub fn text_str(&self) -> &str {
        core::str::from_utf8(&self.text[..self.text_len]).unwrap_or("")
    }
}

/// A static label widget.
pub struct LabelWidget {
    pub rect: Rect,
    pub text: [u8; 128],
    pub text_len: usize,
    pub color: Color,
    pub alignment: TextAlign,
}

impl LabelWidget {
    pub fn new(x: i32, y: i32, text: &str) -> Self {
        let mut text_buf = [0u8; 128];
        let tlen = core::cmp::min(text.len(), 128);
        for (i, b) in text.bytes().enumerate().take(tlen) {
            text_buf[i] = b;
        }
        LabelWidget {
            rect: Rect::new(x, y, (tlen as u32) * 8, 16),
            text: text_buf,
            text_len: tlen,
            color: 0xFF000000,
            alignment: TextAlign::Left,
        }
    }

    pub fn text_str(&self) -> &str {
        core::str::from_utf8(&self.text[..self.text_len]).unwrap_or("")
    }
}

/// A text input box widget.
pub struct TextBoxWidget {
    pub rect: Rect,
    pub buffer: [u8; 512],
    pub buffer_len: usize,
    pub cursor_pos: usize,
    pub is_focused: bool,
    pub scroll_offset: usize,
    pub placeholder: [u8; 32],
    pub placeholder_len: usize,
    pub border_color: Color,
    pub bg_color: Color,
}

impl TextBoxWidget {
    pub fn new(x: i32, y: i32, w: u32, placeholder: &str) -> Self {
        let mut placeholder_buf = [0u8; 32];
        let plen = core::cmp::min(placeholder.len(), 32);
        for (i, b) in placeholder.bytes().enumerate().take(plen) {
            placeholder_buf[i] = b;
        }
        TextBoxWidget {
            rect: Rect::new(x, y, w, 24),
            buffer: [0u8; 512],
            buffer_len: 0,
            cursor_pos: 0,
            is_focused: false,
            scroll_offset: 0,
            placeholder: placeholder_buf,
            placeholder_len: plen,
            border_color: 0xFF888888,
            bg_color: 0xFFFFFFFF,
        }
    }

    pub fn insert_char(&mut self, ch: u8) {
        if self.buffer_len < 512 {
            // Shift existing text right
            for i in (self.cursor_pos..self.buffer_len).rev() {
                self.buffer[i + 1] = self.buffer[i];
            }
            self.buffer[self.cursor_pos] = ch;
            self.buffer_len += 1;
            self.cursor_pos += 1;
        }
    }

    pub fn delete_before(&mut self) {
        if self.cursor_pos > 0 && self.buffer_len > 0 {
            for i in self.cursor_pos..self.buffer_len {
                self.buffer[i - 1] = self.buffer[i];
            }
            self.buffer_len -= 1;
            self.cursor_pos -= 1;
        }
    }

    pub fn delete_after(&mut self) {
        if self.cursor_pos < self.buffer_len {
            for i in self.cursor_pos..self.buffer_len {
                self.buffer[i] = self.buffer[i + 1];
            }
            self.buffer_len -= 1;
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.buffer_len {
            self.cursor_pos += 1;
        }
    }

    pub fn content_str(&self) -> &str {
        core::str::from_utf8(&self.buffer[..self.buffer_len]).unwrap_or("")
    }
}

/// A checkbox widget.
pub struct CheckBoxWidget {
    pub rect: Rect,
    pub label: [u8; 64],
    pub label_len: usize,
    pub is_checked: bool,
    pub is_enabled: bool,
}

impl CheckBoxWidget {
    pub fn new(x: i32, y: i32, label: &str) -> Self {
        let mut label_buf = [0u8; 64];
        let llen = core::cmp::min(label.len(), 64);
        for (i, b) in label.bytes().enumerate().take(llen) {
            label_buf[i] = b;
        }
        CheckBoxWidget {
            rect: Rect::new(x, y, (llen as u32) * 8 + 24, 20),
            label: label_buf,
            label_len: llen,
            is_checked: false,
            is_enabled: true,
        }
    }

    pub fn label_str(&self) -> &str {
        core::str::from_utf8(&self.label[..self.label_len]).unwrap_or("")
    }
}

/// A progress bar widget.
pub struct ProgressWidget {
    pub rect: Rect,
    pub value: f32,
    pub color: Color,
    pub track_color: Color,
    pub show_percentage: bool,
    pub indeterminate: bool,
    /// Animation phase for indeterminate mode (0-255).
    pub anim_phase: u8,
}

impl ProgressWidget {
    pub fn new(x: i32, y: i32, w: u32) -> Self {
        ProgressWidget {
            rect: Rect::new(x, y, w, 20),
            value: 0.0,
            color: 0xFF4A90D9,
            track_color: 0xFFE0E0E0,
            show_percentage: true,
            indeterminate: false,
            anim_phase: 0,
        }
    }
}

/// A scrollbar widget.
pub struct ScrollBarWidget {
    pub rect: Rect,
    pub value: f32,
    pub max_value: f32,
    pub page_size: f32,
    pub is_vertical: bool,
    pub is_dragging: bool,
}

impl ScrollBarWidget {
    pub fn new(x: i32, y: i32, w: u32, h: u32, vertical: bool) -> Self {
        ScrollBarWidget {
            rect: Rect::new(x, y, w, h),
            value: 0.0,
            max_value: 100.0,
            page_size: 10.0,
            is_vertical: vertical,
            is_dragging: false,
        }
    }

    /// The fraction of total range visible.
    pub fn visible_ratio(&self) -> f32 {
        if self.max_value <= 0.0 {
            1.0
        } else {
            (self.page_size / (self.max_value + self.page_size)).min(1.0)
        }
    }

    /// The thumb position as a fraction (0.0 - 1.0).
    pub fn thumb_pos(&self) -> f32 {
        if self.max_value <= 0.0 {
            0.0
        } else {
            (self.value / (self.max_value + self.page_size)).min(1.0)
        }
    }
}

// ── Widget Enum ────────────────────────────────────────────────────────────

/// A generic widget that can be one of several concrete types.
pub enum Widget {
    Button(ButtonWidget),
    Label(LabelWidget),
    TextBox(TextBoxWidget),
    CheckBox(CheckBoxWidget),
    ProgressBar(ProgressWidget),
    ScrollBar(ScrollBarWidget),
    DropdownMenu(DropdownMenu),
    Slider(Slider),
    TabBar(TabBar),
    ListView(ListView),
    TreeView(TreeView),
    Dialog(Dialog),
    Tooltip(Tooltip),
    StatusBar(StatusBar),
    Container(Container),
}

// ── Widget Rendering ───────────────────────────────────────────────────────

/// Draw a widget to the framebuffer.
pub fn draw_widget(fb: &mut Framebuffer, widget: &Widget) {
    match widget {
        Widget::Button(b) => draw_button(fb, b),
        Widget::Label(l) => draw_label(fb, l),
        Widget::TextBox(t) => draw_textbox(fb, t),
        Widget::CheckBox(c) => draw_checkbox(fb, c),
        Widget::ProgressBar(p) => draw_progress(fb, p),
        Widget::ScrollBar(s) => draw_scrollbar(fb, s),
        Widget::DropdownMenu(d) => draw_dropdown_menu(fb, d),
        Widget::Slider(s) => draw_slider(fb, s),
        Widget::TabBar(t) => draw_tab_bar(fb, t),
        Widget::ListView(l) => draw_list_view(fb, l),
        Widget::TreeView(t) => draw_tree_view(fb, t),
        Widget::Dialog(d) => draw_dialog(fb, d),
        Widget::Tooltip(t) => draw_tooltip(fb, t),
        Widget::StatusBar(s) => draw_status_bar(fb, s),
        Widget::Container(c) => draw_container(fb, c),
    }
}

fn draw_button(fb: &mut Framebuffer, btn: &ButtonWidget) {
    let face_color = if btn.is_pressed {
        graphics::darken(btn.color, 0.8)
    } else if btn.is_hovered {
        btn.hover_color
    } else {
        btn.color
    };

    // Button background
    if btn.border_radius > 0 {
        fb.fill_rect(
            btn.rect.x as u32, btn.rect.y as u32,
            btn.rect.width, btn.rect.height,
            face_color,
        );
    } else {
        fb.fill_rect(
            btn.rect.x as u32, btn.rect.y as u32,
            btn.rect.width, btn.rect.height,
            face_color,
        );
    }

    // Button border
    let border_color = if btn.is_pressed { graphics::darken(btn.color, 0.6) } else { graphics::darken(btn.color, 0.7) };
    draw_border(fb, &btn.rect, 1, border_color);

    // Button text
    draw_text_centered(fb, &btn.rect, btn.text_str(), btn.text_color, face_color);
}

fn draw_label(fb: &mut Framebuffer, label: &LabelWidget) {
    let text = label.text_str();
    let font = font_8x16();

    let x = match label.alignment {
        TextAlign::Left => label.rect.x,
        TextAlign::Center => label.rect.x + (label.rect.width as i32 - (text.len() as u32 * 8) as i32) / 2,
        TextAlign::Right => label.rect.right() - (text.len() as u32 * 8) as i32,
    };

    draw_text_wrapped(
        fb,
        x.max(0) as u32,
        label.rect.y.max(0) as u32,
        label.rect.width,
        text,
        label.color,
        0x00000000,
    );
}

fn draw_textbox(fb: &mut Framebuffer, tb: &TextBoxWidget) {
    // Background
    fb.fill_rect(
        tb.rect.x as u32, tb.rect.y as u32,
        tb.rect.width, tb.rect.height,
        tb.bg_color,
    );

    // Border
    let border = if tb.is_focused { 0xFF4A90D9 } else { tb.border_color };
    draw_border(fb, &tb.rect, 1, border);

    // Text content
    let font = font_8x16();
    let text_x = tb.rect.x as u32 + 2;
    let text_y = tb.rect.y as u32 + 4;

    if tb.buffer_len > 0 {
        let content = tb.content_str();
        draw_text_wrapped(fb, text_x, text_y, tb.rect.width - 4, content, 0xFF000000, tb.bg_color);
    } else if !tb.is_focused {
        // Placeholder text
        let placeholder = core::str::from_utf8(&tb.placeholder[..tb.placeholder_len]).unwrap_or("");
        draw_text_wrapped(fb, text_x, text_y, tb.rect.width - 4, placeholder, GRAY, tb.bg_color);
    }

    // Cursor (if focused)
    if tb.is_focused {
        let cursor_x = text_x + (tb.cursor_pos as u32) * 8 - (tb.scroll_offset as u32) * 8;
        fb.fill_rect(cursor_x, text_y, 1, 16, 0xFF000000);
    }
}

fn draw_checkbox(fb: &mut Framebuffer, cb: &CheckBoxWidget) {
    // Checkbox square
    let box_x = cb.rect.x;
    let box_y = cb.rect.y + 2;
    let box_rect = Rect::new(box_x, box_y, 14, 14);

    fb.fill_rect(box_x as u32, box_y as u32, 14, 14, WHITE);
    draw_border(fb, &box_rect, 1, DARK_GRAY);

    // Checkmark
    if cb.is_checked {
        fb.draw_line(
            (box_x + 2) as u32, (box_y + 7) as u32,
            (box_x + 6) as u32, (box_y + 11) as u32,
            0xFF4A90D9,
        );
        fb.draw_line(
            (box_x + 6) as u32, (box_y + 11) as u32,
            (box_x + 12) as u32, (box_y + 3) as u32,
            0xFF4A90D9,
        );
    }

    // Label
    let label_x = box_x + 18;
    let font = font_8x16();
    draw_text_wrapped(
        fb, label_x as u32, cb.rect.y as u32,
        200,
        cb.label_str(),
        0xFF000000,
        0x00000000,
    );
}

fn draw_progress(fb: &mut Framebuffer, pb: &ProgressWidget) {
    // Track
    fb.fill_rect(
        pb.rect.x as u32, pb.rect.y as u32,
        pb.rect.width, pb.rect.height,
        pb.track_color,
    );
    draw_border(fb, &pb.rect, 1, DARK_GRAY);

    if pb.indeterminate {
        // Animated bouncing bar
        let bar_w = (pb.rect.width / 3).max(20);
        let phase = pb.anim_phase as usize;
        let max_x = pb.rect.width as usize - bar_w as usize;
        let offset = if max_x > 0 {
            let t = (phase % (2 * max_x)) as usize;
            if t < max_x { t } else { 2 * max_x - t }
        } else {
            0
        };
        fb.fill_rect(
            (pb.rect.x + offset as i32) as u32,
            pb.rect.y as u32,
            bar_w,
            pb.rect.height,
            pb.color,
        );
    } else {
        // Determinate
        let fill_w = ((pb.value.max(0.0).min(1.0)) * pb.rect.width as f32) as u32;
        if fill_w > 0 {
            fb.fill_rect(
                pb.rect.x as u32, pb.rect.y as u32,
                fill_w, pb.rect.height,
                pb.color,
            );
        }
    }

    // Percentage text
    if pb.show_percentage {
        let pct = (pb.value.max(0.0).min(1.0) * 100.0) as u32;
        let mut pct_str = [0u8; 8];
        let len;
        if pct >= 100 {
        let s = b"100%";
        let clen = core::cmp::min(s.len(), pct_str.len());
        for i in 0..clen { pct_str[i] = s[i]; }
        len = 4;
        } else if pct >= 10 {
            pct_str[0] = b'0' + (pct / 10) as u8;
            pct_str[1] = b'0' + (pct % 10) as u8;
            pct_str[2] = b'%';
            len = 3;
        } else {
            pct_str[0] = b'0' + pct as u8;
            pct_str[1] = b'%';
            len = 2;
        }
        let text = core::str::from_utf8(&pct_str[..len]).unwrap_or("0%");
        let rect = Rect::new(pb.rect.x, pb.rect.y, pb.rect.width, pb.rect.height);
        draw_text_centered(fb, &rect, text, WHITE, graphics::TRANSPARENT);
    }
}

fn draw_scrollbar(fb: &mut Framebuffer, sb: &ScrollBarWidget) {
    // Track
    fb.fill_rect(
        sb.rect.x as u32, sb.rect.y as u32,
        sb.rect.width, sb.rect.height,
        0xFFE8E8E8,
    );

    // Thumb
    if sb.is_vertical {
        let track_h = sb.rect.height as f32;
        let thumb_h = (sb.visible_ratio() * track_h).max(16.0);
        let thumb_y = sb.thumb_pos() * (track_h - thumb_h);
        fb.fill_rect(
            (sb.rect.x + 2) as u32,
            (sb.rect.y as f32 + thumb_y) as u32,
            sb.rect.width - 4,
            thumb_h as u32,
            0xFFA0A0A0,
        );
    } else {
        let track_w = sb.rect.width as f32;
        let thumb_w = (sb.visible_ratio() * track_w).max(16.0);
        let thumb_x = sb.thumb_pos() * (track_w - thumb_w);
        fb.fill_rect(
            (sb.rect.x as f32 + thumb_x) as u32,
            (sb.rect.y + 2) as u32,
            thumb_w as u32,
            sb.rect.height - 4,
            0xFFA0A0A0,
        );
    }
}

// ── Hit Testing ────────────────────────────────────────────────────────────

/// Test if a point is within a widget.
pub fn widget_hit_test(widget: &Widget, x: i32, y: i32) -> bool {
    let rect = match widget {
        Widget::Button(b) => &b.rect,
        Widget::Label(l) => &l.rect,
        Widget::TextBox(t) => &t.rect,
        Widget::CheckBox(c) => &c.rect,
        Widget::ProgressBar(p) => &p.rect,
        Widget::ScrollBar(s) => &s.rect,
        Widget::DropdownMenu(d) => {
            if d.is_open {
                // Check expanded dropdown area too
                let expand_h = core::cmp::min(d.item_count, d.max_visible) as u32 * 20;
                let expand_rect = Rect::new(d.rect.x, d.rect.bottom(), d.rect.width, expand_h);
                if expand_rect.contains(&graphics::Point::new(x, y)) {
                    return true;
                }
            }
            &d.rect
        }
        Widget::Slider(s) => &s.rect,
        Widget::TabBar(t) => &t.rect,
        Widget::ListView(l) => &l.rect,
        Widget::TreeView(t) => &t.rect,
        Widget::Dialog(d) => &d.rect,
        Widget::Tooltip(t) => &t.rect,
        Widget::StatusBar(s) => &s.rect,
        Widget::Container(c) => &c.rect,
    };
    rect.contains(&graphics::Point::new(x, y))
}

// ═══════════════════════════════════════════════════════════════════════════════
// V39b — Enhanced Widget Toolkit 2.0
// ═══════════════════════════════════════════════════════════════════════════════

/// Icon type enumeration for list/tree items.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum IconType {
    None,
    Folder,
    File,
    Image,
    Text,
    Archive,
    Executable,
    Home,
    Desktop,
    Documents,
    Downloads,
    Music,
    Pictures,
    Videos,
    Trash,
    Network,
    Drive,
}

// ── Dropdown Item ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct DropdownItem {
    pub label: [u8; 48],
    pub label_len: usize,
    pub enabled: bool,
    pub shortcut: [u8; 8],
    pub shortcut_len: usize,
    pub checked: bool,
    pub separator: bool,
}

impl DropdownItem {
    pub fn new(label: &str) -> Self {
        let mut label_buf = [0u8; 48];
        let llen = core::cmp::min(label.len(), 48);
        for (i, b) in label.bytes().enumerate().take(llen) {
            label_buf[i] = b;
        }
        DropdownItem {
            label: label_buf,
            label_len: llen,
            enabled: true,
            shortcut: [0u8; 8],
            shortcut_len: 0,
            checked: false,
            separator: false,
        }
    }

    pub fn label_str(&self) -> &str {
        core::str::from_utf8(&self.label[..self.label_len]).unwrap_or("")
    }
}

// ── Dropdown Menu ─────────────────────────────────────────────────────────────

pub struct DropdownMenu {
    pub rect: Rect,
    pub items: [DropdownItem; 16],
    pub item_count: usize,
    pub is_open: bool,
    pub selected_index: usize,
    pub max_visible: usize,
    pub scroll_offset: usize,
    pub bg_color: Color,
    pub border_color: Color,
    pub text_color: Color,
    pub highlight_color: Color,
}

impl DropdownMenu {
    pub fn new(x: i32, y: i32, w: u32) -> Self {
        DropdownMenu {
            rect: Rect::new(x, y, w, 24),
            items: [DropdownItem::new(""); 16],
            item_count: 0,
            is_open: false,
            selected_index: 0,
            max_visible: 8,
            scroll_offset: 0,
            bg_color: 0xFFFFFFFF,
            border_color: 0xFF888888,
            text_color: 0xFF000000,
            highlight_color: 0xFF4A90D9,
        }
    }

    pub fn add_item(&mut self, item: DropdownItem) -> bool {
        if self.item_count < 16 {
            self.items[self.item_count] = item;
            self.item_count += 1;
            true
        } else {
            false
        }
    }

    pub fn selected_text(&self) -> &str {
        if self.selected_index < self.item_count {
            self.items[self.selected_index].label_str()
        } else {
            ""
        }
    }

    /// Find which dropdown item is at a given y coordinate (when open).
    pub fn item_at_y(&self, y: i32) -> Option<usize> {
        if !self.is_open { return None; }
        let list_y = self.rect.bottom();
        let item_h = 20i32;
        let visible = core::cmp::min(self.item_count, self.max_visible);
        for i in 0..visible {
            let iy = list_y + (i as i32) * item_h;
            if y >= iy && y < iy + item_h {
                return Some(self.scroll_offset + i);
            }
        }
        None
    }
}

// ── Slider ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderOrientation {
    Horizontal,
    Vertical,
}

pub struct Slider {
    pub rect: Rect,
    pub value: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub is_dragging: bool,
    pub track_color: Color,
    pub fill_color: Color,
    pub thumb_color: Color,
    pub thumb_radius: u32,
    pub show_value: bool,
    pub orientation: SliderOrientation,
}

impl Slider {
    pub fn new(x: i32, y: i32, w: u32, h: u32, orientation: SliderOrientation) -> Self {
        Slider {
            rect: Rect::new(x, y, w, h),
            value: 0.5,
            min: 0.0,
            max: 1.0,
            step: 0.0,
            is_dragging: false,
            track_color: 0xFFE0E0E0,
            fill_color: 0xFF4A90D9,
            thumb_color: 0xFF5090E0,
            thumb_radius: 8,
            show_value: true,
            orientation,
        }
    }

    pub fn set_value(&mut self, val: f32) {
        let clamped = val.max(self.min).min(self.max);
        if self.step > 0.0 {
            // Manual rounding (round-to-nearest, avoid f32 methods not on target)
            let ratio = clamped / self.step;
            let rounded = (ratio + 0.5) as i32 as f32;
            self.value = rounded * self.step;
        } else {
            self.value = clamped;
        }
    }

    /// Convert a pixel x coordinate to a slider value.
    pub fn pixel_to_value(&self, px: i32) -> f32 {
        let track_w = self.rect.width as i32 - 2 * self.thumb_radius as i32;
        if track_w <= 0 { return self.min; }
        let rel = (px - self.rect.x - self.thumb_radius as i32).max(0).min(track_w);
        let t = rel as f32 / track_w as f32;
        self.min + t * (self.max - self.min)
    }
}

// ── Tab ──────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct Tab {
    pub label: [u8; 32],
    pub label_len: usize,
    pub content_rect: Rect,
    pub closable: bool,
}

impl Tab {
    pub fn new(label: &str) -> Self {
        let mut label_buf = [0u8; 32];
        let llen = core::cmp::min(label.len(), 32);
        for (i, b) in label.bytes().enumerate().take(llen) {
            label_buf[i] = b;
        }
        Tab {
            label: label_buf,
            label_len: llen,
            content_rect: Rect::new(0, 0, 0, 0),
            closable: true,
        }
    }

    pub fn label_str(&self) -> &str {
        core::str::from_utf8(&self.label[..self.label_len]).unwrap_or("")
    }
}

// ── Tab Bar ──────────────────────────────────────────────────────────────────

pub struct TabBar {
    pub rect: Rect,
    pub tabs: [Tab; 8],
    pub tab_count: usize,
    pub active_tab: usize,
    pub tab_height: u32,
    pub bg_color: Color,
    pub active_color: Color,
    pub inactive_color: Color,
    pub text_color: Color,
}

impl TabBar {
    pub fn new(x: i32, y: i32, w: u32) -> Self {
        TabBar {
            rect: Rect::new(x, y, w, 28),
            tabs: [Tab::new(""); 8],
            tab_count: 0,
            active_tab: 0,
            tab_height: 28,
            bg_color: 0xFFE8E8E8,
            active_color: 0xFFFFFFFF,
            inactive_color: 0xFFD0D0D0,
            text_color: 0xFF000000,
        }
    }

    pub fn add_tab(&mut self, label: &str) -> bool {
        if self.tab_count < 8 {
            self.tabs[self.tab_count] = Tab::new(label);
            self.tab_count += 1;
            true
        } else {
            false
        }
    }

    /// Tab width computed from available space.
    pub fn tab_width(&self) -> u32 {
        if self.tab_count == 0 { return 0; }
        self.rect.width / self.tab_count as u32
    }

    /// Which tab index is at a given x coordinate.
    pub fn tab_at_x(&self, x: i32) -> Option<usize> {
        let tw = self.tab_width() as i32;
        if tw <= 0 { return None; }
        let rel = x - self.rect.x;
        if rel < 0 { return None; }
        let idx = (rel / tw) as usize;
        if idx < self.tab_count { Some(idx) } else { None }
    }
}

// ── List View Item ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct ListViewItem {
    pub text: [u8; 64],
    pub text_len: usize,
    pub icon_type: IconType,
    pub data: usize,
    pub selected: bool,
}

impl ListViewItem {
    pub fn new(text: &str) -> Self {
        let mut text_buf = [0u8; 64];
        let tlen = core::cmp::min(text.len(), 64);
        for (i, b) in text.bytes().enumerate().take(tlen) {
            text_buf[i] = b;
        }
        ListViewItem {
            text: text_buf,
            text_len: tlen,
            icon_type: IconType::None,
            data: 0,
            selected: false,
        }
    }

    pub fn text_str(&self) -> &str {
        core::str::from_utf8(&self.text[..self.text_len]).unwrap_or("")
    }
}

// ── List View ────────────────────────────────────────────────────────────────

pub struct ListView {
    pub rect: Rect,
    pub items: [ListViewItem; 64],
    pub item_count: usize,
    pub selected_index: isize,
    pub scroll_offset: usize,
    pub visible_count: usize,
    pub item_height: u32,
    pub multi_select: bool,
    pub selected_indices: [isize; 16],
    pub selection_count: usize,
    pub sort_column: isize,
    pub sort_ascending: bool,
    pub bg_color: Color,
    pub text_color: Color,
    pub selection_color: Color,
    pub alt_row_color: Color,
    pub border_color: Color,
}

impl ListView {
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        let item_h = 20u32;
        let vis = if item_h > 0 { h / item_h } else { 1 };
        ListView {
            rect: Rect::new(x, y, w, h),
            items: [ListViewItem::new(""); 64],
            item_count: 0,
            selected_index: -1,
            scroll_offset: 0,
            visible_count: vis as usize,
            item_height: item_h,
            multi_select: false,
            selected_indices: [-1; 16],
            selection_count: 0,
            sort_column: -1,
            sort_ascending: true,
            bg_color: 0xFFFFFFFF,
            text_color: 0xFF000000,
            selection_color: 0xFF4A90D9,
            alt_row_color: 0xFFF5F5F5,
            border_color: 0xFFCCCCCC,
        }
    }

    pub fn add_item(&mut self, text: &str) -> bool {
        if self.item_count < 64 {
            self.items[self.item_count] = ListViewItem::new(text);
            self.item_count += 1;
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.item_count = 0;
        self.selected_index = -1;
        self.selection_count = 0;
        self.scroll_offset = 0;
    }

    /// Which item index is at a given y coordinate.
    pub fn item_at_y(&self, y: i32) -> Option<usize> {
        let rel = y - self.rect.y;
        if rel < 0 { return None; }
        let idx = (rel as usize) / self.item_height as usize;
        let global_idx = self.scroll_offset + idx;
        if global_idx < self.item_count { Some(global_idx) } else { None }
    }

    /// Get selected item text.
    pub fn selected_text(&self) -> &str {
        if self.selected_index >= 0 && (self.selected_index as usize) < self.item_count {
            self.items[self.selected_index as usize].text_str()
        } else {
            ""
        }
    }

    /// Remove item at index.
    pub fn remove_item(&mut self, idx: usize) {
        if idx >= self.item_count { return; }
        for i in idx..self.item_count - 1 {
            self.items[i] = self.items[i + 1];
        }
        self.item_count -= 1;
        if self.selected_index as usize == idx {
            self.selected_index = -1;
        }
    }

    /// Sort items alphabetically.
    pub fn sort(&mut self) {
        // Simple selection sort by text
        for i in 0..self.item_count {
            for j in i + 1..self.item_count {
                let a = self.items[i].text_str();
                let b = self.items[j].text_str();
                let cmp = if self.sort_ascending { a > b } else { a < b };
                if cmp {
                    self.items.swap(i, j);
                }
            }
        }
    }
}

// ── Tree Node ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct TreeNode {
    pub label: [u8; 48],
    pub label_len: usize,
    pub icon_type: IconType,
    pub children: [usize; 8],
    pub child_count: usize,
    pub expanded: bool,
    pub data: usize,
}

impl TreeNode {
    pub fn new(label: &str) -> Self {
        let mut label_buf = [0u8; 48];
        let llen = core::cmp::min(label.len(), 48);
        for (i, b) in label.bytes().enumerate().take(llen) {
            label_buf[i] = b;
        }
        TreeNode {
            label: label_buf,
            label_len: llen,
            icon_type: IconType::None,
            children: [0; 8],
            child_count: 0,
            expanded: false,
            data: 0,
        }
    }

    pub fn label_str(&self) -> &str {
        core::str::from_utf8(&self.label[..self.label_len]).unwrap_or("")
    }
}

// ── Tree View ────────────────────────────────────────────────────────────────

pub struct TreeView {
    pub rect: Rect,
    pub root: TreeNode,
    pub node_pool: [TreeNode; 64],
    pub pool_count: usize,
    pub selected_path: [usize; 8],
    pub selection_depth: usize,
    pub scroll_offset: usize,
    pub expanded_nodes: u64,
    pub bg_color: Color,
    pub text_color: Color,
    pub selection_color: Color,
    pub indent_width: u32,
    pub item_height: u32,
}

impl TreeView {
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        TreeView {
            rect: Rect::new(x, y, w, h),
            root: TreeNode::new("root"),
            node_pool: [TreeNode::new(""); 64],
            pool_count: 0,
            selected_path: [0; 8],
            selection_depth: 0,
            scroll_offset: 0,
            expanded_nodes: 0,
            bg_color: 0xFFFFFFFF,
            text_color: 0xFF000000,
            selection_color: 0xFF4A90D9,
            indent_width: 16,
            item_height: 20,
        }
    }

    /// Add a child node to the pool and return its index.
    pub fn add_child(&mut self, label: &str) -> Option<usize> {
        if self.pool_count >= 64 {
            return None;
        }
        let idx = self.pool_count;
        self.node_pool[idx] = TreeNode::new(label);
        self.pool_count += 1;
        Some(idx)
    }

    /// Expand or collapse a node.
    pub fn toggle_node(&mut self, idx: usize) {
        if idx >= 64 { return; }
        self.node_pool[idx].expanded = !self.node_pool[idx].expanded;
        if self.node_pool[idx].expanded {
            self.expanded_nodes |= 1u64 << idx;
        } else {
            self.expanded_nodes &= !(1u64 << idx);
        }
    }
}

// ── Dialog ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DialogIcon {
    None,
    Info,
    Warning,
    Error,
    Question,
}

#[derive(Clone, Copy, Debug)]
pub struct DialogButton {
    pub label: [u8; 16],
    pub label_len: usize,
    pub is_default: bool,
    pub is_cancel: bool,
    pub result: i32,
}

impl DialogButton {
    pub fn new(label: &str) -> Self {
        let mut label_buf = [0u8; 16];
        let llen = core::cmp::min(label.len(), 16);
        for (i, b) in label.bytes().enumerate().take(llen) {
            label_buf[i] = b;
        }
        DialogButton {
            label: label_buf,
            label_len: llen,
            is_default: false,
            is_cancel: false,
            result: 0,
        }
    }

    pub fn label_str(&self) -> &str {
        core::str::from_utf8(&self.label[..self.label_len]).unwrap_or("")
    }
}

pub struct Dialog {
    pub rect: Rect,
    pub title: [u8; 64],
    pub title_len: usize,
    pub message: [u8; 256],
    pub message_len: usize,
    pub buttons: [DialogButton; 3],
    pub button_count: usize,
    pub icon_type: DialogIcon,
    pub is_modal: bool,
    pub visible: bool,
    pub bg_color: Color,
    pub title_bg: Color,
    pub border_color: Color,
    pub text_color: Color,
}

impl Dialog {
    pub fn new(title: &str, message: &str, w: u32, h: u32) -> Self {
        let mut title_buf = [0u8; 64];
        let tlen = core::cmp::min(title.len(), 64);
        for (i, b) in title.bytes().enumerate().take(tlen) {
            title_buf[i] = b;
        }
        let mut msg_buf = [0u8; 256];
        let mlen = core::cmp::min(message.len(), 256);
        for (i, b) in message.bytes().enumerate().take(mlen) {
            msg_buf[i] = b;
        }
        Dialog {
            rect: Rect::new(0, 0, w, h),
            title: title_buf,
            title_len: tlen,
            message: msg_buf,
            message_len: mlen,
            buttons: [
                DialogButton::new(""),
                DialogButton::new(""),
                DialogButton::new(""),
            ],
            button_count: 0,
            icon_type: DialogIcon::None,
            is_modal: true,
            visible: false,
            bg_color: 0xFFF0F0F0,
            title_bg: 0xFF2D2D44,
            border_color: 0xFF404040,
            text_color: 0xFF000000,
        }
    }

    pub fn title_str(&self) -> &str {
        core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")
    }

    pub fn message_str(&self) -> &str {
        core::str::from_utf8(&self.message[..self.message_len]).unwrap_or("")
    }

    pub fn add_button(&mut self, label: &str, result: i32, is_default: bool, is_cancel: bool) -> bool {
        if self.button_count < 3 {
            let mut btn = DialogButton::new(label);
            btn.result = result;
            btn.is_default = is_default;
            btn.is_cancel = is_cancel;
            self.buttons[self.button_count] = btn;
            self.button_count += 1;
            true
        } else {
            false
        }
    }

    /// Center the dialog within a given parent rect.
    pub fn center_on(&mut self, parent: &Rect) {
        self.rect.x = parent.x + (parent.width as i32 - self.rect.width as i32) / 2;
        self.rect.y = parent.y + (parent.height as i32 - self.rect.height as i32) / 2;
    }

    /// Find which button is at a given point.
    pub fn button_at(&self, x: i32, y: i32) -> Option<usize> {
        let btn_w = 80u32;
        let btn_h = 28u32;
        let total_w = (self.button_count as u32) * (btn_w + 8);
        let start_x = self.rect.x + (self.rect.width as i32 - total_w as i32) / 2;
        let btn_y = self.rect.bottom() - 40;
        for i in 0..self.button_count {
            let bx = start_x + (i as i32) * (btn_w as i32 + 8);
            let br = Rect::new(bx, btn_y, btn_w, btn_h);
            if br.contains(&graphics::Point::new(x, y)) {
                return Some(i);
            }
        }
        None
    }
}

// ── Tooltip ───────────────────────────────────────────────────────────────────

pub struct Tooltip {
    pub text: [u8; 128],
    pub text_len: usize,
    pub visible: bool,
    pub position: graphics::Point,
    /// Computed bounding rect (updated by show()).
    pub rect: Rect,
    pub delay_ms: u64,
    pub show_timer: u64,
    pub bg_color: Color,
    pub text_color: Color,
    pub border_color: Color,
}

impl Tooltip {
    pub fn new() -> Self {
        Tooltip {
            text: [0u8; 128],
            text_len: 0,
            visible: false,
            position: graphics::Point::new(0, 0),
            rect: Rect::new(0, 0, 0, 0),
            delay_ms: 500,
            show_timer: 0,
            bg_color: 0xFFEEEE00,
            text_color: 0xFF000000,
            border_color: 0xFF888888,
        }
    }

    pub fn set_text(&mut self, text: &str) {
        let tlen = core::cmp::min(text.len(), 128);
        for (i, b) in text.bytes().enumerate().take(tlen) {
            self.text[i] = b;
        }
        self.text_len = tlen;
        self.update_rect();
    }

    pub fn text_str(&self) -> &str {
        core::str::from_utf8(&self.text[..self.text_len]).unwrap_or("")
    }

    /// Update the bounding rect from current position and text length.
    fn update_rect(&mut self) {
        let tw = self.text_len as u32 * 8 + 12;
        self.rect = Rect::new(self.position.x, self.position.y, tw, 24);
    }

    pub fn show(&mut self, x: i32, y: i32) {
        self.position.x = x + 10;
        self.position.y = y + 10;
        self.update_rect();
        self.visible = true;
        self.show_timer = 0;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }
}

// ── Status Bar Section ───────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct StatusBarSection {
    pub text: [u8; 64],
    pub text_len: usize,
    pub width_percent: u8,
}

impl StatusBarSection {
    pub fn new(text: &str, width_pct: u8) -> Self {
        let mut text_buf = [0u8; 64];
        let tlen = core::cmp::min(text.len(), 64);
        for (i, b) in text.bytes().enumerate().take(tlen) {
            text_buf[i] = b;
        }
        StatusBarSection {
            text: text_buf,
            text_len: tlen,
            width_percent: width_pct.min(100),
        }
    }

    pub fn text_str(&self) -> &str {
        core::str::from_utf8(&self.text[..self.text_len]).unwrap_or("")
    }

    pub fn set_text(&mut self, s: &str) {
        let tlen = core::cmp::min(s.len(), 64);
        for (i, b) in s.bytes().enumerate().take(tlen) {
            self.text[i] = b;
        }
        self.text_len = tlen;
    }
}

// ── Status Bar ───────────────────────────────────────────────────────────────

pub struct StatusBar {
    pub rect: Rect,
    pub sections: [StatusBarSection; 4],
    pub section_count: usize,
    pub color: Color,
    pub text_color: Color,
    pub border_color: Color,
}

impl StatusBar {
    pub fn new(x: i32, y: i32, w: u32) -> Self {
        StatusBar {
            rect: Rect::new(x, y, w, 24),
            sections: [StatusBarSection::new("", 0); 4],
            section_count: 0,
            color: 0xFF2D2D44,
            text_color: 0xFFFFFFFF,
            border_color: 0xFF404040,
        }
    }

    pub fn add_section(&mut self, text: &str, width_pct: u8) -> bool {
        if self.section_count < 4 {
            self.sections[self.section_count] = StatusBarSection::new(text, width_pct);
            self.section_count += 1;
            true
        } else {
            false
        }
    }

    pub fn set_section(&mut self, idx: usize, text: &str) {
        if idx < self.section_count {
            self.sections[idx].set_text(text);
        }
    }
}

// ── Layout Types ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub enum Layout {
    VBox { spacing: u32, padding: u32 },
    HBox { spacing: u32, padding: u32 },
    Grid { rows: u32, cols: u32, h_spacing: u32, v_spacing: u32 },
    Absolute,
}

// ── Container ────────────────────────────────────────────────────────────────

/// A container with a layout that positions child widgets.
pub struct Container {
    pub rect: Rect,
    pub layout: Layout,
    pub child_rects: [Rect; 16],
    pub child_data: [usize; 16],  // user-defined IDs for children
    pub child_count: usize,
    pub scrollable: bool,
    pub clip_children: bool,
    pub scroll_offset: u32,
    pub bg_color: Color,
    pub border_color: Color,
}

impl Container {
    pub fn new(rect: Rect, layout: Layout) -> Self {
        Container {
            rect,
            layout,
            child_rects: [Rect::new(0, 0, 0, 0); 16],
            child_data: [0; 16],
            child_count: 0,
            scrollable: false,
            clip_children: true,
            scroll_offset: 0,
            bg_color: 0x00000000, // transparent
            border_color: 0x00000000,
        }
    }

    pub fn add_child(&mut self, widget_data: usize) -> bool {
        if self.child_count < 16 {
            self.child_data[self.child_count] = widget_data;
            self.child_count += 1;
            self.layout_children();
            true
        } else {
            false
        }
    }

    /// Compute positions for all children based on the layout.
    pub fn layout_children(&mut self) {
        match self.layout {
            Layout::VBox { spacing, padding } => {
                let mut cy = self.rect.y + padding as i32;
                for i in 0..self.child_count {
                    let remaining = self.child_count - i;
                    let child_h = (self.rect.height as i32 - 2 * padding as i32
                        - (remaining as i32 - 1) * spacing as i32)
                        / remaining as i32;
                    self.child_rects[i] = Rect::new(
                        self.rect.x + padding as i32,
                        cy,
                        self.rect.width - 2 * padding,
                        child_h.max(0) as u32,
                    );
                    cy += child_h + spacing as i32;
                }
            }
            Layout::HBox { spacing, padding } => {
                let mut cx = self.rect.x + padding as i32;
                for i in 0..self.child_count {
                    let remaining = self.child_count - i;
                    let child_w = (self.rect.width as i32 - 2 * padding as i32
                        - (remaining as i32 - 1) * spacing as i32)
                        / remaining as i32;
                    self.child_rects[i] = Rect::new(
                        cx,
                        self.rect.y + padding as i32,
                        child_w.max(0) as u32,
                        self.rect.height - 2 * padding,
                    );
                    cx += child_w + spacing as i32;
                }
            }
            Layout::Grid { rows, cols, h_spacing, v_spacing } => {
                if rows == 0 || cols == 0 { return; }
                let cell_w = (self.rect.width - (cols - 1) * h_spacing) / cols;
                let cell_h = (self.rect.height - (rows - 1) * v_spacing) / rows;
                for i in 0..self.child_count {
                    let r = (i as u32) / cols;
                    let c = (i as u32) % cols;
                    if r >= rows { break; }
                    self.child_rects[i] = Rect::new(
                        self.rect.x + (c * (cell_w + h_spacing)) as i32,
                        self.rect.y + (r * (cell_h + v_spacing)) as i32,
                        cell_w,
                        cell_h,
                    );
                }
            }
            Layout::Absolute => {
                // Children keep their existing rects
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// V39b — Draw Functions for Enhanced Widgets
// ═══════════════════════════════════════════════════════════════════════════════

fn draw_dropdown_menu(fb: &mut Framebuffer, dd: &DropdownMenu) {
    let font = font_8x16();

    // Main selector box
    fb.fill_rect(dd.rect.x as u32, dd.rect.y as u32, dd.rect.width, dd.rect.height, dd.bg_color);
    draw_border(fb, &dd.rect, 1, dd.border_color);

    // Selected text
    let text = dd.selected_text();
    draw_text_wrapped(fb, (dd.rect.x + 4) as u32, (dd.rect.y + 4) as u32,
        dd.rect.width - 24, text, dd.text_color, dd.bg_color);

    // Dropdown arrow (▾)
    let arrow_x = dd.rect.right() - 14;
    let arrow_y = dd.rect.y + 6;
    if dd.is_open {
        // Draw ▲ when open
        fb.fill_rect(arrow_x as u32, arrow_y as u32, 8, 2, dd.text_color);
        fb.fill_rect((arrow_x + 1) as u32, (arrow_y + 2) as u32, 6, 2, dd.text_color);
        fb.fill_rect((arrow_x + 2) as u32, (arrow_y + 4) as u32, 4, 2, dd.text_color);
        fb.fill_rect((arrow_x + 3) as u32, (arrow_y + 6) as u32, 2, 2, dd.text_color);
    } else {
        // Draw ▼
        fb.fill_rect((arrow_x + 3) as u32, arrow_y as u32, 2, 2, dd.text_color);
        fb.fill_rect((arrow_x + 2) as u32, (arrow_y + 2) as u32, 4, 2, dd.text_color);
        fb.fill_rect((arrow_x + 1) as u32, (arrow_y + 4) as u32, 6, 2, dd.text_color);
        fb.fill_rect(arrow_x as u32, (arrow_y + 6) as u32, 8, 2, dd.text_color);
    }

    // Dropdown list (when open)
    if dd.is_open {
        let visible = core::cmp::min(dd.item_count, dd.max_visible);
        let list_h = visible as u32 * 20;
        let list_rect = Rect::new(dd.rect.x, dd.rect.bottom(), dd.rect.width, list_h);

        fb.fill_rect(list_rect.x as u32, list_rect.y as u32, list_rect.width, list_rect.height, dd.bg_color);
        draw_border(fb, &list_rect, 1, dd.border_color);

        for i in 0..visible {
            let global_i = dd.scroll_offset + i;
            if global_i >= dd.item_count { break; }
            let item = &dd.items[global_i];
            let iy = list_rect.y + (i as i32) * 20;

            if item.separator {
                fb.fill_rect(list_rect.x as u32, (iy + 9) as u32, list_rect.width, 2, 0xFFCCCCCC);
                continue;
            }

            let bg = if global_i == dd.selected_index {
                if item.enabled { dd.highlight_color } else { 0xFFE0E0E0 }
            } else {
                dd.bg_color
            };
            fb.fill_rect(list_rect.x as u32, iy as u32, list_rect.width, 20, bg);

            // Checkmark
            if item.checked {
                let chk_x = list_rect.x + 4;
                let chk_y = iy + 4;
                fb.draw_line(
                    chk_x as u32, (chk_y + 6) as u32,
                    (chk_x + 4) as u32, (chk_y + 10) as u32,
                    if item.enabled { dd.text_color } else { 0xFF888888 },
                );
                fb.draw_line(
                    (chk_x + 4) as u32, (chk_y + 10) as u32,
                    (chk_x + 10) as u32, (chk_y + 2) as u32,
                    if item.enabled { dd.text_color } else { 0xFF888888 },
                );
            }

            // Label
            let tx = list_rect.x + 18;
            let ty = iy + 2;
            let color = if item.enabled { dd.text_color } else { 0xFF888888 };
            draw_text_wrapped(fb, tx as u32, ty as u32, list_rect.width - 50, item.label_str(), color, bg);

            // Shortcut hint
            if item.shortcut_len > 0 {
                let shortcut = core::str::from_utf8(&item.shortcut[..item.shortcut_len]).unwrap_or("");
                let sx = list_rect.right() - 50;
                draw_text_wrapped(fb, sx as u32, ty as u32, 48, shortcut, 0xFF888888, bg);
            }
        }
    }
}

fn draw_slider(fb: &mut Framebuffer, sl: &Slider) {
    match sl.orientation {
        SliderOrientation::Horizontal => {
            let track_y = sl.rect.y + (sl.rect.height / 2) as i32 - 2;
            // Track
            fb.fill_rect(
                (sl.rect.x + sl.thumb_radius as i32) as u32,
                track_y as u32,
                sl.rect.width - 2 * sl.thumb_radius,
                4,
                sl.track_color,
            );
            // Fill (left portion)
            let t = ((sl.value - sl.min) / (sl.max - sl.min)).max(0.0).min(1.0);
            let fill_w = ((sl.rect.width as f32 - 2.0 * sl.thumb_radius as f32) * t) as u32;
            if fill_w > 0 {
                fb.fill_rect(
                    (sl.rect.x + sl.thumb_radius as i32) as u32,
                    track_y as u32,
                    fill_w,
                    4,
                    sl.fill_color,
                );
            }
            // Thumb
            let thumb_cx = sl.rect.x + sl.thumb_radius as i32 + fill_w as i32;
            let thumb_cy = sl.rect.y + (sl.rect.height / 2) as i32;
            // Simple circle approximation using filled rect
            let r = sl.thumb_radius;
            fb.fill_rect(
                (thumb_cx - r as i32) as u32,
                (thumb_cy - r as i32) as u32,
                r * 2,
                r * 2,
                sl.thumb_color,
            );
            // Border around thumb
            let thumb_rect = Rect::new(
                thumb_cx - r as i32,
                thumb_cy - r as i32,
                r * 2,
                r * 2,
            );
            draw_border(fb, &thumb_rect, 1, graphics::darken(sl.thumb_color, 0.7));
        }
        SliderOrientation::Vertical => {
            let track_x = sl.rect.x + (sl.rect.width / 2) as i32 - 2;
            fb.fill_rect(
                track_x as u32,
                (sl.rect.y + sl.thumb_radius as i32) as u32,
                4,
                sl.rect.height - 2 * sl.thumb_radius,
                sl.track_color,
            );
            let t = ((sl.value - sl.min) / (sl.max - sl.min)).max(0.0).min(1.0);
            let fill_h = ((sl.rect.height as f32 - 2.0 * sl.thumb_radius as f32) * t) as u32;
            if fill_h > 0 {
                fb.fill_rect(
                    track_x as u32,
                    (sl.rect.y + sl.thumb_radius as i32) as u32 + (sl.rect.height - 2 * sl.thumb_radius - fill_h),
                    4,
                    fill_h,
                    sl.fill_color,
                );
            }
            let thumb_cx = sl.rect.x + (sl.rect.width / 2) as i32;
            let thumb_cy = sl.rect.bottom() - sl.thumb_radius as i32 - fill_h as i32;
            let r = sl.thumb_radius;
            fb.fill_rect(
                (thumb_cx - r as i32) as u32,
                (thumb_cy - r as i32) as u32,
                r * 2,
                r * 2,
                sl.thumb_color,
            );
        }
    }

    // Value label
    if sl.show_value {
        let mut val_buf = [0u8; 16];
        let vlen = format_value(sl.value, sl.min, sl.max, &mut val_buf);
        let val_str = core::str::from_utf8(&val_buf[..vlen]).unwrap_or("");
        let vx = sl.rect.right() - 40;
        let vy = sl.rect.y + 2;
        draw_text_wrapped(fb, vx as u32, vy as u32, 38, val_str, sl.fill_color, 0x00000000);
    }
}

/// Format a slider value as a short string in a fixed buffer.
fn format_value(val: f32, min: f32, max: f32, buf: &mut [u8; 16]) -> usize {
    if max <= 1.0 && min >= 0.0 {
        let pct = (val * 100.0) as u32;
        if pct >= 100 {
            buf[0] = b'1'; buf[1] = b'0'; buf[2] = b'0'; buf[3] = b'%';
            4
        } else if pct >= 10 {
            buf[0] = b'0' + (pct / 10) as u8;
            buf[1] = b'0' + (pct % 10) as u8;
            buf[2] = b'%';
            3
        } else {
            buf[0] = b'0' + pct as u8;
            buf[1] = b'%';
            2
        }
    } else {
        // Simple integer display
        let ival = val as u32;
        if ival >= 1000 {
            buf[0] = b'0' + (ival / 1000) as u8;
            buf[1] = b'0' + ((ival / 100) % 10) as u8;
            buf[2] = b'0' + ((ival / 10) % 10) as u8;
            buf[3] = b'0' + (ival % 10) as u8;
            4
        } else if ival >= 100 {
            buf[0] = b'0' + (ival / 100) as u8;
            buf[1] = b'0' + ((ival / 10) % 10) as u8;
            buf[2] = b'0' + (ival % 10) as u8;
            3
        } else if ival >= 10 {
            buf[0] = b'0' + (ival / 10) as u8;
            buf[1] = b'0' + (ival % 10) as u8;
            2
        } else {
            buf[0] = b'0' + ival as u8;
            1
        }
    }
}

fn draw_tab_bar(fb: &mut Framebuffer, tb: &TabBar) {
    // Background
    fb.fill_rect(tb.rect.x as u32, tb.rect.y as u32, tb.rect.width, tb.rect.height, tb.bg_color);

    let tw = tb.tab_width();
    let font = font_8x16();

    for i in 0..tb.tab_count {
        let tab = &tb.tabs[i];
        let tab_rect = Rect::new(
            tb.rect.x + (i as u32 * tw) as i32,
            tb.rect.y,
            tw,
            tb.tab_height,
        );

        let bg = if i == tb.active_tab { tb.active_color } else { tb.inactive_color };

        // Tab background
        fb.fill_rect(tab_rect.x as u32, tab_rect.y as u32, tab_rect.width, tab_rect.height, bg);

        // Active tab gets a top accent line
        if i == tb.active_tab {
            fb.fill_rect(tab_rect.x as u32, tab_rect.y as u32, tab_rect.width, 2, 0xFF4A90D9);
            // Bottom line omitted for active tab (blends with content area)
        } else {
            // Bottom border for inactive tabs
            fb.fill_rect(tab_rect.x as u32, tab_rect.bottom() as u32 - 1, tab_rect.width, 1, 0xFFCCCCCC);
        }

        // Tab label
        let label = tab.label_str();
        let lx = tab_rect.x + (tab_rect.width as i32 - (label.len() as u32 * 8) as i32) / 2;
        let ly = tab_rect.y + 6;
        draw_text_wrapped(fb, lx.max(0) as u32, ly.max(0) as u32,
            tab_rect.width, label, tb.text_color, bg);

        // Close button (if closable and active)
        if tab.closable && i == tb.active_tab {
            let cx = tab_rect.right() - 16;
            let cy = tab_rect.y + 6;
            fb.draw_line(
                cx as u32, cy as u32,
                (cx + 8) as u32, (cy + 8) as u32,
                0xFF888888,
            );
            fb.draw_line(
                (cx + 8) as u32, cy as u32,
                cx as u32, (cy + 8) as u32,
                0xFF888888,
            );
        }
    }

    // Bottom separator line (for inactive tabs area)
    if tb.tab_count > 0 {
        let last_right = tb.rect.x + (tb.tab_count as u32 * tw) as i32;
        if last_right < tb.rect.right() {
            fb.fill_rect(
                last_right as u32,
                (tb.rect.bottom() - 1) as u32,
                (tb.rect.right() - last_right) as u32,
                1,
                0xFFCCCCCC,
            );
        }
    }
}

fn draw_list_view(fb: &mut Framebuffer, lv: &ListView) {
    // Background
    fb.fill_rect(lv.rect.x as u32, lv.rect.y as u32, lv.rect.width, lv.rect.height, lv.bg_color);
    draw_border(fb, &lv.rect, 1, lv.border_color);

    // Clip visible region
    let font = font_8x16();

    let end = core::cmp::min(
        lv.scroll_offset + lv.visible_count,
        lv.item_count,
    );

    for i in lv.scroll_offset..end {
        let row_idx = i - lv.scroll_offset;
        let iy = lv.rect.y + (row_idx as u32 * lv.item_height) as i32;

        // Off-screen check
        if iy + lv.item_height as i32 > lv.rect.bottom() { break; }

        let is_selected = i == lv.selected_index as usize
            || lv.selected_indices.contains(&(i as isize));

        let bg = if is_selected {
            lv.selection_color
        } else if i % 2 == 1 {
            lv.alt_row_color
        } else {
            lv.bg_color
        };

        fb.fill_rect(lv.rect.x as u32, iy as u32, lv.rect.width, lv.item_height, bg);

        // Icon indicator (simple colored square)
        let item = &lv.items[i];
        match item.icon_type {
            IconType::Folder => {
                fb.fill_rect((lv.rect.x + 3) as u32, (iy + 4) as u32, 12, 12, 0xFFFFD700);
            }
            IconType::File => {
                fb.fill_rect((lv.rect.x + 4) as u32, (iy + 4) as u32, 10, 12, 0xFFD0D0D0);
            }
            IconType::Image => {
                fb.fill_rect((lv.rect.x + 4) as u32, (iy + 4) as u32, 10, 12, 0xFF00AAFF);
            }
            IconType::Executable => {
                fb.fill_rect((lv.rect.x + 4) as u32, (iy + 4) as u32, 10, 12, 0xFF44CC44);
            }
            _ => {}
        }

        // Text
        let tx = lv.rect.x + 20;
        let ty = iy + 2;
        let text = item.text_str();
        draw_text_wrapped(fb, tx as u32, ty as u32,
            lv.rect.width - 24, text,
            if is_selected { 0xFFFFFFFF } else { lv.text_color },
            bg,
        );
    }

    // Scroll indicator (if content exceeds visible area)
    if lv.item_count > lv.visible_count {
        let bar_h = lv.rect.height * lv.visible_count as u32 / lv.item_count as u32;
        let bar_y = lv.rect.y as u32
            + (lv.rect.height - bar_h) * lv.scroll_offset as u32
            / (lv.item_count - lv.visible_count) as u32;
        fb.fill_rect(
            (lv.rect.right() - 6) as u32,
            bar_y.max(lv.rect.y as u32),
            4,
            bar_h.max(16).min(lv.rect.height),
            0xFFAAAAAA,
        );
    }
}

fn draw_tree_view(fb: &mut Framebuffer, tv: &TreeView) {
    // Background
    fb.fill_rect(tv.rect.x as u32, tv.rect.y as u32, tv.rect.width, tv.rect.height, tv.bg_color);
    draw_border(fb, &tv.rect, 1, 0xFFCCCCCC);

    let font = font_8x16();
    let max_y = tv.rect.bottom();

    // Walk tree iteratively: render one level at a time
    // Level 0: root
    let mut y = tv.rect.y;

    // Render a single node and recurse into children (as a separate function)
    // Use a helper function that takes tv as pointer
    render_tree_node(fb, tv, &tv.root, 0, &mut y, max_y, font);

    // Then render root's children from pool
    for ci in 0..tv.root.child_count {
        let child_idx = tv.root.children[ci];
        if child_idx < tv.pool_count {
            let child = &tv.node_pool[child_idx];
            render_tree_node(fb, tv, child, 1, &mut y, max_y, font);
        }
    }
}

/// Recursive tree node rendering helper.
fn render_tree_node(
    fb: &mut Framebuffer,
    tv: &TreeView,
    node: &TreeNode,
    depth: usize,
    y: &mut i32,
    max_y: i32,
    _font: &[u8],
) {
    if *y >= max_y { return; }

    let indent = depth as u32 * tv.indent_width;
    let nx = tv.rect.x + indent as i32;

    // Expand/collapse arrow
    if node.child_count > 0 {
        let arrow_x = nx;
        let arrow_y = *y + 6;
        if node.expanded {
            fb.fill_rect(arrow_x as u32, (arrow_y + 2) as u32, 8, 2, tv.text_color);
            fb.fill_rect((arrow_x + 1) as u32, (arrow_y + 4) as u32, 6, 2, tv.text_color);
            fb.fill_rect((arrow_x + 2) as u32, (arrow_y + 6) as u32, 4, 2, tv.text_color);
            fb.fill_rect((arrow_x + 3) as u32, (arrow_y + 8) as u32, 2, 2, tv.text_color);
        } else {
            fb.fill_rect((arrow_x + 2) as u32, arrow_y as u32, 2, 2, tv.text_color);
            fb.fill_rect((arrow_x + 4) as u32, (arrow_y + 2) as u32, 2, 2, tv.text_color);
            fb.fill_rect(arrow_x as u32, (arrow_y + 4) as u32, 8, 2, tv.text_color);
            fb.fill_rect((arrow_x + 4) as u32, (arrow_y + 6) as u32, 2, 2, tv.text_color);
            fb.fill_rect((arrow_x + 2) as u32, (arrow_y + 8) as u32, 2, 2, tv.text_color);
        }
    }

    // Icon
    let icon_x = nx + 12;
    match node.icon_type {
        IconType::Folder => {
            fb.fill_rect(icon_x as u32, (*y + 4) as u32, 12, 12, 0xFFFFD700);
        }
        IconType::File => {
            fb.fill_rect(icon_x as u32, (*y + 4) as u32, 10, 12, 0xFFD0D0D0);
        }
        _ => {}
    }

    // Label
    let tx = icon_x + 16;
    draw_text_wrapped(fb, tx as u32, (*y + 2) as u32,
        tv.rect.width - (tx - tv.rect.x) as u32,
        node.label_str(), tv.text_color, tv.bg_color);

    *y += tv.item_height as i32;

    // Recurse into children if expanded
    if node.expanded {
        for ci in 0..node.child_count {
            let child_idx = node.children[ci];
            if child_idx < tv.pool_count {
                let child = &tv.node_pool[child_idx];
                render_tree_node(fb, tv, child, depth + 1, y, max_y, _font);
                if *y >= max_y { return; }
            }
        }
    }
}

fn draw_dialog(fb: &mut Framebuffer, dlg: &Dialog) {
    if !dlg.visible { return; }
    let font = font_8x16();

    // Shadow
    let shadow_rect = Rect::new(
        dlg.rect.x + 3,
        dlg.rect.y + 3,
        dlg.rect.width,
        dlg.rect.height,
    );
    graphics::draw_shadow(fb, &shadow_rect, 4, 40);

    // Modal overlay (semi-transparent background)
    if dlg.is_modal {
        // Light overlay tint for full screen — skipped for simplicity
    }

    // Dialog border
    draw_border(fb, &dlg.rect, 2, dlg.border_color);

    // Dialog background
    fb.fill_rect(
        (dlg.rect.x + 2) as u32,
        (dlg.rect.y + 2) as u32,
        dlg.rect.width - 4,
        dlg.rect.height - 4,
        dlg.bg_color,
    );

    // Title bar
    let title_bar = Rect::new(
        dlg.rect.x + 2,
        dlg.rect.y + 2,
        dlg.rect.width - 4,
        24,
    );
    fb.fill_rect(title_bar.x as u32, title_bar.y as u32, title_bar.width, title_bar.height, dlg.title_bg);
    draw_text_wrapped(fb,
        (title_bar.x + 6) as u32, (title_bar.y + 4) as u32,
        title_bar.width - 12,
        dlg.title_str(), 0xFFFFFFFF, dlg.title_bg);

    // Dialog icon
    let icon_x = dlg.rect.x + 16;
    let icon_y = dlg.rect.y + 50;
    match dlg.icon_type {
        DialogIcon::Info => {
            fb.fill_rect(icon_x as u32, icon_y as u32, 24, 24, 0xFF4A90D9);
            // "i"
            draw_text_wrapped(fb, (icon_x + 9) as u32, (icon_y + 4) as u32, 10, "i", 0xFFFFFFFF, 0xFF4A90D9);
        }
        DialogIcon::Warning => {
            fb.fill_rect(icon_x as u32, icon_y as u32, 24, 24, 0xFFFFAA00);
            // "!"
            draw_text_wrapped(fb, (icon_x + 9) as u32, (icon_y + 4) as u32, 10, "!", 0xFFFFFFFF, 0xFFFFAA00);
        }
        DialogIcon::Error => {
            fb.fill_rect(icon_x as u32, icon_y as u32, 24, 24, 0xFFCC3333);
            // "X"
            draw_text_wrapped(fb, (icon_x + 8) as u32, (icon_y + 4) as u32, 10, "X", 0xFFFFFFFF, 0xFFCC3333);
        }
        DialogIcon::Question => {
            fb.fill_rect(icon_x as u32, icon_y as u32, 24, 24, 0xFF4A90D9);
            // "?"
            draw_text_wrapped(fb, (icon_x + 9) as u32, (icon_y + 4) as u32, 10, "?", 0xFFFFFFFF, 0xFF4A90D9);
        }
        DialogIcon::None => {}
    }

    // Message text
    let msg_x = if dlg.icon_type == DialogIcon::None {
        dlg.rect.x + 16
    } else {
        dlg.rect.x + 52
    };
    let msg_w = dlg.rect.right() - msg_x - 16;
    draw_text_wrapped(fb,
        msg_x as u32, (dlg.rect.y + 50) as u32,
        msg_w.max(10) as u32,
        dlg.message_str(), dlg.text_color, dlg.bg_color);

    // Buttons
    let btn_w = 80u32;
    let btn_h = 28u32;
    let total_w = (dlg.button_count as u32) * (btn_w + 8);
    let start_x = dlg.rect.x + (dlg.rect.width as i32 - total_w as i32) / 2;
    let btn_y = dlg.rect.bottom() - 40;

    for i in 0..dlg.button_count {
        let btn = &dlg.buttons[i];
        let bx = start_x + (i as i32) * (btn_w as i32 + 8);
        let b_rect = Rect::new(bx, btn_y, btn_w, btn_h);

        let face = if btn.is_default { 0xFF4A90D9 } else { 0xFFE0E0E0 };
        let text = if btn.is_default { 0xFFFFFFFF } else { 0xFF000000 };

        fb.fill_rect(b_rect.x as u32, b_rect.y as u32, b_rect.width, b_rect.height, face);
        draw_border(fb, &b_rect, 1, 0xFF888888);

        let label = btn.label_str();
        draw_text_centered(fb, &b_rect, label, text, face);
    }
}

fn draw_tooltip(fb: &mut Framebuffer, tt: &Tooltip) {
    if !tt.visible { return; }
    let text = tt.text_str();
    if text.is_empty() { return; }

    let tw = text.len() as u32 * 8 + 12;
    let th = 24u32;
    let tip_rect = Rect::new(tt.position.x, tt.position.y, tw, th);

    fb.fill_rect(tip_rect.x as u32, tip_rect.y as u32, tip_rect.width, tip_rect.height, tt.bg_color);
    draw_border(fb, &tip_rect, 1, tt.border_color);

    draw_text_wrapped(fb,
        (tip_rect.x + 6) as u32, (tip_rect.y + 4) as u32,
        tw - 12,
        text, tt.text_color, tt.bg_color);
}

fn draw_status_bar(fb: &mut Framebuffer, sb: &StatusBar) {
    fb.fill_rect(sb.rect.x as u32, sb.rect.y as u32, sb.rect.width, sb.rect.height, sb.color);
    draw_border(fb, &sb.rect, 1, sb.border_color);

    let font = font_8x16();
    let mut cx = sb.rect.x + 6;

    for i in 0..sb.section_count {
        let sec = &sb.sections[i];
        let sec_w = (sb.rect.width as u32 * sec.width_percent as u32) / 100;
        let text = sec.text_str();

        draw_text_wrapped(fb, cx as u32, (sb.rect.y + 4) as u32,
            sec_w - 6, text, sb.text_color, sb.color);

        cx += sec_w as i32;

        // Section separator
        if i < sb.section_count - 1 {
            fb.fill_rect(cx as u32, (sb.rect.y + 4) as u32, 1, sb.rect.height - 8, 0xFF555566);
            cx += 6;
        }
    }
}

fn draw_container(fb: &mut Framebuffer, ct: &Container) {
    // Background
    if ct.bg_color & 0xFF000000 != 0 {
        fb.fill_rect(ct.rect.x as u32, ct.rect.y as u32,
            ct.rect.width, ct.rect.height, ct.bg_color);
    }
    if ct.border_color & 0xFF000000 != 0 {
        draw_border(fb, &ct.rect, 1, ct.border_color);
    }

    // Scroll indicator
    if ct.scrollable {
        let content_h = ct.child_count as u32 * 100; // estimated
        if content_h > ct.rect.height {
            let bar_h = ct.rect.height * ct.rect.height / content_h;
            let bar_y = ct.rect.y as u32 + ct.scroll_offset;
            fb.fill_rect(
                (ct.rect.right() - 6) as u32,
                bar_y,
                4,
                bar_h.max(16).min(ct.rect.height),
                0xFFAAAAAA,
            );
        }
    }
}

// ── Fixed-size allocator for widget IDs ───────────────────────────────────────

/// A simple widget registry that assigns IDs to widgets.
/// Used by desktop applications to manage collections of widgets.
pub struct WidgetRegistry {
    pub widgets: [Option<Widget>; 32],
}

impl WidgetRegistry {
    pub fn new() -> Self {
        const NONE: Option<Widget> = None;
        WidgetRegistry {
            widgets: [NONE; 32],
        }
    }

    /// Allocate a new widget ID and store the widget.
    pub fn alloc(&mut self, widget: Widget) -> Option<usize> {
        for i in 0..32 {
            if self.widgets[i].is_none() {
                self.widgets[i] = Some(widget);
                return Some(i);
            }
        }
        None
    }

    /// Free a widget by ID.
    pub fn free(&mut self, id: usize) {
        if id < 32 {
            self.widgets[id] = None;
        }
    }

    /// Get a mutable reference to a widget by ID.
    pub fn get_mut(&mut self, id: usize) -> Option<&mut Widget> {
        if id < 32 {
            self.widgets[id].as_mut()
        } else {
            None
        }
    }

    /// Get a reference to a widget by ID.
    pub fn get(&self, id: usize) -> Option<&Widget> {
        if id < 32 {
            self.widgets[id].as_ref()
        } else {
            None
        }
    }
}
