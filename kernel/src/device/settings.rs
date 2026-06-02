// V39b — Desktop Settings Panel
//
// Provides a graphical settings panel for customizing the TrainOS desktop
// environment: theme, display, desktop behavior, and system information.

use super::framebuffer::Framebuffer;
use super::graphics::{
    self, draw_border, draw_text_centered, draw_text_wrapped,
    font_8x16, Color, DARK_GRAY, GRAY, LIGHT_GRAY, WHITE, Rect, BLACK,
};
use super::widgets::{
    self, CheckBoxWidget, DropdownMenu, DropdownItem, ListView, ListViewItem,
    Slider, SliderOrientation,
};

// ── Settings Pages ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingsPage {
    Appearance,
    Desktop,
    Display,
    Network,
    System,
    Keyboard,
    About,
}

/// Returns a human-readable label for a settings page.
pub fn settings_page_label(page: SettingsPage) -> &'static str {
    match page {
        SettingsPage::Appearance => "Appearance",
        SettingsPage::Desktop => "Desktop",
        SettingsPage::Display => "Display",
        SettingsPage::Network => "Network",
        SettingsPage::System => "System",
        SettingsPage::Keyboard => "Keyboard",
        SettingsPage::About => "About",
    }
}

// ── Settings Panel ───────────────────────────────────────────────────────────

/// Settings Panel application.
pub struct SettingsPanel {
    pub window_id: usize,
    /// Navigation sidebar (list of settings categories)
    pub sidebar: ListView,
    /// Current settings page
    pub current_page: SettingsPage,
    /// Theme settings
    pub theme_select: DropdownMenu,
    pub dark_mode_toggle: CheckBoxWidget,
    pub accent_color_select: DropdownMenu,
    /// Display settings
    pub wallpaper_select: DropdownMenu,
    pub resolution_select: DropdownMenu,
    pub font_size_slider: Slider,
    /// Desktop settings
    pub icon_size_slider: Slider,
    pub taskbar_position: DropdownMenu,
    pub clock_format_select: DropdownMenu,
    pub show_clock_toggle: CheckBoxWidget,
    /// Network settings
    pub hostname_box: DropdownMenu,  // Simplified as dropdown for display
    /// About section text
    pub about_text: [u8; 512],
    pub about_text_len: usize,
    /// Client area
    pub client_rect: Rect,
}

impl SettingsPanel {
    pub fn new(window_id: usize) -> Self {
        // Setup about text
        let mut about_buf = [0u8; 512];
        let about_str = "TrainOS Desktop\n\
                         Version: 39b\n\
                         Architecture: RISC-V 64-bit\n\
                         Kernel: Microkernel\n\
                         GUI: V37b (Framebuffer + Window Manager)\n\
                         Desktop: V39b (File Manager, Terminal, Monitor)\n\
                         \n\
                         (c) 2026 TrainOS Project\n\
                         MIT License";
        let alen = core::cmp::min(about_str.len(), 512);
        for (i, b) in about_str.bytes().enumerate().take(alen) {
            about_buf[i] = b;
        }

        // Theme dropdown
        let mut theme_dd = DropdownMenu::new(0, 0, 200);
        theme_dd.add_item(DropdownItem::new("TrainOS Dark"));
        theme_dd.add_item(DropdownItem::new("TrainOS Light"));
        theme_dd.add_item(DropdownItem::new("High Contrast"));
        theme_dd.add_item(DropdownItem::new("Solarized Dark"));
        theme_dd.add_item(DropdownItem::new("Solarized Light"));

        // Accent color dropdown
        let mut accent_dd = DropdownMenu::new(0, 0, 200);
        accent_dd.add_item(DropdownItem::new("Blue"));
        accent_dd.add_item(DropdownItem::new("Green"));
        accent_dd.add_item(DropdownItem::new("Red"));
        accent_dd.add_item(DropdownItem::new("Purple"));
        accent_dd.add_item(DropdownItem::new("Orange"));

        // Wallpaper dropdown
        let mut wall_dd = DropdownMenu::new(0, 0, 200);
        wall_dd.add_item(DropdownItem::new("Checkerboard"));
        wall_dd.add_item(DropdownItem::new("Solid Dark"));
        wall_dd.add_item(DropdownItem::new("Solid Light"));
        wall_dd.add_item(DropdownItem::new("Mountain"));
        wall_dd.add_item(DropdownItem::new("Abstract"));

        // Resolution dropdown
        let mut res_dd = DropdownMenu::new(0, 0, 200);
        res_dd.add_item(DropdownItem::new("1024x768"));
        res_dd.add_item(DropdownItem::new("1280x720"));
        res_dd.add_item(DropdownItem::new("1280x1024"));
        res_dd.add_item(DropdownItem::new("1920x1080"));

        // Taskbar position
        let mut taskbar_dd = DropdownMenu::new(0, 0, 200);
        taskbar_dd.add_item(DropdownItem::new("Bottom"));
        taskbar_dd.add_item(DropdownItem::new("Top"));
        taskbar_dd.add_item(DropdownItem::new("Left"));
        taskbar_dd.add_item(DropdownItem::new("Right"));

        // Clock format
        let mut clock_dd = DropdownMenu::new(0, 0, 200);
        clock_dd.add_item(DropdownItem::new("12-hour"));
        clock_dd.add_item(DropdownItem::new("24-hour"));

        // Hostname dropdown (simplified)
        let mut host_dd = DropdownMenu::new(0, 0, 200);
        host_dd.add_item(DropdownItem::new("trainos.local"));

        // Sidebar (settings categories)
        let mut sidebar = ListView::new(0, 0, 180, 400);
        sidebar.add_item("Appearance");
        sidebar.add_item("Desktop");
        sidebar.add_item("Display");
        sidebar.add_item("Network");
        sidebar.add_item("System");
        sidebar.add_item("Keyboard");
        sidebar.add_item("About");
        sidebar.selected_index = 0;

        SettingsPanel {
            window_id,
            sidebar,
            current_page: SettingsPage::Appearance,
            theme_select: theme_dd,
            dark_mode_toggle: CheckBoxWidget::new(0, 0, "Dark Mode"),
            accent_color_select: accent_dd,
            wallpaper_select: wall_dd,
            resolution_select: res_dd,
            font_size_slider: Slider::new(0, 0, 200, 24, SliderOrientation::Horizontal),
            icon_size_slider: Slider::new(0, 0, 200, 24, SliderOrientation::Horizontal),
            taskbar_position: taskbar_dd,
            clock_format_select: clock_dd,
            show_clock_toggle: CheckBoxWidget::new(0, 0, "Show Clock"),
            hostname_box: host_dd,
            about_text: about_buf,
            about_text_len: alen,
            client_rect: Rect::new(0, 0, 600, 480),
        }
    }

    /// Set client area rect.
    pub fn set_client_rect(&mut self, x: i32, y: i32, w: u32, h: u32) {
        self.client_rect = Rect::new(x, y, w, h);

        self.sidebar.rect = Rect::new(x + 10, y + 10, 180, h - 20);

        // Content area starts after sidebar
        let cx = x + 200;
        let cw = (w - 210).max(200);

        self.theme_select.rect = Rect::new(cx, y + 40, cw, 24);
        self.accent_color_select.rect = Rect::new(cx, y + 80, cw, 24);
        self.dark_mode_toggle.rect = Rect::new(cx, y + 120, 200, 20);

        self.wallpaper_select.rect = Rect::new(cx, y + 40, cw, 24);
        self.resolution_select.rect = Rect::new(cx, y + 80, cw, 24);
        self.font_size_slider.rect = Rect::new(cx, y + 120, cw, 24);

        self.icon_size_slider.rect = Rect::new(cx, y + 40, cw, 24);
        self.taskbar_position.rect = Rect::new(cx, y + 80, cw, 24);
        self.clock_format_select.rect = Rect::new(cx, y + 120, cw, 24);
        self.show_clock_toggle.rect = Rect::new(cx, y + 160, 200, 20);
    }

    /// Switch to a settings page.
    pub fn switch_page(&mut self, page: SettingsPage) {
        self.current_page = page;
    }

    /// Apply a theme.
    pub fn apply_theme(&mut self, _theme_index: usize) {
        // Would modify global theme colors
        // For now, just update dark mode toggle
        if _theme_index == 1 {
            self.dark_mode_toggle.is_checked = false; // Light theme
        } else {
            self.dark_mode_toggle.is_checked = true; // Dark theme
        }
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    /// Render the settings panel.
    pub fn render(&self, fb: &mut Framebuffer) {
        // Background
        fb.fill_rect(
            self.client_rect.x as u32, self.client_rect.y as u32,
            self.client_rect.width, self.client_rect.height,
            0xFFF0F0F0,
        );

        // Title
        draw_text_wrapped(fb,
            (self.client_rect.x + 10) as u32,
            (self.client_rect.y + 5) as u32,
            200, "Settings", 0xFF333333, 0xFFF0F0F0);

        // Sidebar
        let sidebar_bg = Rect::new(
            self.sidebar.rect.x, self.sidebar.rect.y,
            self.sidebar.rect.width, self.sidebar.rect.height,
        );
        fb.fill_rect(sidebar_bg.x as u32, sidebar_bg.y as u32,
            sidebar_bg.width, sidebar_bg.height, 0xFFFFFFFF);
        draw_border(fb, &sidebar_bg, 1, 0xFFCCCCCC);

        // Sidebar items (rendered manually for category style)
        let categories = [
            "Appearance",
            "Desktop",
            "Display",
            "Network",
            "System",
            "Keyboard",
            "About",
        ];
        let current_idx = self.current_page as usize;

        for (i, cat) in categories.iter().enumerate() {
            let iy = sidebar_bg.y + 4 + (i as u32 * 28) as i32;
            let item_rect = Rect::new(
                sidebar_bg.x + 2,
                iy,
                sidebar_bg.width - 4,
                26,
            );

            let bg = if i == current_idx { 0xFF4A90D9 } else { 0xFFFFFFFF };
            let fg = if i == current_idx { WHITE } else { 0xFF333333 };

            fb.fill_rect(item_rect.x as u32, item_rect.y as u32,
                item_rect.width, item_rect.height, bg);

            draw_text_wrapped(fb,
                (item_rect.x + 8) as u32, (item_rect.y + 5) as u32,
                item_rect.width - 16, cat, fg, bg);
        }

        // Content area
        let cx = self.client_rect.x + 200;
        let cw = (self.client_rect.width - 210).max(200);
        let cy = self.client_rect.y + 10;

        draw_text_wrapped(fb, cx as u32, cy as u32,
            cw, settings_page_label(self.current_page),
            0xFF333333, 0xFFF0F0F0);

        // Divider
        fb.fill_rect(cx as u32, (cy + 22) as u32, cw, 1, 0xFFCCCCCC);

        // Render page-specific content
        match self.current_page {
            SettingsPage::Appearance => {
                self.render_appearance_page(fb, cx, cy + 28, cw);
            }
            SettingsPage::Desktop => {
                self.render_desktop_page(fb, cx, cy + 28, cw);
            }
            SettingsPage::Display => {
                self.render_display_page(fb, cx, cy + 28, cw);
            }
            SettingsPage::Network => {
                self.render_network_page(fb, cx, cy + 28, cw);
            }
            SettingsPage::System => {
                self.render_system_page(fb, cx, cy + 28, cw);
            }
            SettingsPage::Keyboard => {
                self.render_keyboard_page(fb, cx, cy + 28, cw);
            }
            SettingsPage::About => {
                self.render_about_page(fb, cx, cy + 28, cw);
            }
        }
    }

    fn render_appearance_page(&self, fb: &mut Framebuffer, cx: i32, cy: i32, cw: u32) {
        let font = font_8x16();

        // Theme
        draw_text_wrapped(fb, cx as u32, cy as u32, 100, "Theme:", 0xFF333333, 0xFFF0F0F0);
        widgets::draw_widget(fb, &widgets::Widget::DropdownMenu(widgets::DropdownMenu {
            rect: self.theme_select.rect,
            items: self.theme_select.items,
            item_count: self.theme_select.item_count,
            is_open: self.theme_select.is_open,
            selected_index: self.theme_select.selected_index,
            max_visible: self.theme_select.max_visible,
            scroll_offset: self.theme_select.scroll_offset,
            bg_color: 0xFFFFFFFF,
            border_color: 0xFF888888,
            text_color: 0xFF000000,
            highlight_color: 0xFF4A90D9,
        }));

        // Accent Color
        draw_text_wrapped(fb, cx as u32, (cy + 36) as u32, 100, "Accent:", 0xFF333333, 0xFFF0F0F0);
        widgets::draw_widget(fb, &widgets::Widget::DropdownMenu(widgets::DropdownMenu {
            rect: self.accent_color_select.rect,
            items: self.accent_color_select.items,
            item_count: self.accent_color_select.item_count,
            is_open: self.accent_color_select.is_open,
            selected_index: self.accent_color_select.selected_index,
            max_visible: self.accent_color_select.max_visible,
            scroll_offset: self.accent_color_select.scroll_offset,
            bg_color: 0xFFFFFFFF,
            border_color: 0xFF888888,
            text_color: 0xFF000000,
            highlight_color: 0xFF4A90D9,
        }));

        // Dark Mode Toggle
        draw_text_wrapped(fb, cx as u32, (cy + 72) as u32, 100, "Dark Mode:", 0xFF333333, 0xFFF0F0F0);
        widgets::draw_widget(fb, &widgets::Widget::CheckBox(widgets::CheckBoxWidget {
            rect: self.dark_mode_toggle.rect,
            label: self.dark_mode_toggle.label,
            label_len: self.dark_mode_toggle.label_len,
            is_checked: self.dark_mode_toggle.is_checked,
            is_enabled: self.dark_mode_toggle.is_enabled,
        }));
    }

    fn render_desktop_page(&self, fb: &mut Framebuffer, cx: i32, cy: i32, cw: u32) {
        // Wallpaper
        draw_text_wrapped(fb, cx as u32, cy as u32, 100, "Wallpaper:", 0xFF333333, 0xFFF0F0F0);
        widgets::draw_widget(fb, &widgets::Widget::DropdownMenu(widgets::DropdownMenu {
            rect: self.wallpaper_select.rect,
            items: self.wallpaper_select.items,
            item_count: self.wallpaper_select.item_count,
            is_open: self.wallpaper_select.is_open,
            selected_index: self.wallpaper_select.selected_index,
            max_visible: self.wallpaper_select.max_visible,
            scroll_offset: self.wallpaper_select.scroll_offset,
            bg_color: 0xFFFFFFFF,
            border_color: 0xFF888888,
            text_color: 0xFF000000,
            highlight_color: 0xFF4A90D9,
        }));

        // Icon size
        draw_text_wrapped(fb, cx as u32, (cy + 36) as u32, 100, "Icon Size:", 0xFF333333, 0xFFF0F0F0);
        widgets::draw_widget(fb, &widgets::Widget::Slider(widgets::Slider {
            rect: self.icon_size_slider.rect,
            value: 0.5,
            min: 0.25,
            max: 1.0,
            step: 0.125,
            is_dragging: false,
            track_color: 0xFFE0E0E0,
            fill_color: 0xFF4A90D9,
            thumb_color: 0xFF5090E0,
            thumb_radius: 8,
            show_value: true,
            orientation: SliderOrientation::Horizontal,
        }));

        // Taskbar position
        draw_text_wrapped(fb, cx as u32, (cy + 72) as u32, 120, "Taskbar:", 0xFF333333, 0xFFF0F0F0);
        widgets::draw_widget(fb, &widgets::Widget::DropdownMenu(widgets::DropdownMenu {
            rect: self.taskbar_position.rect,
            items: self.taskbar_position.items,
            item_count: self.taskbar_position.item_count,
            is_open: self.taskbar_position.is_open,
            selected_index: self.taskbar_position.selected_index,
            max_visible: self.taskbar_position.max_visible,
            scroll_offset: self.taskbar_position.scroll_offset,
            bg_color: 0xFFFFFFFF,
            border_color: 0xFF888888,
            text_color: 0xFF000000,
            highlight_color: 0xFF4A90D9,
        }));

        // Clock format
        draw_text_wrapped(fb, cx as u32, (cy + 108) as u32, 120, "Clock Format:", 0xFF333333, 0xFFF0F0F0);
        widgets::draw_widget(fb, &widgets::Widget::DropdownMenu(widgets::DropdownMenu {
            rect: self.clock_format_select.rect,
            items: self.clock_format_select.items,
            item_count: self.clock_format_select.item_count,
            is_open: self.clock_format_select.is_open,
            selected_index: self.clock_format_select.selected_index,
            max_visible: self.clock_format_select.max_visible,
            scroll_offset: self.clock_format_select.scroll_offset,
            bg_color: 0xFFFFFFFF,
            border_color: 0xFF888888,
            text_color: 0xFF000000,
            highlight_color: 0xFF4A90D9,
        }));

        // Show clock toggle
        widgets::draw_widget(fb, &widgets::Widget::CheckBox(widgets::CheckBoxWidget {
            rect: self.show_clock_toggle.rect,
            label: self.show_clock_toggle.label,
            label_len: self.show_clock_toggle.label_len,
            is_checked: self.show_clock_toggle.is_checked,
            is_enabled: self.show_clock_toggle.is_enabled,
        }));
    }

    fn render_display_page(&self, fb: &mut Framebuffer, cx: i32, cy: i32, cw: u32) {
        // Resolution
        draw_text_wrapped(fb, cx as u32, cy as u32, 100, "Resolution:", 0xFF333333, 0xFFF0F0F0);
        widgets::draw_widget(fb, &widgets::Widget::DropdownMenu(widgets::DropdownMenu {
            rect: self.resolution_select.rect,
            items: self.resolution_select.items,
            item_count: self.resolution_select.item_count,
            is_open: self.resolution_select.is_open,
            selected_index: self.resolution_select.selected_index,
            max_visible: self.resolution_select.max_visible,
            scroll_offset: self.resolution_select.scroll_offset,
            bg_color: 0xFFFFFFFF,
            border_color: 0xFF888888,
            text_color: 0xFF000000,
            highlight_color: 0xFF4A90D9,
        }));

        // Font Size
        draw_text_wrapped(fb, cx as u32, (cy + 36) as u32, 100, "Font Size:", 0xFF333333, 0xFFF0F0F0);
        widgets::draw_widget(fb, &widgets::Widget::Slider(widgets::Slider {
            rect: self.font_size_slider.rect,
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
            orientation: SliderOrientation::Horizontal,
        }));
    }

    fn render_network_page(&self, fb: &mut Framebuffer, cx: i32, cy: i32, cw: u32) {
        // Hostname
        draw_text_wrapped(fb, cx as u32, cy as u32, 100, "Hostname:", 0xFF333333, 0xFFF0F0F0);
        widgets::draw_widget(fb, &widgets::Widget::DropdownMenu(widgets::DropdownMenu {
            rect: self.hostname_box.rect,
            items: self.hostname_box.items,
            item_count: self.hostname_box.item_count,
            is_open: self.hostname_box.is_open,
            selected_index: self.hostname_box.selected_index,
            max_visible: self.hostname_box.max_visible,
            scroll_offset: self.hostname_box.scroll_offset,
            bg_color: 0xFFFFFFFF,
            border_color: 0xFF888888,
            text_color: 0xFF000000,
            highlight_color: 0xFF4A90D9,
        }));

        let network_info = "Network Status:\n  Interface: virtio-net\n  MAC: 52:54:00:12:34:56\n  IP: 10.0.2.15/24\n  Gateway: 10.0.2.2\n  DNS: 10.0.2.3";
        let info_y = cy + 36;
        draw_text_wrapped(fb, cx as u32, info_y as u32,
            cw, network_info, 0xFF333333, 0xFFF0F0F0);
    }

    fn render_system_page(&self, fb: &mut Framebuffer, cx: i32, cy: i32, cw: u32) {
        let sys_info = "System Information\n\n\
                        Kernel: TrainOS 39b\n\
                        Architecture: RISC-V rv64gc\n\
                        CPU: 2 cores (QEMU virt)\n\
                        Memory: 512 MB\n\
                        Storage: 64 MB (VirtIO BLK)\n\
                        Runtime: RustSBI (M-mode)\n\
                        \n\
                        Date: 2026-06-02\n\
                        Uptime: varies\n\
                        Users: 1";
        draw_text_wrapped(fb, cx as u32, cy as u32,
            cw, sys_info, 0xFF333333, 0xFFF0F0F0);
    }

    fn render_keyboard_page(&self, fb: &mut Framebuffer, cx: i32, cy: i32, cw: u32) {
        let kb_info = "Keyboard Layout\n\n\
                       Current Layout: US English\n\
                       \n\
                       Available layouts:\n\
                       - US English (current)\n\
                       - UK English\n\
                       - German\n\
                       - French\n\
                       - Japanese\n\
                       \n\
                       Modifiers:\n\
                       Ctrl, Alt, Shift, Caps Lock\n\
                       \n\
                       Note: Layout switching is handled\n\
                       by the input subsystem.";
        draw_text_wrapped(fb, cx as u32, cy as u32,
            cw, kb_info, 0xFF333333, 0xFFF0F0F0);
    }

    fn render_about_page(&self, fb: &mut Framebuffer, cx: i32, cy: i32, _cw: u32) {
        let about_text = core::str::from_utf8(&self.about_text[..self.about_text_len])
            .unwrap_or("");

        // About section with a styled box
        let box_rect = Rect::new(cx, cy, 300, 200);
        fb.fill_rect(box_rect.x as u32, box_rect.y as u32,
            box_rect.width, box_rect.height, 0xFFFFFFFF);
        draw_border(fb, &box_rect, 1, 0xFFCCCCCC);

        draw_text_wrapped(fb,
            (box_rect.x + 10) as u32,
            (box_rect.y + 10) as u32,
            box_rect.width - 20,
            about_text, 0xFF333333, 0xFFFFFFFF);

        // TrainOS logo placeholder
        fb.fill_rect(
            (self.client_rect.right() - 80) as u32,
            (cy + 10) as u32,
            60, 60, 0xFF4A90D9,
        );
        draw_text_centered(fb,
            &Rect::new(self.client_rect.right() - 80, cy + 10, 60, 60),
            "TOS", WHITE, 0xFF4A90D9);
    }

    // ── Event Handling ─────────────────────────────────────────────────────

    /// Handle a mouse click.
    pub fn handle_click(&mut self, x: i32, y: i32, _button: u8) {
        // Check sidebar clicks
        let sidebar_items_start = self.sidebar.rect.y + 4;
        if x >= self.sidebar.rect.x + 2
            && x <= self.sidebar.rect.right() - 2
            && y >= sidebar_items_start
        {
            let rel_y = (y - sidebar_items_start) as u32;
            let item_idx = rel_y / 28;
            if item_idx < 7 {
                let pages = [
                    SettingsPage::Appearance,
                    SettingsPage::Desktop,
                    SettingsPage::Display,
                    SettingsPage::Network,
                    SettingsPage::System,
                    SettingsPage::Keyboard,
                    SettingsPage::About,
                ];
                self.switch_page(pages[item_idx as usize]);
                return;
            }
        }

        // Theme select click
        if self.theme_select.rect.contains(&graphics::Point::new(x, y)) {
            self.theme_select.is_open = !self.theme_select.is_open;
            return;
        }
        if self.theme_select.is_open {
            if let Some(idx) = self.theme_select.item_at_y(y) {
                self.theme_select.selected_index = idx;
                self.theme_select.is_open = false;
                self.apply_theme(idx);
                return;
            }
        }
    }
}
