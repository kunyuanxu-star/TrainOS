// V39b — Desktop Terminal Emulator
//
// Provides a graphical terminal emulator with ANSI escape sequence
// parsing, scrollback buffer, selection, and keyboard input handling
// for the TrainOS desktop environment.

use super::framebuffer::Framebuffer;
use super::graphics::{
    self, draw_text_wrapped, font_8x16, Color, BLACK, WHITE, Rect,
};
use super::widgets::ScrollBarWidget;

// ── Terminal Cell ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct TerminalCell {
    pub ch: u8,
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

impl TerminalCell {
    pub const fn new() -> Self {
        TerminalCell {
            ch: b' ',
            fg: 0xFFD0D0D0,
            bg: 0xFF1E1E2E,
            bold: false,
            italic: false,
            underline: false,
            inverse: false,
        }
    }
}

/// Default terminal color scheme.
const TERM_BLACK: Color = 0xFF000000;
const TERM_RED: Color = 0xFFFF3333;
const TERM_GREEN: Color = 0xFF33FF33;
const TERM_YELLOW: Color = 0xFFFFFF33;
const TERM_BLUE: Color = 0xFF3399FF;
const TERM_MAGENTA: Color = 0xFFFF33FF;
const TERM_CYAN: Color = 0xFF33FFFF;
const TERM_WHITE: Color = 0xFFFFFFFF;
const TERM_BRIGHT_BLACK: Color = 0xFF666666;

/// 16-color ANSI palette.
fn ansi_color(code: u8, bright: bool) -> Color {
    let c = match code % 8 {
        0 => TERM_BLACK,
        1 => TERM_RED,
        2 => TERM_GREEN,
        3 => TERM_YELLOW,
        4 => TERM_BLUE,
        5 => TERM_MAGENTA,
        6 => TERM_CYAN,
        7 => TERM_WHITE,
        _ => TERM_WHITE,
    };
    if bright {
        // Lighten by mixing with white
        graphics::lighten(c, 0.5)
    } else {
        c
    }
}

fn color_256(idx: u8) -> Color {
    if idx < 16 {
        ansi_color(idx, idx >= 8)
    } else if idx < 232 {
        // 6x6x6 color cube
        let r = (idx - 16) / 36;
        let g = ((idx - 16) / 6) % 6;
        let b = (idx - 16) % 6;
        let rv = (r * 255 / 5).max(0).min(255);
        let gv = (g * 255 / 5).max(0).min(255);
        let bv = (b * 255 / 5).max(0).min(255);
        graphics::rgba(rv as u8, gv as u8, bv as u8, 0xFF)
    } else {
        // Grayscale
        let v = (idx - 232) * 10 + 8;
        graphics::rgba(v, v, v, 0xFF)
    }
}

// ── ANSI Parser ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
enum AnsiState {
    Normal,
    Escape,
    Csi,
    Osc,
}

/// Actions produced by the ANSI parser.
#[derive(Clone, Copy, Debug)]
pub enum TerminalAction {
    Print(u8),
    MoveCursor { row: u32, col: u32 },
    MoveCursorUp(u32),
    MoveCursorDown(u32),
    MoveCursorLeft(u32),
    MoveCursorRight(u32),
    ClearScreen,
    ClearLine,
    ClearToEndOfLine,
    ClearToEndOfScreen,
    SetFg(Color),
    SetBg(Color),
    SetBold(bool),
    SetItalic(bool),
    SetUnderline(bool),
    SetInverse(bool),
    ResetAttributes,
    SaveCursor,
    RestoreCursor,
    ScrollUp(u32),
    ScrollDown(u32),
    Bell,
    SetCursorVisible(bool),
    CarriageReturn,
    LineFeed,
    Backspace,
    Tab,
}

/// Parses ANSI escape sequences from a byte stream.
pub struct AnsiParser {
    state: AnsiState,
    params: [u16; 16],
    param_count: usize,
    current_param: u16,
    saved_x: u32,
    saved_y: u32,
    buf: [u8; 32],
    buf_len: usize,
}

impl AnsiParser {
    pub fn new() -> Self {
        AnsiParser {
            state: AnsiState::Normal,
            params: [0u16; 16],
            param_count: 0,
            current_param: 0,
            saved_x: 0,
            saved_y: 0,
            buf: [0u8; 32],
            buf_len: 0,
        }
    }

    /// Feed one byte into the parser. Returns actions to perform.
    pub fn feed(&mut self, byte: u8) -> Option<TerminalAction> {
        match self.state {
            AnsiState::Normal => {
                match byte {
                    0x1B => {
                        self.state = AnsiState::Escape;
                        self.param_count = 0;
                        self.current_param = 0;
                        self.buf_len = 0;
                        None
                    }
                    0x0A => Some(TerminalAction::LineFeed),
                    0x0D => Some(TerminalAction::CarriageReturn),
                    0x08 => Some(TerminalAction::Backspace),
                    0x09 => Some(TerminalAction::Tab),
                    0x07 => Some(TerminalAction::Bell),
                    0x7F => Some(TerminalAction::Backspace),
                    _ if byte >= 0x20 => Some(TerminalAction::Print(byte)),
                    _ => None,
                }
            }
            AnsiState::Escape => {
                match byte {
                    b'[' => {
                        self.state = AnsiState::Csi;
                        self.params = [0u16; 16];
                        self.param_count = 0;
                        self.current_param = 0;
                        None
                    }
                    b']' => {
                        self.state = AnsiState::Osc;
                        self.buf_len = 0;
                        None
                    }
                    b'7' => {
                        self.state = AnsiState::Normal;
                        Some(TerminalAction::SaveCursor)
                    }
                    b'8' => {
                        self.state = AnsiState::Normal;
                        Some(TerminalAction::RestoreCursor)
                    }
                    b'D' => {
                        self.state = AnsiState::Normal;
                        Some(TerminalAction::MoveCursorDown(1))
                    }
                    b'M' => {
                        self.state = AnsiState::Normal;
                        Some(TerminalAction::MoveCursorUp(1))
                    }
                    b'c' => {
                        self.state = AnsiState::Normal;
                        Some(TerminalAction::ResetAttributes)
                    }
                    _ => {
                        self.state = AnsiState::Normal;
                        None
                    }
                }
            }
            AnsiState::Csi => {
                match byte {
                    b'0'..=b'9' => {
                        self.current_param = self.current_param * 10 + (byte - b'0') as u16;
                        None
                    }
                    b';' => {
                        if self.param_count < 16 {
                            self.params[self.param_count] = self.current_param;
                            self.param_count += 1;
                        }
                        self.current_param = 0;
                        None
                    }
                    b'A' => {
                        self.finalize_param();
                        let n = self.get_param(0, 1);
                        self.state = AnsiState::Normal;
                        Some(TerminalAction::MoveCursorUp(n as u32))
                    }
                    b'B' => {
                        self.finalize_param();
                        let n = self.get_param(0, 1);
                        self.state = AnsiState::Normal;
                        Some(TerminalAction::MoveCursorDown(n as u32))
                    }
                    b'C' => {
                        self.finalize_param();
                        let n = self.get_param(0, 1);
                        self.state = AnsiState::Normal;
                        Some(TerminalAction::MoveCursorRight(n as u32))
                    }
                    b'D' => {
                        self.finalize_param();
                        let n = self.get_param(0, 1);
                        self.state = AnsiState::Normal;
                        Some(TerminalAction::MoveCursorLeft(n as u32))
                    }
                    b'H' | b'f' => {
                        self.finalize_param();
                        let row = self.get_param(0, 1).saturating_sub(1) as u32;
                        let col = self.get_param(1, 1).saturating_sub(1) as u32;
                        self.state = AnsiState::Normal;
                        Some(TerminalAction::MoveCursor { row, col })
                    }
                    b'J' => {
                        self.finalize_param();
                        let n = self.get_param(0, 0);
                        self.state = AnsiState::Normal;
                        match n {
                            0 => Some(TerminalAction::ClearToEndOfScreen),
                            1 => Some(TerminalAction::ClearToEndOfScreen), // simplified
                            2 => Some(TerminalAction::ClearScreen),
                            _ => Some(TerminalAction::ClearScreen),
                        }
                    }
                    b'K' => {
                        self.finalize_param();
                        let n = self.get_param(0, 0);
                        self.state = AnsiState::Normal;
                        match n {
                            0 => Some(TerminalAction::ClearToEndOfLine),
                            1 => Some(TerminalAction::ClearToEndOfLine), // simplified
                            2 => Some(TerminalAction::ClearLine),
                            _ => Some(TerminalAction::ClearLine),
                        }
                    }
                    b'm' => {
                        self.finalize_param();
                        self.state = AnsiState::Normal;
                        if self.param_count == 0 {
                            return Some(TerminalAction::ResetAttributes);
                        }
                        // Process SGR parameters
                        // We produce the last meaningful action
                        let mut action = None;
                        let mut i = 0;
                        while i <= self.param_count {
                            let p = if i == self.param_count {
                                self.current_param
                            } else {
                                self.params[i]
                            };
                            match p {
                                0 => action = Some(TerminalAction::ResetAttributes),
                                1 => action = Some(TerminalAction::SetBold(true)),
                                3 => action = Some(TerminalAction::SetItalic(true)),
                                4 => action = Some(TerminalAction::SetUnderline(true)),
                                7 => action = Some(TerminalAction::SetInverse(true)),
                                22 => action = Some(TerminalAction::SetBold(false)),
                                23 => action = Some(TerminalAction::SetItalic(false)),
                                24 => action = Some(TerminalAction::SetUnderline(false)),
                                27 => action = Some(TerminalAction::SetInverse(false)),
                                30..=37 => {
                                    action = Some(TerminalAction::SetFg(ansi_color((p - 30) as u8, false)));
                                }
                                38 => {
                                    // Extended foreground color
                                    if i + 2 <= self.param_count && self.params[i + 1] == 5 {
                                        let cidx = self.params[i + 2] as u8;
                                        action = Some(TerminalAction::SetFg(color_256(cidx)));
                                        i += 2;
                                    }
                                }
                                40..=47 => {
                                    action = Some(TerminalAction::SetBg(ansi_color((p - 40) as u8, false)));
                                }
                                48 => {
                                    if i + 2 <= self.param_count && self.params[i + 1] == 5 {
                                        let cidx = self.params[i + 2] as u8;
                                        action = Some(TerminalAction::SetBg(color_256(cidx)));
                                        i += 2;
                                    }
                                }
                                90..=97 => {
                                    action = Some(TerminalAction::SetFg(ansi_color((p - 90) as u8, true)));
                                }
                                100..=107 => {
                                    action = Some(TerminalAction::SetBg(ansi_color((p - 100) as u8, true)));
                                }
                                _ => {}
                            }
                            i += 1;
                        }
                        action
                    }
                    b's' => {
                        self.state = AnsiState::Normal;
                        Some(TerminalAction::SaveCursor)
                    }
                    b'u' => {
                        self.state = AnsiState::Normal;
                        Some(TerminalAction::RestoreCursor)
                    }
                    b'l' | b'h' => {
                        // DEC mode set/reset — simplified
                        self.state = AnsiState::Normal;
                        None
                    }
                    _ => {
                        self.state = AnsiState::Normal;
                        None
                    }
                }
            }
            AnsiState::Osc => {
                if byte == 0x07 || byte == 0x1B {
                    self.state = AnsiState::Normal;
                }
                None
            }
        }
    }

    fn finalize_param(&mut self) {
        if self.param_count < 16 {
            self.params[self.param_count] = self.current_param;
            self.param_count += 1;
        }
        self.current_param = 0;
    }

    fn get_param(&self, idx: usize, default: u16) -> u16 {
        if idx < self.param_count && self.params[idx] > 0 {
            self.params[idx]
        } else {
            default
        }
    }
}

// ── Terminal Emulator ────────────────────────────────────────────────────────

const MAX_COLS: u32 = 256;
const MAX_ROWS: u32 = 64;
const SCROLLBACK_SIZE: usize = 8;

/// Graphical Terminal Emulator.
pub struct TerminalEmulator {
    pub window_id: usize,
    pub cols: u32,
    pub rows: u32,
    /// Character grid buffer [row][col]
    pub screen: [[TerminalCell; 256]; 64],
    pub cursor_x: u32,
    pub cursor_y: u32,
    /// Scrollback buffer (up to 8 extra screens)
    pub scrollback: [[TerminalCell; 256]; 512],
    pub scrollback_rows: usize,
    pub scrollback_offset: usize,
    /// Visual
    pub fg_color: Color,
    pub bg_color: Color,
    pub cursor_color: Color,
    pub selection_color: Color,
    pub font_size: u32,
    /// Selection
    pub selection_start: Option<(u32, u32)>,
    pub selection_end: Option<(u32, u32)>,
    /// Settings
    pub cursor_visible: bool,
    pub cursor_blink: bool,
    pub cursor_blink_state: bool,
    pub cursor_blink_timer: u64,
    pub cursor_row: u32,
    pub cursor_col: u32,
    /// Scrollbar
    pub scrollbar: ScrollBarWidget,
    /// ANSI parser
    pub parser: AnsiParser,
    /// Bold/inverse flags
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    /// Dirty flag
    pub dirty: bool,
    /// Window client rect
    pub client_rect: Rect,
}

impl TerminalEmulator {
    pub fn new(window_id: usize, cols: u32, rows: u32) -> Self {
        let ccols = core::cmp::min(cols, MAX_COLS);
        let rrows = core::cmp::min(rows, MAX_ROWS);

        TerminalEmulator {
            window_id,
            cols: ccols,
            rows: rrows,
            screen: [[TerminalCell::new(); 256]; 64],
            cursor_x: 0,
            cursor_y: 0,
            scrollback: [[TerminalCell::new(); 256]; 512],
            scrollback_rows: 0,
            scrollback_offset: 0,
            fg_color: 0xFFD0D0D0,
            bg_color: 0xFF1E1E2E,
            cursor_color: 0xFFD0D0D0,
            selection_color: 0xFF4A525A,
            font_size: 16,
            selection_start: None,
            selection_end: None,
            cursor_visible: true,
            cursor_blink: true,
            cursor_blink_state: true,
            cursor_blink_timer: 0,
            cursor_row: 0,
            cursor_col: 0,
            scrollbar: ScrollBarWidget::new(0, 0, 14, 0, true),
            parser: AnsiParser::new(),
            bold: false,
            italic: false,
            underline: false,
            inverse: false,
            dirty: true,
            client_rect: Rect::new(0, 0, cols * 8, rows * 16),
        }
    }

    /// Write a single byte to the terminal (processes ANSI sequences).
    pub fn write(&mut self, byte: u8) {
        if let Some(action) = self.parser.feed(byte) {
            self.apply_action(action);
        }
    }

    /// Write a string to the terminal.
    pub fn write_str(&mut self, s: &str) {
        for &b in s.as_bytes() {
            self.write(b);
        }
    }

    fn apply_action(&mut self, action: TerminalAction) {
        match action {
            TerminalAction::Print(ch) => {
                self.put_char(ch);
            }
            TerminalAction::MoveCursor { row, col } => {
                self.cursor_row = row.min(self.rows - 1);
                self.cursor_col = col.min(self.cols - 1);
            }
            TerminalAction::MoveCursorUp(n) => {
                self.cursor_row = self.cursor_row.saturating_sub(n);
            }
            TerminalAction::MoveCursorDown(n) => {
                self.cursor_row = (self.cursor_row + n).min(self.rows - 1);
            }
            TerminalAction::MoveCursorLeft(n) => {
                self.cursor_col = self.cursor_col.saturating_sub(n);
            }
            TerminalAction::MoveCursorRight(n) => {
                self.cursor_col = (self.cursor_col + n).min(self.cols - 1);
            }
            TerminalAction::ClearScreen => {
                for row in 0..self.rows as usize {
                    for col in 0..self.cols as usize {
                        self.screen[row][col] = TerminalCell::new();
                    }
                }
                self.cursor_row = 0;
                self.cursor_col = 0;
                self.dirty = true;
            }
            TerminalAction::ClearLine => {
                for col in 0..self.cols as usize {
                    self.screen[self.cursor_row as usize][col] = TerminalCell::new();
                }
                self.dirty = true;
            }
            TerminalAction::ClearToEndOfLine => {
                let row = self.cursor_row as usize;
                for col in self.cursor_col as usize..self.cols as usize {
                    self.screen[row][col] = TerminalCell::new();
                }
                self.dirty = true;
            }
            TerminalAction::ClearToEndOfScreen => {
                let start_row = self.cursor_row as usize;
                for row in start_row..self.rows as usize {
                    for col in 0..self.cols as usize {
                        self.screen[row][col] = TerminalCell::new();
                    }
                }
                self.dirty = true;
            }
            TerminalAction::SetFg(c) => self.fg_color = c,
            TerminalAction::SetBg(c) => self.bg_color = c,
            TerminalAction::SetBold(b) => self.bold = b,
            TerminalAction::SetItalic(i) => self.italic = i,
            TerminalAction::SetUnderline(u) => self.underline = u,
            TerminalAction::SetInverse(i) => self.inverse = i,
            TerminalAction::ResetAttributes => {
                self.fg_color = 0xFFD0D0D0;
                self.bg_color = 0xFF1E1E2E;
                self.bold = false;
                self.italic = false;
                self.underline = false;
                self.inverse = false;
            }
            TerminalAction::SaveCursor => {
                self.parser.saved_x = self.cursor_col;
                self.parser.saved_y = self.cursor_row;
            }
            TerminalAction::RestoreCursor => {
                self.cursor_col = self.parser.saved_x;
                self.cursor_row = self.parser.saved_y;
            }
            TerminalAction::ScrollUp(n) => {
                self.scroll_up(n as usize);
            }
            TerminalAction::ScrollDown(n) => {
                self.scroll_down(n as usize);
            }
            TerminalAction::Bell => {
                // Visual bell: flash screen briefly (skip for now)
            }
            TerminalAction::SetCursorVisible(v) => self.cursor_visible = v,
            TerminalAction::CarriageReturn => {
                self.cursor_col = 0;
            }
            TerminalAction::LineFeed => {
                if self.cursor_row + 1 >= self.rows {
                    self.scroll_up(1);
                } else {
                    self.cursor_row += 1;
                }
            }
            TerminalAction::Backspace => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                }
            }
            TerminalAction::Tab => {
                let next_tab = ((self.cursor_col / 8) + 1) * 8;
                self.cursor_col = next_tab.min(self.cols - 1);
            }
        }
    }

    fn put_char(&mut self, ch: u8) {
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.cursor_row += 1;
            if self.cursor_row >= self.rows {
                self.scroll_up(1);
                self.cursor_row = self.rows - 1;
            }
        }

        let row = self.cursor_row as usize;
        let col = self.cursor_col as usize;

        let cell = &mut self.screen[row][col];

        if self.inverse {
            cell.fg = self.bg_color;
            cell.bg = self.fg_color;
        } else {
            cell.fg = self.fg_color;
            cell.bg = self.bg_color;
        }
        cell.ch = if ch == b'\n' || ch == b'\r' { b' ' } else { ch };
        cell.bold = self.bold;
        cell.italic = self.italic;
        cell.underline = self.underline;

        self.cursor_col += 1;
        self.cursor_x = self.cursor_col;
        self.cursor_y = self.cursor_row;
        self.dirty = true;
    }

    fn scroll_up(&mut self, n: usize) {
        // Save top rows to scrollback
        for i in 0..n.min(self.rows as usize) {
            let sb_idx = self.scrollback_rows % 512;
            for col in 0..self.cols as usize {
                self.scrollback[sb_idx][col] = self.screen[i][col];
            }
            self.scrollback_rows += 1;
        }
        let scroll_rows = n.min(self.rows as usize);
        // Shift rows up
        for row in scroll_rows..self.rows as usize {
            for col in 0..self.cols as usize {
                self.screen[row - scroll_rows][col] = self.screen[row][col];
            }
        }
        // Clear bottom rows
        for row in (self.rows as usize - scroll_rows)..self.rows as usize {
            for col in 0..self.cols as usize {
                self.screen[row][col] = TerminalCell::new();
            }
        }
        // Ensure cursor_y is adjusted
        if self.cursor_row >= self.rows.saturating_sub(n as u32) {
            self.cursor_row = self.rows - 1;
        }
        self.dirty = true;
    }

    fn scroll_down(&mut self, n: usize) {
        let scroll_rows = n.min(self.rows as usize);
        for row in (scroll_rows..self.rows as usize).rev() {
            for col in 0..self.cols as usize {
                self.screen[row][col] = self.screen[row - scroll_rows][col];
            }
        }
        for row in 0..scroll_rows {
            for col in 0..self.cols as usize {
                self.screen[row][col] = TerminalCell::new();
            }
        }
        self.dirty = true;
    }

    /// Set the client area rect.
    pub fn set_client_rect(&mut self, x: i32, y: i32, w: u32, h: u32) {
        self.client_rect = Rect::new(x, y, w, h);
        // Recompute cols/rows based on available space
        self.cols = (w / 8).max(8).min(MAX_COLS);
        self.rows = (h / 16).max(4).min(MAX_ROWS);
        self.scrollbar.rect = Rect::new(
            x + w as i32 - 14,
            y,
            14,
            h,
        );
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    /// Render the terminal to the framebuffer.
    pub fn render(&self, fb: &mut Framebuffer) {
        let font = font_8x16();
        let cell_w = 8u32;
        let cell_h = 16u32;

        // Background fill
        let bg = if self.inverse { self.fg_color } else { self.bg_color };
        fb.fill_rect(
            self.client_rect.x as u32,
            self.client_rect.y as u32,
            self.client_rect.width,
            self.client_rect.height,
            bg,
        );

        // Render cells
        for row in 0..self.rows as usize {
            for col in 0..self.cols as usize {
                let cell = &self.screen[row][col];
                let x = self.client_rect.x as u32 + col as u32 * cell_w;
                let y = self.client_rect.y as u32 + row as u32 * cell_h;

                // Check selection
                let is_selected = self.selection_start.map_or(false, |_| false);
                let fg = if is_selected { self.bg_color } else { cell.fg };
                let bg = if is_selected { self.selection_color } else { cell.bg };

                // Background
                if bg != 0xFF000000 || true {
                    fb.fill_rect(x, y, cell_w, cell_h, bg);
                }

                // Character
                if cell.ch != b' ' && cell.ch != 0 {
                    let ch = cell.ch as char;
                    fb.draw_char(x, y, ch, fg, bg, font);

                    // Underline
                    if cell.underline {
                        fb.fill_rect(x, (y + cell_h - 2), cell_w, 1, fg);
                    }
                }
            }
        }

        // Cursor
        if self.cursor_visible && self.cursor_blink_state {
            let cx = self.client_rect.x as u32 + self.cursor_col * cell_w;
            let cy = self.client_rect.y as u32 + self.cursor_row * cell_h;
            fb.fill_rect(cx, cy, cell_w, cell_h, self.cursor_color);

            // Draw the character at cursor position in inverse
            let row = self.cursor_row as usize;
            let col = self.cursor_col as usize;
            if row < self.rows as usize && col < self.cols as usize {
                let cell = self.screen[row][col];
                if cell.ch != b' ' {
                    let ch = cell.ch as char;
                    fb.draw_char(cx, cy, ch, bg, self.cursor_color, font);
                }
            }
        }

        // Scrollbar
        let sb = &self.scrollbar;
        fb.fill_rect(
            sb.rect.x as u32, sb.rect.y as u32,
            sb.rect.width, sb.rect.height,
            0xFF1A1A2E,
        );
        if self.scrollback_rows > self.rows as usize {
            let track_h = sb.rect.height as f32;
            let thumb_h = sb.visible_ratio() * track_h;
            let thumb_y = sb.thumb_pos() * (track_h - thumb_h);
            fb.fill_rect(
                (sb.rect.x + 2) as u32,
                (sb.rect.y as f32 + thumb_y) as u32,
                sb.rect.width - 4,
                thumb_h.max(16.0) as u32,
                0xFF555566,
            );
        }
    }

    // ── Input ───────────────────────────────────────────────────────────────

    /// Handle keyboard input (produces key sequences for the shell).
    pub fn handle_key(&mut self, keycode: u8, modifier: u8) -> Option<u8> {
        use crate::device::input::{
            KEY_BACKSPACE, KEY_ENTER, KEY_TAB, KEY_UP, KEY_DOWN,
            KEY_LEFT, KEY_RIGHT, MOD_CTRL,
        };

        match keycode {
            KEY_ENTER => Some(b'\n'),
            KEY_BACKSPACE => Some(0x7F),
            KEY_TAB => Some(b'\t'),
            KEY_UP => Some(0x1B), // ESC [ A
            KEY_DOWN => Some(0x1B),
            KEY_LEFT => Some(0x1B),
            KEY_RIGHT => Some(0x1B),
            _ => {
                // Regular character: convert via scancode mapping
                let shift = modifier & 1 != 0;
                crate::device::input::keycode_to_ascii_caps(keycode, shift, false)
                    .map(|c| c as u8)
            }
        }
    }

    /// Handle mouse click (for selection).
    pub fn handle_click(&mut self, x: i32, _y: i32, _button: u8) {
        // Convert pixel coords to grid coords
        let col = ((x - self.client_rect.x) as u32) / 8;
        let _row = ((_y - self.client_rect.y) as u32) / 16;
        if col < self.cols {
            self.selection_start = Some((col, self.cursor_row));
            self.selection_end = None;
        }
    }

    /// Handle paste of text.
    pub fn handle_paste(&mut self, _text: &str) {
        for b in _text.bytes() {
            self.write(b);
        }
    }

    /// Scroll up/down by delta lines.
    pub fn scroll(&mut self, delta: i32) {
        if delta > 0 {
            self.scroll_up(delta as usize);
        } else {
            self.scroll_down((-delta) as usize);
        }
    }

    /// Resize terminal.
    pub fn resize(&mut self, cols: u32, rows: u32) {
        self.cols = core::cmp::min(cols, MAX_COLS);
        self.rows = core::cmp::min(rows, MAX_ROWS);
        self.cursor_row = self.cursor_row.min(self.rows - 1);
        self.cursor_col = self.cursor_col.min(self.cols - 1);
        self.dirty = true;
    }

    /// Copy selection to buffer.
    pub fn copy_selection(&self) -> Option<&[u8]> {
        // Placeholder — would iterate selected cells and build text
        None
    }

    /// Clear the terminal.
    pub fn clear(&mut self) {
        for row in 0..self.rows as usize {
            for col in 0..self.cols as usize {
                self.screen[row][col] = TerminalCell::new();
            }
        }
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.dirty = true;
    }
}
