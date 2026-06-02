// V39a — Desktop Shell
//
// Provides a full desktop experience with wallpaper, desktop icons,
// taskbar, start menu, system tray, and context menus.
// Integrates with the window manager, compositor, and theme engine.

use super::framebuffer::Framebuffer;
use super::graphics::{self, Color, Rect, Point};
use super::theme::{self, ColorRole};
use super::window::WindowManager;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum desktop icons.
const MAX_ICONS: usize = 64;

/// Maximum taskbar tabs.
const MAX_TABS: usize = 32;

/// Maximum start menu items.
const MAX_MENU_ITEMS: usize = 16;

/// Maximum tray icons.
const MAX_TRAY_ICONS: usize = 8;

/// Maximum context menu items.
const MAX_CONTEXT_ITEMS: usize = 12;

/// Icon grid layout constants.
const ICON_SIZE: u32 = 48;
const ICON_SPACING: u32 = 16;
const ICON_LABEL_HEIGHT: u32 = 20;
const ICON_TOTAL_HEIGHT: u32 = ICON_SIZE + ICON_LABEL_HEIGHT + 4;
const ICON_START_X: u32 = 24;
const ICON_START_Y: u32 = 24;
const ICONS_PER_ROW: u32 = 6;

/// Taskbar constants.
const TASKBAR_HEIGHT: u32 = 40;
const START_BUTTON_WIDTH: u32 = 48;
const CLOCK_WIDTH: u32 = 100;

/// Start menu constants.
const START_MENU_WIDTH: u32 = 280;
const START_MENU_HEIGHT: u32 = 360;

// ── Desktop Icon ──────────────────────────────────────────────────────────────

/// Type of desktop icon.
#[derive(Clone, Copy, PartialEq)]
pub enum IconType {
    Application,
    Folder,
    File,
    Trash,
}

/// Action triggered by clicking an icon.
#[derive(Clone, Copy)]
pub enum IconAction {
    None,
    Launch { path: [u8; 64] },
    OpenFolder { path: [u8; 64] },
    RunCommand { cmd: [u8; 128] },
}

impl IconAction {
    pub fn launch(name: &str) -> Self {
        let mut path = [0u8; 64];
        let len = core::cmp::min(name.len(), 63);
        for (i, b) in name.bytes().enumerate().take(len) {
            path[i] = b;
        }
        IconAction::Launch { path }
    }

    pub fn folder(path_str: &str) -> Self {
        let mut path = [0u8; 64];
        let len = core::cmp::min(path_str.len(), 63);
        for (i, b) in path_str.bytes().enumerate().take(len) {
            path[i] = b;
        }
        IconAction::OpenFolder { path }
    }

    pub fn command(cmd_str: &str) -> Self {
        let mut cmd = [0u8; 128];
        let len = core::cmp::min(cmd_str.len(), 127);
        for (i, b) in cmd_str.bytes().enumerate().take(len) {
            cmd[i] = b;
        }
        IconAction::RunCommand { cmd }
    }
}

/// A desktop icon (clickable shortcut).
#[derive(Clone, Copy)]
pub struct DesktopIcon {
    pub name: [u8; 32],
    pub name_len: usize,
    pub icon_type: IconType,
    pub rect: Rect,
    pub is_selected: bool,
    pub double_click_action: IconAction,
}

impl DesktopIcon {
    pub fn new(name: &str, icon_type: IconType, action: IconAction) -> Self {
        let mut name_buf = [0u8; 32];
        let nlen = core::cmp::min(name.len(), 31);
        for (i, b) in name.bytes().enumerate().take(nlen) {
            name_buf[i] = b;
        }
        DesktopIcon {
            name: name_buf,
            name_len: nlen,
            icon_type,
            rect: Rect::new(0, 0, ICON_SIZE, ICON_TOTAL_HEIGHT),
            is_selected: false,
            double_click_action: action,
        }
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }
}

// ── Taskbar ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct TaskbarTab {
    pub window_id: usize,
    pub title: [u8; 48],
    pub title_len: usize,
    pub is_active: bool,
    pub is_minimized: bool,
    pub rect: Rect,
}

impl TaskbarTab {
    pub fn new(window_id: usize, title: &str) -> Self {
        let mut title_buf = [0u8; 48];
        let tlen = core::cmp::min(title.len(), 47);
        for (i, b) in title.bytes().enumerate().take(tlen) {
            title_buf[i] = b;
        }
        TaskbarTab {
            window_id,
            title: title_buf,
            title_len: tlen,
            is_active: false,
            is_minimized: false,
            rect: Rect::new(0, 0, 0, 0),
        }
    }

    pub fn title_str(&self) -> &str {
        core::str::from_utf8(&self.title[..self.title_len]).unwrap_or("")
    }
}

pub struct Taskbar {
    pub rect: Rect,
    pub height: u32,
    pub color: Color,
    pub start_button: Rect,
    pub start_button_hovered: bool,
    pub window_tabs: [TaskbarTab; MAX_TABS],
    pub tab_count: usize,
    pub tray_area: Rect,
    pub clock_text: [u8; 16],
    pub clock_len: usize,
}

impl Taskbar {
    pub fn new(width: u32, height: u32) -> Self {
        Taskbar {
            rect: Rect::new(0, (height - TASKBAR_HEIGHT) as i32, width, TASKBAR_HEIGHT),
            height: TASKBAR_HEIGHT,
            color: 0xFF11111B,
            start_button: Rect::new(0, (height - TASKBAR_HEIGHT) as i32, START_BUTTON_WIDTH, TASKBAR_HEIGHT),
            start_button_hovered: false,
            window_tabs: {
                const EMPTY_TAB: TaskbarTab = TaskbarTab {
                    window_id: 0,
                    title: [0u8; 48],
                    title_len: 0,
                    is_active: false,
                    is_minimized: false,
                    rect: Rect::new(0, 0, 0, 0),
                };
                [EMPTY_TAB; MAX_TABS]
            },
            tab_count: 0,
            tray_area: Rect::new(
                (width - CLOCK_WIDTH - 8) as i32,
                (height - TASKBAR_HEIGHT) as i32,
                CLOCK_WIDTH + 8,
                TASKBAR_HEIGHT,
            ),
            clock_text: [0u8; 16],
            clock_len: 0,
        }
    }

    /// Update rect positions when screen resizes.
    pub fn update_layout(&mut self, width: u32, height: u32) {
        self.rect = Rect::new(0, (height - TASKBAR_HEIGHT) as i32, width, TASKBAR_HEIGHT);
        self.start_button = Rect::new(0, (height - TASKBAR_HEIGHT) as i32, START_BUTTON_WIDTH, TASKBAR_HEIGHT);
        self.tray_area = Rect::new(
            (width - CLOCK_WIDTH - 8) as i32,
            (height - TASKBAR_HEIGHT) as i32,
            CLOCK_WIDTH + 8,
            TASKBAR_HEIGHT,
        );
        // Re-layout window tabs
        self.layout_tabs();
    }

    /// Layout window tabs horizontally.
    fn layout_tabs(&mut self) {
        let tab_w: u32 = 120;
        let start_x = START_BUTTON_WIDTH + 4;
        for i in 0..self.tab_count {
            self.window_tabs[i].rect = Rect::new(
                (start_x + i as u32 * (tab_w + 2)) as i32,
                self.rect.y + 4,
                tab_w,
                self.rect.height - 8,
            );
        }
    }

    /// Hit-test the start button.
    pub fn hit_start_button(&self, x: i32, y: i32) -> bool {
        self.start_button.contains(&Point::new(x, y))
    }

    /// Hit-test a window tab.
    pub fn hit_tab(&self, x: i32, y: i32) -> Option<usize> {
        for i in 0..self.tab_count {
            if self.window_tabs[i].rect.contains(&Point::new(x, y)) {
                return Some(self.window_tabs[i].window_id);
            }
        }
        None
    }

    /// Hit-test the clock/tray area.
    pub fn hit_tray(&self, x: i32, y: i32) -> bool {
        self.tray_area.contains(&Point::new(x, y))
    }
}

// ── Start Menu ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct MenuItem {
    pub name: [u8; 48],
    pub name_len: usize,
    pub icon_type: IconType,
    pub action: IconAction,
    pub shortcut: [u8; 8],
    pub shortcut_len: usize,
    pub enabled: bool,
}

impl MenuItem {
    pub fn new(name: &str, icon_type: IconType, action: IconAction) -> Self {
        let mut name_buf = [0u8; 48];
        let nlen = core::cmp::min(name.len(), 47);
        for (i, b) in name.bytes().enumerate().take(nlen) {
            name_buf[i] = b;
        }
        MenuItem {
            name: name_buf,
            name_len: nlen,
            icon_type,
            action,
            shortcut: [0u8; 8],
            shortcut_len: 0,
            enabled: true,
        }
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }
}

pub struct StartMenu {
    pub is_open: bool,
    pub rect: Rect,
    pub items: [MenuItem; MAX_MENU_ITEMS],
    pub item_count: usize,
    pub search_text: [u8; 64],
    pub search_len: usize,
    pub selected_index: Option<usize>,
    pub power_button: Rect,
}

impl StartMenu {
    pub fn new(width: u32, height: u32) -> Self {
        let taskbar_y = (height - TASKBAR_HEIGHT) as i32;
        StartMenu {
            is_open: false,
            rect: Rect::new(0, taskbar_y - START_MENU_HEIGHT as i32, START_MENU_WIDTH, START_MENU_HEIGHT),
            items: {
                const EMPTY_MI: MenuItem = MenuItem {
                    name: [0u8; 48],
                    name_len: 0,
                    icon_type: IconType::Application,
                    action: IconAction::None,
                    shortcut: [0u8; 8],
                    shortcut_len: 0,
                    enabled: false,
                };
                [EMPTY_MI; MAX_MENU_ITEMS]
            },
            item_count: 0,
            search_text: [0u8; 64],
            search_len: 0,
            selected_index: None,
            power_button: Rect::new(
                START_MENU_WIDTH as i32 - 52,
                taskbar_y - 44,
                48,
                40,
            ),
        }
    }

    /// Add a menu item.
    pub fn add_item(&mut self, item: MenuItem) -> bool {
        if self.item_count < MAX_MENU_ITEMS {
            self.items[self.item_count] = item;
            self.item_count += 1;
            true
        } else {
            false
        }
    }

    /// Hit-test a menu item and return its index.
    pub fn hit_item(&self, x: i32, y: i32) -> Option<usize> {
        if !self.is_open { return None; }
        let item_start_y = self.rect.y + 48; // Below search bar
        for i in 0..self.item_count {
            let item_rect = Rect::new(
                self.rect.x + 4,
                item_start_y + i as i32 * 36,
                self.rect.width - 8,
                34,
            );
            if item_rect.contains(&Point::new(x, y)) {
                return Some(i);
            }
        }
        None
    }

    /// Hit-test the power button.
    pub fn hit_power(&self, x: i32, y: i32) -> bool {
        self.is_open && self.power_button.contains(&Point::new(x, y))
    }

    /// Toggle open/closed.
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
        if !self.is_open {
            self.selected_index = None;
        }
    }
}

// ── System Tray ───────────────────────────────────────────────────────────────

pub struct TrayIcon {
    pub name: [u8; 16],
    pub name_len: usize,
    pub icon_type: IconType,
    pub tooltip: [u8; 32],
    pub tooltip_len: usize,
    pub has_notification: bool,
}

impl TrayIcon {
    pub fn new(name: &str, icon_type: IconType) -> Self {
        let mut name_buf = [0u8; 16];
        let nlen = core::cmp::min(name.len(), 15);
        for (i, b) in name.bytes().enumerate().take(nlen) {
            name_buf[i] = b;
        }
        TrayIcon {
            name: name_buf,
            name_len: nlen,
            icon_type,
            tooltip: [0u8; 32],
            tooltip_len: 0,
            has_notification: false,
        }
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }
}

pub struct SystemTray {
    pub rect: Rect,
    pub icons: [TrayIcon; MAX_TRAY_ICONS],
    pub icon_count: usize,
}

impl SystemTray {
    pub fn new() -> Self {
        SystemTray {
            rect: Rect::new(0, 0, 0, 0),
            icons: {
                const EMPTY_TI: TrayIcon = TrayIcon {
                    name: [0u8; 16],
                    name_len: 0,
                    icon_type: IconType::Application,
                    tooltip: [0u8; 32],
                    tooltip_len: 0,
                    has_notification: false,
                };
                [EMPTY_TI; MAX_TRAY_ICONS]
            },
            icon_count: 0,
        }
    }

    pub fn add_icon(&mut self, icon: TrayIcon) -> bool {
        if self.icon_count < MAX_TRAY_ICONS {
            self.icons[self.icon_count] = icon;
            self.icon_count += 1;
            true
        } else {
            false
        }
    }
}

// ── Context Menu ──────────────────────────────────────────────────────────────

pub struct ContextMenuItem {
    pub label: [u8; 32],
    pub label_len: usize,
    pub enabled: bool,
    pub separator: bool,
    pub submenu: Option<usize>,
    pub action: IconAction,
}

impl ContextMenuItem {
    pub fn new(label: &str) -> Self {
        let mut label_buf = [0u8; 32];
        let llen = core::cmp::min(label.len(), 31);
        for (i, b) in label.bytes().enumerate().take(llen) {
            label_buf[i] = b;
        }
        ContextMenuItem {
            label: label_buf,
            label_len: llen,
            enabled: true,
            separator: false,
            submenu: None,
            action: IconAction::None,
        }
    }

    pub fn separator() -> Self {
        ContextMenuItem {
            label: [0u8; 32],
            label_len: 0,
            enabled: false,
            separator: true,
            submenu: None,
            action: IconAction::None,
        }
    }

    pub fn label_str(&self) -> &str {
        core::str::from_utf8(&self.label[..self.label_len]).unwrap_or("")
    }
}

pub struct ContextMenu {
    pub rect: Rect,
    pub items: [ContextMenuItem; MAX_CONTEXT_ITEMS],
    pub item_count: usize,
    pub visible: bool,
    pub hovered_index: Option<usize>,
}

impl ContextMenu {
    pub fn new() -> Self {
        ContextMenu {
            rect: Rect::new(0, 0, 180, 0),
            items: {
                const EMPTY_CI: ContextMenuItem = ContextMenuItem {
                    label: [0u8; 32],
                    label_len: 0,
                    enabled: false,
                    separator: false,
                    submenu: None,
                    action: IconAction::None,
                };
                [EMPTY_CI; MAX_CONTEXT_ITEMS]
            },
            item_count: 0,
            visible: false,
            hovered_index: None,
        }
    }

    pub fn show(&mut self, x: i32, y: i32) {
        // Calculate height based on items
        let h = self.item_count as u32 * 28 + 4;
        self.rect.x = x;
        self.rect.y = y;
        self.rect.height = h;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.hovered_index = None;
    }

    pub fn hit_item(&self, x: i32, y: i32) -> Option<usize> {
        if !self.visible { return None; }
        for i in 0..self.item_count {
            let item_rect = Rect::new(
                self.rect.x + 2,
                self.rect.y + 2 + i as i32 * 28,
                self.rect.width - 4,
                26,
            );
            if item_rect.contains(&Point::new(x, y)) {
                return Some(i);
            }
        }
        None
    }
}

// ── Wallpaper ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum WallpaperMode {
    SolidColor,
    Gradient,
    TiledBitmap,
    CenteredBitmap,
}

impl WallpaperMode {
    pub fn default() -> Self { WallpaperMode::Gradient }
}

pub struct Wallpaper {
    pub mode: WallpaperMode,
    pub color: Color,
    pub gradient_top: Color,
    pub gradient_bottom: Color,
    pub bitmap: [u32; 8192],
    pub bitmap_width: u32,
    pub bitmap_height: u32,
}

impl Wallpaper {
    pub fn new() -> Self {
        Wallpaper {
            mode: WallpaperMode::Gradient,
            color: 0xFF2E3440,
            gradient_top: 0xFF1E1E2E,
            gradient_bottom: 0xFF2E3440,
            bitmap: [0u32; 8192],
            bitmap_width: 0,
            bitmap_height: 0,
        }
    }

    pub fn solid(color: Color) -> Self {
        Wallpaper {
            mode: WallpaperMode::SolidColor,
            color,
            ..Wallpaper::new()
        }
    }

    pub fn gradient(top: Color, bottom: Color) -> Self {
        Wallpaper {
            mode: WallpaperMode::Gradient,
            gradient_top: top,
            gradient_bottom: bottom,
            ..Wallpaper::new()
        }
    }

    /// Render the wallpaper to the full framebuffer (or a region of it).
    pub fn render(&self, fb: &mut Framebuffer, width: u32, height: u32) {
        match self.mode {
            WallpaperMode::SolidColor => {
                fb.fill_rect(0, 0, width, height, self.color);
            }
            WallpaperMode::Gradient => {
                if height > 0 {
                    for y in 0..height {
                        let t = y as f32 / (height - 1) as f32;
                        let c = graphics::lerp_color(self.gradient_top, self.gradient_bottom, t);
                        fb.fill_rect(0, y, width, 1, c);
                    }
                }
            }
            WallpaperMode::TiledBitmap => {
                if self.bitmap_width > 0 && self.bitmap_height > 0 {
                    let bw = self.bitmap_width;
                    let bh = self.bitmap_height;
                    for ty in (0..height).step_by(bh as usize) {
                        for tx in (0..width).step_by(bw as usize) {
                            let copy_w = core::cmp::min(bw, width - tx);
                            let copy_h = core::cmp::min(bh, height - ty);
                            for row in 0..copy_h {
                                for col in 0..copy_w {
                                    let src_idx = (row * bw + col) as usize;
                                    if src_idx < 8192 {
                                        fb.put_pixel(tx + col, ty + row, self.bitmap[src_idx]);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            WallpaperMode::CenteredBitmap => {
                if self.bitmap_width > 0 && self.bitmap_height > 0 {
                    let bx = (width as i32 - self.bitmap_width as i32) / 2;
                    let by = (height as i32 - self.bitmap_height as i32) / 2;
                    for row in 0..self.bitmap_height {
                        for col in 0..self.bitmap_width {
                            let src_idx = (row * self.bitmap_width + col) as usize;
                            if src_idx < 8192 {
                                let px = bx + col as i32;
                                let py = by + row as i32;
                                if px >= 0 && py >= 0 {
                                    fb.put_pixel(px as u32, py as u32, self.bitmap[src_idx]);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Clock Format ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum ClockFormat {
    Time24h,
    Time12h,
    TimeDate,
}

// ── Desktop Events ────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub enum DesktopEvent {
    None,
    StartMenuToggled,
    IconLaunched(IconAction),
    PowerOff,
    Restart,
    ShowDesktop,
    OpenSettings,
    OpenFileManager,
    OpenTerminal,
}

// ── Desktop Shell ─────────────────────────────────────────────────────────────

/// The main desktop shell — manages wallpaper, icons, taskbar, and menus.
pub struct DesktopShell {
    pub icons: [DesktopIcon; MAX_ICONS],
    pub icon_count: usize,
    pub wallpaper: Wallpaper,
    pub taskbar: Taskbar,
    pub start_menu: StartMenu,
    pub system_tray: SystemTray,
    pub context_menu: ContextMenu,
    pub clock_format: ClockFormat,
    screen_width: u32,
    screen_height: u32,
}

impl DesktopShell {
    /// Create a new desktop shell for the given resolution.
    pub fn new(width: u32, height: u32) -> Self {
        DesktopShell {
            icons: {
                const EMPTY_DI: DesktopIcon = DesktopIcon {
                    name: [0u8; 32],
                    name_len: 0,
                    icon_type: IconType::Application,
                    rect: Rect::new(0, 0, 0, 0),
                    is_selected: false,
                    double_click_action: IconAction::None,
                };
                [EMPTY_DI; MAX_ICONS]
            },
            icon_count: 0,
            wallpaper: Wallpaper::gradient(0xFF1E1E2E, 0xFF2E3440),
            taskbar: Taskbar::new(width, height),
            start_menu: StartMenu::new(width, height),
            system_tray: SystemTray::new(),
            context_menu: ContextMenu::new(),
            clock_format: ClockFormat::Time24h,
            screen_width: width,
            screen_height: height,
        }
    }

    /// Render the complete desktop (wallpaper + icons + taskbar + start menu).
    pub fn render(&self, fb: &mut Framebuffer, wm: &WindowManager) {
        // 1. Wallpaper
        self.wallpaper.render(fb, self.screen_width, self.screen_height);

        // 2. Desktop icons
        self.render_icons(fb);

        // 3. Context menu (if visible)
        if self.context_menu.visible {
            self.render_context_menu(fb);
        }

        // 4. Window previews are handled by the window manager / compositor

        // 5. Taskbar (top layer)
        self.render_taskbar(fb);

        // 6. Start menu (top-most)
        if self.start_menu.is_open {
            self.render_start_menu(fb);
        }
    }

    /// Render desktop icons on the framebuffer.
    fn render_icons(&self, fb: &mut Framebuffer) {
        for i in 0..self.icon_count {
            let icon = &self.icons[i];
            let r = &icon.rect;

            // Selection highlight
            if icon.is_selected {
                fb.fill_rect(
                    (r.x - 2) as u32,
                    (r.y - 2) as u32,
                    r.width + 4,
                    r.height + 4,
                    graphics::rgba(137, 180, 250, 60), // Semi-transparent blue
                );
            }

            // Icon background (simplified colored box per type)
            let icon_color = match icon.icon_type {
                IconType::Application => 0xFF89B4FA,
                IconType::Folder => 0xFFFAB387,
                IconType::File => 0xFFA6E3A1,
                IconType::Trash => 0xFFF38BA8,
            };

            // Draw icon as rounded rectangle
            let icon_w = 32u32;
            let icon_h = 32u32;
            let icon_x = r.x + (ICON_SIZE as i32 - icon_w as i32) / 2;
            let icon_y = r.y + 4;
            fb.fill_rect(icon_x as u32, icon_y as u32, icon_w, icon_h, icon_color);

            // Icon symbol (simplified)
            let symbol_color = 0xFF1E1E2E;
            match icon.icon_type {
                IconType::Application => {
                    // Draw a simple app-like symbol
                    fb.fill_rect((icon_x + 8) as u32, (icon_y + 8) as u32, 16, 16, symbol_color);
                }
                IconType::Folder => {
                    // Draw a folder-like shape
                    fb.fill_rect((icon_x + 6) as u32, (icon_y + 10) as u32, 20, 14, symbol_color);
                    fb.fill_rect((icon_x + 8) as u32, (icon_y + 8) as u32, 12, 4, symbol_color);
                }
                IconType::File => {
                    // Document shape
                    fb.fill_rect((icon_x + 8) as u32, (icon_y + 6) as u32, 16, 20, symbol_color);
                }
                IconType::Trash => {
                    // Trash can
                    fb.fill_rect((icon_x + 10) as u32, (icon_y + 8) as u32, 12, 4, symbol_color);
                    fb.fill_rect((icon_x + 8) as u32, (icon_y + 12) as u32, 16, 14, symbol_color);
                }
            }

            // Icon label
            let font = graphics::font_8x16();
            let text = icon.name_str();
            let text_w = (text.len() as u32) * 8;
            let text_x = r.x + (ICON_SIZE as i32 - text_w as i32) / 2;
            let text_y = r.y + ICON_SIZE as i32 + 2;
            if text_x >= 0 {
                fb.draw_text(
                    text_x as u32,
                    text_y as u32,
                    text,
                    0xFFCDD6F4, // Light text
                    0x00000000,
                    font,
                );
            }
        }
    }

    /// Render the taskbar.
    fn render_taskbar(&self, fb: &mut Framebuffer) {
        let tb = &self.taskbar;

        // Taskbar background
        fb.fill_rect(
            tb.rect.x as u32, tb.rect.y as u32,
            tb.rect.width as u32, tb.rect.height as u32,
            tb.color,
        );

        // Top border line
        fb.fill_rect(
            tb.rect.x as u32, tb.rect.y as u32,
            tb.rect.width as u32, 1,
            graphics::rgba(69, 71, 90, 255), // Border color
        );

        // Start button
        let start_bg = if tb.start_button_hovered { 0xFF313244 } else { 0xFF1E1E2E };
        fb.fill_rect(
            tb.start_button.x as u32, tb.start_button.y as u32,
            tb.start_button.width as u32, tb.start_button.height as u32,
            start_bg,
        );

        // Start button "TrainOS" logo (simplified: colored dot)
        fb.fill_rect(
            (tb.start_button.x + 12) as u32,
            (tb.start_button.y + 12) as u32,
            24, 16,
            0xFF4A90D9,
        );

        // Window tabs
        for i in 0..tb.tab_count {
            let tab = &tb.window_tabs[i];
            let tab_bg = if tab.is_active {
                0xFF2D323C
            } else if tab.is_minimized {
                0xFF22262E
            } else {
                0xFF1A1D23
            };

            fb.fill_rect(
                tab.rect.x as u32, tab.rect.y as u32,
                tab.rect.width as u32, tab.rect.height as u32,
                tab_bg,
            );

            // Active indicator line
            if tab.is_active {
                fb.fill_rect(
                    tab.rect.x as u32, (tab.rect.y + tab.rect.height as i32 - 2) as u32,
                    tab.rect.width as u32, 2,
                    0xFF4A90D9,
                );
            }

            // Tab title (truncated)
            let title = tab.title_str();
            let font = graphics::font_8x16();
            let max_chars = (tab.rect.width / 8) as usize;
            let display = if title.len() > max_chars {
                &title[..max_chars.saturating_sub(1)]
            } else {
                title
            };
            fb.draw_text(
                (tab.rect.x + 4) as u32,
                (tab.rect.y + 6) as u32,
                display,
                0xFFE0E2E8,
                tab_bg,
                font,
            );
        }

        // Clock area
        let clock_bg = 0xFF1E1E2E;
        fb.fill_rect(
            tb.tray_area.x as u32, tb.tray_area.y as u32,
            tb.tray_area.width as u32, tb.tray_area.height as u32,
            clock_bg,
        );

        // Clock text
        let clock_str = core::str::from_utf8(&tb.clock_text[..tb.clock_len]).unwrap_or("00:00");
        let font = graphics::font_8x16();
        let clock_x = tb.tray_area.x + (tb.tray_area.width as i32 - (clock_str.len() as u32 * 8) as i32) / 2;
        fb.draw_text(
            clock_x.max(0) as u32,
            (tb.tray_area.y + (TASKBAR_HEIGHT as i32 - 16) / 2) as u32,
            clock_str,
            0xFFCDD6F4,
            clock_bg,
            font,
        );

        // System tray icons (right of clock)
        for i in 0..self.system_tray.icon_count {
            let icon = &self.system_tray.icons[i];
            let ix = tb.tray_area.x as u32 - (i as u32 + 1) * 24;
            let iy = tb.tray_area.y as u32 + 8;
            let tray_color = if icon.has_notification { 0xFFF38BA8 } else { 0xFF89B4FA };
            fb.fill_rect(ix, iy, 16, 16, tray_color);
        }
    }

    /// Render the start menu.
    fn render_start_menu(&self, fb: &mut Framebuffer) {
        let sm = &self.start_menu;

        // Menu background
        fb.fill_rect(
            sm.rect.x as u32, sm.rect.y as u32,
            sm.rect.width, sm.rect.height,
            0xFF1E1E2E,
        );

        // Menu border
        fb.fill_rect(
            sm.rect.x as u32, sm.rect.y as u32,
            sm.rect.width, 1,
            0xFF45475A,
        );
        fb.fill_rect(
            sm.rect.x as u32, (sm.rect.y + sm.rect.height as i32 - 1) as u32,
            sm.rect.width, 1,
            0xFF45475A,
        );
        fb.fill_rect(
            sm.rect.x as u32, sm.rect.y as u32,
            1, sm.rect.height,
            0xFF45475A,
        );
        fb.fill_rect(
            (sm.rect.x + sm.rect.width as i32 - 1) as u32, sm.rect.y as u32,
            1, sm.rect.height,
            0xFF45475A,
        );

        // Search bar
        fb.fill_rect(
            (sm.rect.x + 8) as u32,
            (sm.rect.y + 8) as u32,
            sm.rect.width - 16,
            28,
            0xFF313244,
        );

        // Search placeholder
        let font = graphics::font_8x16();
        fb.draw_text(
            (sm.rect.x + 12) as u32,
            (sm.rect.y + 14) as u32,
            "Search...",
            0xFF585B70,
            0x00000000,
            font,
        );

        // Menu items
        let item_start_y = sm.rect.y + 48;
        for i in 0..sm.item_count {
            let item = &sm.items[i];
            let item_y = item_start_y + i as i32 * 36;
            let item_bg = if sm.selected_index == Some(i) {
                0xFF313244
            } else {
                0xFF1E1E2E
            };

            fb.fill_rect(
                (sm.rect.x + 4) as u32,
                item_y as u32,
                sm.rect.width - 8,
                34,
                item_bg,
            );

            // Item icon
            let icon_color = match item.icon_type {
                IconType::Application => 0xFF89B4FA,
                IconType::Folder => 0xFFFAB387,
                _ => 0xFFA6E3A1,
            };
            fb.fill_rect(
                (sm.rect.x + 8) as u32,
                (item_y + 5) as u32,
                24, 24,
                icon_color,
            );

            // Item name
            fb.draw_text(
                (sm.rect.x + 40) as u32,
                (item_y + 9) as u32,
                item.name_str(),
                if item.enabled { 0xFFCDD6F4 } else { 0xFF585B70 },
                0x00000000,
                font,
            );
        }

        // Power button
        let power_bg = if sm.hit_power(sm.power_button.x, sm.power_button.y) {
            0xFF313244
        } else {
            0xFF1E1E2E
        };
        fb.fill_rect(
            sm.power_button.x as u32, sm.power_button.y as u32,
            sm.power_button.width, sm.power_button.height,
            power_bg,
        );
        // Power icon (red circle)
        fb.fill_rect(
            (sm.power_button.x + 14) as u32,
            (sm.power_button.y + 10) as u32,
            20, 20,
            0xFFF38BA8,
        );
    }

    /// Render the context menu.
    fn render_context_menu(&self, fb: &mut Framebuffer) {
        let cm = &self.context_menu;

        // Background
        fb.fill_rect(
            cm.rect.x as u32, cm.rect.y as u32,
            cm.rect.width, cm.rect.height,
            0xFF1E1E2E,
        );

        // Border
        let border_color = 0xFF45475A;
        fb.fill_rect(cm.rect.x as u32, cm.rect.y as u32, cm.rect.width, 1, border_color);
        fb.fill_rect(cm.rect.x as u32, (cm.rect.y + cm.rect.height as i32 - 1) as u32, cm.rect.width, 1, border_color);
        fb.fill_rect(cm.rect.x as u32, cm.rect.y as u32, 1, cm.rect.height, border_color);
        fb.fill_rect((cm.rect.x + cm.rect.width as i32 - 1) as u32, cm.rect.y as u32, 1, cm.rect.height, border_color);

        let font = graphics::font_8x16();
        for i in 0..cm.item_count {
            let item = &cm.items[i];
            let item_y = cm.rect.y + 2 + i as i32 * 28;

            if item.separator {
                // Horizontal separator
                fb.fill_rect(
                    (cm.rect.x + 8) as u32,
                    (item_y + 13) as u32,
                    cm.rect.width - 16,
                    1,
                    0xFF45475A,
                );
                continue;
            }

            let item_bg = if cm.hovered_index == Some(i) { 0xFF313244 } else { 0xFF1E1E2E };
            fb.fill_rect(
                (cm.rect.x + 2) as u32,
                item_y as u32,
                cm.rect.width - 4,
                26,
                item_bg,
            );

            fb.draw_text(
                (cm.rect.x + 12) as u32,
                (item_y + 5) as u32,
                item.label_str(),
                if item.enabled { 0xFFCDD6F4 } else { 0xFF585B70 },
                0x00000000,
                font,
            );
        }
    }

    // ── Event Handling ─────────────────────────────────────────────────────

    /// Handle a mouse click on desktop elements.
    /// Returns a DesktopEvent describing the action, if any.
    pub fn handle_click(&mut self, x: i32, y: i32, _button: u8) -> DesktopEvent {
        // Check start menu first (top-most)
        if self.start_menu.is_open {
            // Check power button
            if self.start_menu.hit_power(x, y) {
                self.start_menu.is_open = false;
                return DesktopEvent::PowerOff;
            }

            // Check menu items
            if let Some(idx) = self.start_menu.hit_item(x, y) {
                let action = &self.start_menu.items[idx].action;
                match action {
                    IconAction::None => {}
                    IconAction::Launch { .. } => {
                        self.start_menu.is_open = false;
                        return DesktopEvent::IconLaunched(*action);
                    }
                    IconAction::OpenFolder { .. } => {
                        self.start_menu.is_open = false;
                        return DesktopEvent::IconLaunched(*action);
                    }
                    IconAction::RunCommand { .. } => {
                        self.start_menu.is_open = false;
                        return DesktopEvent::IconLaunched(*action);
                    }
                }
            }

            // Click outside start menu closes it
            if !self.start_menu.rect.contains(&Point::new(x, y)) {
                self.start_menu.is_open = false;
            }
        }

        // Check context menu (if visible)
        if self.context_menu.visible {
            if let Some(idx) = self.context_menu.hit_item(x, y) {
                let action = self.context_menu.items[idx].action;
                self.context_menu.hide();
                return DesktopEvent::IconLaunched(action);
            }
            // Click outside closes
            if !self.context_menu.rect.contains(&Point::new(x, y)) {
                self.context_menu.hide();
            }
        }

        // Check taskbar
        if self.taskbar.hit_start_button(x, y) {
            self.start_menu.toggle();
            return DesktopEvent::StartMenuToggled;
        }

        // Check window tabs
        if let Some(_win_id) = self.taskbar.hit_tab(x, y) {
            // Focus/minimize handled by WM
            return DesktopEvent::None;
        }

        // Check desktop icons
        for i in 0..self.icon_count {
            if self.icons[i].rect.contains(&Point::new(x, y)) {
                // Select the icon
                for j in 0..self.icon_count {
                    self.icons[j].is_selected = j == i;
                }
                return DesktopEvent::None;
            }
        }

        // Deselect all icons on empty area click
        for i in 0..self.icon_count {
            self.icons[i].is_selected = false;
        }

        DesktopEvent::None
    }

    /// Handle double-click on desktop elements.
    pub fn handle_double_click(&mut self, x: i32, y: i32) -> DesktopEvent {
        for i in 0..self.icon_count {
            if self.icons[i].rect.contains(&Point::new(x, y)) {
                let action = self.icons[i].double_click_action;
                return DesktopEvent::IconLaunched(action);
            }
        }
        DesktopEvent::None
    }

    // ── Desktop Management ─────────────────────────────────────────────────

    /// Toggle the start menu.
    pub fn toggle_start_menu(&mut self) {
        self.start_menu.toggle();
    }

    /// Update the clock display from system time components.
    pub fn update_clock(&mut self) {
        // For now, set a placeholder clock string
        match self.clock_format {
            ClockFormat::Time24h => {
                let s = b"00:00";
                for i in 0..s.len() {
                    self.taskbar.clock_text[i] = s[i];
                }
                self.taskbar.clock_len = 5;
            }
            ClockFormat::Time12h => {
                let s = b"12:00 AM";
                for i in 0..s.len() {
                    self.taskbar.clock_text[i] = s[i];
                }
                self.taskbar.clock_len = 8;
            }
            ClockFormat::TimeDate => {
                let s = b"00:00 01/01";
                for i in 0..s.len() {
                    self.taskbar.clock_text[i] = s[i];
                }
                self.taskbar.clock_len = 11;
            }
        }
    }

    /// Update the clock with actual hour/minute values.
    pub fn set_clock_hour_minute(&mut self, hour: u8, minute: u8, day: u8, month: u8) {
        match self.clock_format {
            ClockFormat::Time24h => {
                self.taskbar.clock_text = [0u8; 16];
                self.taskbar.clock_text[0] = b'0' + (hour / 10);
                self.taskbar.clock_text[1] = b'0' + (hour % 10);
                self.taskbar.clock_text[2] = b':';
                self.taskbar.clock_text[3] = b'0' + (minute / 10);
                self.taskbar.clock_text[4] = b'0' + (minute % 10);
                self.taskbar.clock_len = 5;
            }
            ClockFormat::Time12h => {
                let pm = hour >= 12;
                let h12 = if hour == 0 { 12 } else if hour > 12 { hour - 12 } else { hour };
                self.taskbar.clock_text = [0u8; 16];
                self.taskbar.clock_text[0] = b'0' + (h12 / 10);
                self.taskbar.clock_text[1] = b'0' + (h12 % 10);
                self.taskbar.clock_text[2] = b':';
                self.taskbar.clock_text[3] = b'0' + (minute / 10);
                self.taskbar.clock_text[4] = b'0' + (minute % 10);
                self.taskbar.clock_text[5] = b' ';
                self.taskbar.clock_text[6] = if pm { b'P' } else { b'A' };
                self.taskbar.clock_text[7] = b'M';
                self.taskbar.clock_len = 8;
            }
            ClockFormat::TimeDate => {
                self.taskbar.clock_text = [0u8; 16];
                self.taskbar.clock_text[0] = b'0' + (hour / 10);
                self.taskbar.clock_text[1] = b'0' + (hour % 10);
                self.taskbar.clock_text[2] = b':';
                self.taskbar.clock_text[3] = b'0' + (minute / 10);
                self.taskbar.clock_text[4] = b'0' + (minute % 10);
                self.taskbar.clock_text[5] = b' ';
                self.taskbar.clock_text[6] = b'0' + (month / 10);
                self.taskbar.clock_text[7] = b'0' + (month % 10);
                self.taskbar.clock_text[8] = b'/';
                self.taskbar.clock_text[9] = b'0' + (day / 10);
                self.taskbar.clock_text[10] = b'0' + (day % 10);
                self.taskbar.clock_len = 11;
            }
        }
    }

    /// Add a desktop icon.
    pub fn add_icon(&mut self, name: &str, icon_type: IconType, action: IconAction) -> bool {
        if self.icon_count >= MAX_ICONS { return false; }
        let mut icon = DesktopIcon::new(name, icon_type, action);
        // Auto-position the icon in the grid
        let col = (self.icon_count as u32) % ICONS_PER_ROW;
        let row = (self.icon_count as u32) / ICONS_PER_ROW;
        icon.rect.x = (ICON_START_X + col * (ICON_SIZE + ICON_SPACING)) as i32;
        icon.rect.y = (ICON_START_Y + row * (ICON_TOTAL_HEIGHT + ICON_SPACING)) as i32;
        self.icons[self.icon_count] = icon;
        self.icon_count += 1;
        true
    }

    /// Auto-arrange icons in the grid layout.
    pub fn arrange_icons(&mut self) {
        for i in 0..self.icon_count {
            let col = (i as u32) % ICONS_PER_ROW;
            let row = (i as u32) / ICONS_PER_ROW;
            self.icons[i].rect.x = (ICON_START_X + col * (ICON_SIZE + ICON_SPACING)) as i32;
            self.icons[i].rect.y = (ICON_START_Y + row * (ICON_TOTAL_HEIGHT + ICON_SPACING)) as i32;
        }
    }

    /// Remove all icons.
    pub fn clear_icons(&mut self) {
        self.icon_count = 0;
    }

    /// Check if given point is on the taskbar.
    pub fn is_on_taskbar(&self, x: i32, y: i32) -> bool {
        self.taskbar.rect.contains(&Point::new(x, y))
    }

    // ── Taskbar Window Management ─────────────────────────────────────────

    /// Add a window tab to the taskbar.
    pub fn taskbar_add_window(&mut self, window_id: usize, title: &str) {
        if self.taskbar.tab_count >= MAX_TABS { return; }
        let mut tab = TaskbarTab::new(window_id, title);
        let tab_w = 120u32;
        let start_x = START_BUTTON_WIDTH + 4;
        tab.rect = Rect::new(
            (start_x + (self.taskbar.tab_count as u32) * (tab_w + 2)) as i32,
            self.taskbar.rect.y + 4,
            tab_w,
            self.taskbar.rect.height - 8,
        );
        self.taskbar.window_tabs[self.taskbar.tab_count] = tab;
        self.taskbar.tab_count += 1;
    }

    /// Remove a window from the taskbar.
    pub fn taskbar_remove_window(&mut self, window_id: usize) {
        let mut found = false;
        for i in 0..self.taskbar.tab_count {
            if self.taskbar.window_tabs[i].window_id == window_id {
                found = true;
            }
            if found && i + 1 < self.taskbar.tab_count {
                self.taskbar.window_tabs[i] = self.taskbar.window_tabs[i + 1];
            }
        }
        if found {
            self.taskbar.tab_count = self.taskbar.tab_count.saturating_sub(1);
        }
        self.taskbar.layout_tabs();
    }

    /// Set the active window in the taskbar.
    pub fn taskbar_set_active(&mut self, window_id: usize) {
        for i in 0..self.taskbar.tab_count {
            self.taskbar.window_tabs[i].is_active =
                self.taskbar.window_tabs[i].window_id == window_id;
        }
    }

    /// Update taskbar window title.
    pub fn taskbar_update_title(&mut self, window_id: usize, title: &str) {
        for i in 0..self.taskbar.tab_count {
            if self.taskbar.window_tabs[i].window_id == window_id {
                let tlen = core::cmp::min(title.len(), 47);
                for (j, b) in title.bytes().enumerate().take(tlen) {
                    self.taskbar.window_tabs[i].title[j] = b;
                }
                self.taskbar.window_tabs[i].title_len = tlen;
                break;
            }
        }
    }

    // ── Context Menu ──────────────────────────────────────────────────────

    /// Show the context menu at the given position.
    pub fn show_context_menu(&mut self, x: i32, y: i32) {
        // Populate with default items
        if self.context_menu.item_count == 0 {
            let mut item = ContextMenuItem::new("Open Terminal");
            item.action = IconAction::command("/bin/sh");
            self.context_menu.items[0] = item;

            self.context_menu.items[1] = ContextMenuItem::separator();

            let mut item2 = ContextMenuItem::new("Arrange Icons");
            item2.action = IconAction::command("arrange");
            self.context_menu.items[2] = item2;

            let mut item3 = ContextMenuItem::new("Change Wallpaper");
            item3.action = IconAction::command("wallpaper");
            self.context_menu.items[3] = item3;

            self.context_menu.items[4] = ContextMenuItem::separator();

            let mut item5 = ContextMenuItem::new("Settings");
            item5.action = IconAction::command("settings");
            self.context_menu.items[5] = item5;

            self.context_menu.item_count = 6;
        }
        self.context_menu.show(x, y);
    }

    /// Hide the context menu.
    pub fn hide_context_menu(&mut self) {
        self.context_menu.hide();
    }

    // ── Screen Resize ─────────────────────────────────────────────────────

    /// Handle screen resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_width = width;
        self.screen_height = height;
        self.taskbar.update_layout(width, height);
        self.start_menu = StartMenu::new(width, height);
    }

    // ── Accessors ─────────────────────────────────────────────────────────

    pub fn screen_width(&self) -> u32 { self.screen_width }
    pub fn screen_height(&self) -> u32 { self.screen_height }
    pub fn icon_count(&self) -> usize { self.icon_count }
}

// ── Global Desktop Shell Instance ─────────────────────────────────────────────

static mut DESKTOP_SHELL: Option<DesktopShell> = None;

/// Initialize the desktop shell.
pub fn desktop_init(width: u32, height: u32) {
    unsafe {
        if DESKTOP_SHELL.is_none() {
            let mut shell = DesktopShell::new(width, height);

            // Add default desktop icons
            shell.add_icon("Terminal", IconType::Application,
                IconAction::launch("/bin/sh"));
            shell.add_icon("File Manager", IconType::Folder,
                IconAction::folder("/home"));
            shell.add_icon("Settings", IconType::Application,
                IconAction::command("settings"));
            shell.add_icon("Trash", IconType::Trash, IconAction::None);

            // Add system tray icons
            shell.system_tray.add_icon(TrayIcon::new("Network", IconType::Application));
            shell.system_tray.add_icon(TrayIcon::new("Sound", IconType::Application));

            // Set initial clock
            shell.update_clock();

            DESKTOP_SHELL = Some(shell);

            crate::println!("  V39a: Desktop shell initialized ({}x{})", width, height);
        }
    }
}

/// Access the global desktop shell.
pub fn desktop_shell() -> Option<&'static mut DesktopShell> {
    unsafe { DESKTOP_SHELL.as_mut() }
}

/// Render the desktop (wallpaper + shell).
pub fn desktop_render(fb: &mut Framebuffer, wm: &WindowManager) {
    unsafe {
        if let Some(ref shell) = DESKTOP_SHELL {
            shell.render(fb, wm);
        }
    }
}
