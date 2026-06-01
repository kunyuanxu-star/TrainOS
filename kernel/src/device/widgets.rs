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
    };
    rect.contains(&graphics::Point::new(x, y))
}
