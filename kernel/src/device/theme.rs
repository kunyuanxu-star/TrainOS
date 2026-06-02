// V39a — Theme Engine
//
// Provides a CSS-like theming system for the desktop environment.
// Defines color palettes, font settings, and spacing rules that
// are used by all widget and desktop rendering code.

use super::graphics::Color;

/// Copy a string into a fixed-size 32-byte array, zero-padded.
fn str_fixed32(s: &str) -> [u8; 32] {
    let mut buf = [0u8; 32];
    let len = core::cmp::min(s.len(), 31);
    for (i, b) in s.bytes().enumerate().take(len) {
        buf[i] = b;
    }
    buf
}

// ── Theme Colors ─────────────────────────────────────────────────────────────

/// A complete theme definition for the desktop environment.
pub struct Theme {
    pub name: [u8; 32],
    pub is_dark: bool,
    // Base colors
    pub bg_primary: Color,
    pub bg_secondary: Color,
    pub bg_tertiary: Color,
    pub fg_primary: Color,
    pub fg_secondary: Color,
    pub fg_disabled: Color,
    // Accent colors
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_pressed: Color,
    pub danger: Color,
    pub warning: Color,
    pub success: Color,
    pub info: Color,
    // Window chrome
    pub window_bg: Color,
    pub window_border: Color,
    pub titlebar_bg: Color,
    pub titlebar_fg: Color,
    pub titlebar_button_hover: Color,
    pub titlebar_button_close: Color,
    // Widget colors
    pub button_bg: Color,
    pub button_fg: Color,
    pub button_hover: Color,
    pub input_bg: Color,
    pub input_fg: Color,
    pub input_border: Color,
    pub input_focus_border: Color,
    // Taskbar
    pub taskbar_bg: Color,
    pub taskbar_fg: Color,
    pub taskbar_active: Color,
    // Font
    pub font_family: [u8; 32],
    pub font_size: u8,
    pub font_size_small: u8,
    pub font_size_large: u8,
    pub font_size_title: u8,
    // Borders
    pub border_radius: u32,
    pub border_width: u32,
    // Spacing
    pub padding_small: u32,
    pub padding_medium: u32,
    pub padding_large: u32,
}

impl Theme {
    /// Default dark theme (Catppuccin Mocha inspired).
    pub fn dark() -> Self {
        Theme {
            name: str_fixed32("Catppuccin Mocha Dark"),
            is_dark: true,
            bg_primary: 0xFF1E1E2E,
            bg_secondary: 0xFF181825,
            bg_tertiary: 0xFF313244,
            fg_primary: 0xFFCDD6F4,
            fg_secondary: 0xFFBAC2DE,
            fg_disabled: 0xFF585B70,
            accent: 0xFF89B4FA,
            accent_hover: 0xFFB4D0FB,
            accent_pressed: 0xFF74C7EC,
            danger: 0xFFF38BA8,
            warning: 0xFFFAB387,
            success: 0xFFA6E3A1,
            info: 0xFF89DCEB,
            window_bg: 0xFF1E1E2E,
            window_border: 0xFF45475A,
            titlebar_bg: 0xFF181825,
            titlebar_fg: 0xFFCDD6F4,
            titlebar_button_hover: 0xFF45475A,
            titlebar_button_close: 0xFFF38BA8,
            button_bg: 0xFF89B4FA,
            button_fg: 0xFF1E1E2E,
            button_hover: 0xFFB4D0FB,
            input_bg: 0xFF313244,
            input_fg: 0xFFCDD6F4,
            input_border: 0xFF45475A,
            input_focus_border: 0xFF89B4FA,
            taskbar_bg: 0xFF11111B,
            taskbar_fg: 0xFFCDD6F4,
            taskbar_active: 0xFF313244,
            font_family: str_fixed32("Sans"),
            font_size: 14,
            font_size_small: 11,
            font_size_large: 18,
            font_size_title: 24,
            border_radius: 6,
            border_width: 1,
            padding_small: 4,
            padding_medium: 8,
            padding_large: 16,
        }
    }

    /// Light theme (Solarized Light inspired).
    pub fn light() -> Self {
        Theme {
            name: str_fixed32("Solarized Light"),
            is_dark: false,
            bg_primary: 0xFFFDF6E3,
            bg_secondary: 0xFFEEE8D5,
            bg_tertiary: 0xFFE0D8C8,
            fg_primary: 0xFF073642,
            fg_secondary: 0xFF586E75,
            fg_disabled: 0xFF93A1A1,
            accent: 0xFF268BD2,
            accent_hover: 0xFF3388DD,
            accent_pressed: 0xFF1A6DA0,
            danger: 0xFFDC322F,
            warning: 0xFFCB4B16,
            success: 0xFF859900,
            info: 0xFF2AA198,
            window_bg: 0xFFFDF6E3,
            window_border: 0xFFD3CBB7,
            titlebar_bg: 0xFFEEE8D5,
            titlebar_fg: 0xFF073642,
            titlebar_button_hover: 0xFFD3CBB7,
            titlebar_button_close: 0xFFDC322F,
            button_bg: 0xFF268BD2,
            button_fg: 0xFFFDF6E3,
            button_hover: 0xFF3388DD,
            input_bg: 0xFFFDF6E3,
            input_fg: 0xFF073642,
            input_border: 0xFFD3CBB7,
            input_focus_border: 0xFF268BD2,
            taskbar_bg: 0xFFEEE8D5,
            taskbar_fg: 0xFF073642,
            taskbar_active: 0xFFD3CBB7,
            font_family: str_fixed32("Sans"),
            font_size: 14,
            font_size_small: 11,
            font_size_large: 18,
            font_size_title: 24,
            border_radius: 6,
            border_width: 1,
            padding_small: 4,
            padding_medium: 8,
            padding_large: 16,
        }
    }

    /// High-contrast theme for accessibility.
    pub fn high_contrast() -> Self {
        Theme {
            name: str_fixed32("High Contrast"),
            is_dark: true,
            bg_primary: 0xFF000000,
            bg_secondary: 0xFF1A1A1A,
            bg_tertiary: 0xFF333333,
            fg_primary: 0xFFFFFFFF,
            fg_secondary: 0xFFCCCCCC,
            fg_disabled: 0xFF666666,
            accent: 0xFFFFFF00,
            accent_hover: 0xFFFFFF66,
            accent_pressed: 0xFFFFCC00,
            danger: 0xFFFF0000,
            warning: 0xFFFF8800,
            success: 0xFF00FF00,
            info: 0xFF00CCFF,
            window_bg: 0xFF000000,
            window_border: 0xFFFFFFFF,
            titlebar_bg: 0xFF000000,
            titlebar_fg: 0xFFFFFFFF,
            titlebar_button_hover: 0xFF333333,
            titlebar_button_close: 0xFFFF0000,
            button_bg: 0xFFFFFF00,
            button_fg: 0xFF000000,
            button_hover: 0xFFFFFF66,
            input_bg: 0xFF000000,
            input_fg: 0xFFFFFFFF,
            input_border: 0xFFFFFFFF,
            input_focus_border: 0xFFFFFF00,
            taskbar_bg: 0xFF000000,
            taskbar_fg: 0xFFFFFFFF,
            taskbar_active: 0xFF333333,
            font_family: str_fixed32("Sans"),
            font_size: 16,
            font_size_small: 13,
            font_size_large: 20,
            font_size_title: 28,
            border_radius: 0,
            border_width: 2,
            padding_small: 6,
            padding_medium: 12,
            padding_large: 20,
        }
    }

    /// Solarized Dark theme.
    pub fn solarized_dark() -> Self {
        Theme {
            name: str_fixed32("Solarized Dark"),
            is_dark: true,
            bg_primary: 0xFF002B36,
            bg_secondary: 0xFF073642,
            bg_tertiary: 0xFF093E4A,
            fg_primary: 0xFF839496,
            fg_secondary: 0xFF657B83,
            fg_disabled: 0xFF586E75,
            accent: 0xFF268BD2,
            accent_hover: 0xFF3388DD,
            accent_pressed: 0xFF1A6DA0,
            danger: 0xFFDC322F,
            warning: 0xFFCB4B16,
            success: 0xFF859900,
            info: 0xFF2AA198,
            window_bg: 0xFF002B36,
            window_border: 0xFF073642,
            titlebar_bg: 0xFF073642,
            titlebar_fg: 0xFF93A1A1,
            titlebar_button_hover: 0xFF0A4A5A,
            titlebar_button_close: 0xFFDC322F,
            button_bg: 0xFF268BD2,
            button_fg: 0xFF002B36,
            button_hover: 0xFF3388DD,
            input_bg: 0xFF073642,
            input_fg: 0xFF839496,
            input_border: 0xFF586E75,
            input_focus_border: 0xFF268BD2,
            taskbar_bg: 0xFF002B36,
            taskbar_fg: 0xFF93A1A1,
            taskbar_active: 0xFF073642,
            font_family: str_fixed32("Sans"),
            font_size: 14,
            font_size_small: 11,
            font_size_large: 18,
            font_size_title: 24,
            border_radius: 4,
            border_width: 1,
            padding_small: 4,
            padding_medium: 8,
            padding_large: 16,
        }
    }

    /// Nord theme.
    pub fn nord() -> Self {
        Theme {
            name: str_fixed32("Nord"),
            is_dark: true,
            bg_primary: 0xFF2E3440,
            bg_secondary: 0xFF3B4252,
            bg_tertiary: 0xFF434C5E,
            fg_primary: 0xFFECEFF4,
            fg_secondary: 0xFFD8DEE9,
            fg_disabled: 0xFF4C566A,
            accent: 0xFF88C0D0,
            accent_hover: 0xFF8FBCBB,
            accent_pressed: 0xFF5E81AC,
            danger: 0xFFBF616A,
            warning: 0xFFD08770,
            success: 0xFFA3BE8C,
            info: 0xFF81A1C1,
            window_bg: 0xFF2E3440,
            window_border: 0xFF4C566A,
            titlebar_bg: 0xFF3B4252,
            titlebar_fg: 0xFFECEFF4,
            titlebar_button_hover: 0xFF434C5E,
            titlebar_button_close: 0xFFBF616A,
            button_bg: 0xFF88C0D0,
            button_fg: 0xFF2E3440,
            button_hover: 0xFF8FBCBB,
            input_bg: 0xFF3B4252,
            input_fg: 0xFFECEFF4,
            input_border: 0xFF4C566A,
            input_focus_border: 0xFF88C0D0,
            taskbar_bg: 0xFF2E3440,
            taskbar_fg: 0xFFD8DEE9,
            taskbar_active: 0xFF434C5E,
            font_family: str_fixed32("Sans"),
            font_size: 14,
            font_size_small: 11,
            font_size_large: 18,
            font_size_title: 24,
            border_radius: 6,
            border_width: 1,
            padding_small: 4,
            padding_medium: 8,
            padding_large: 16,
        }
    }

    /// Catppuccin Mocha (rich purple/pink variant).
    pub fn catppuccin() -> Self {
        Theme {
            name: str_fixed32("Catppuccin"),
            is_dark: true,
            bg_primary: 0xFF1E1E2E,
            bg_secondary: 0xFF181825,
            bg_tertiary: 0xFF313244,
            fg_primary: 0xFFCDD6F4,
            fg_secondary: 0xFFBAC2DE,
            fg_disabled: 0xFF585B70,
            accent: 0xFFCBA6F7,  // Mauve
            accent_hover: 0xFFDDB9FF,
            accent_pressed: 0xFFB4BEFE,
            danger: 0xFFF38BA8,
            warning: 0xFFFAB387,
            success: 0xFFA6E3A1,
            info: 0xFF89DCEB,
            window_bg: 0xFF1E1E2E,
            window_border: 0xFF45475A,
            titlebar_bg: 0xFF181825,
            titlebar_fg: 0xFFCDD6F4,
            titlebar_button_hover: 0xFF45475A,
            titlebar_button_close: 0xFFF38BA8,
            button_bg: 0xFFCBA6F7,
            button_fg: 0xFF1E1E2E,
            button_hover: 0xFFDDB9FF,
            input_bg: 0xFF313244,
            input_fg: 0xFFCDD6F4,
            input_border: 0xFF45475A,
            input_focus_border: 0xFFCBA6F7,
            taskbar_bg: 0xFF11111B,
            taskbar_fg: 0xFFCDD6F4,
            taskbar_active: 0xFF313244,
            font_family: str_fixed32("Sans"),
            font_size: 14,
            font_size_small: 11,
            font_size_large: 18,
            font_size_title: 24,
            border_radius: 8,
            border_width: 1,
            padding_small: 4,
            padding_medium: 8,
            padding_large: 16,
        }
    }

    /// TrainOS brand theme (blue/orange industrial).
    pub fn trainos() -> Self {
        Theme {
            name: str_fixed32("TrainOS"),
            is_dark: true,
            bg_primary: 0xFF1A1D23,
            bg_secondary: 0xFF22262E,
            bg_tertiary: 0xFF2D323C,
            fg_primary: 0xFFE0E2E8,
            fg_secondary: 0xFFB0B6C4,
            fg_disabled: 0xFF5A6070,
            accent: 0xFF4A90D9,  // TrainOS blue
            accent_hover: 0xFF5BA0E9,
            accent_pressed: 0xFF3A80C9,
            danger: 0xFFE05656,
            warning: 0xFFE8A040,
            success: 0xFF56C87A,
            info: 0xFF56B8E0,
            window_bg: 0xFF22262E,
            window_border: 0xFF383D48,
            titlebar_bg: 0xFF1A1D23,
            titlebar_fg: 0xFFE0E2E8,
            titlebar_button_hover: 0xFF2D323C,
            titlebar_button_close: 0xFFE05656,
            button_bg: 0xFF4A90D9,
            button_fg: 0xFFFFFFFF,
            button_hover: 0xFF5BA0E9,
            input_bg: 0xFF2D323C,
            input_fg: 0xFFE0E2E8,
            input_border: 0xFF383D48,
            input_focus_border: 0xFF4A90D9,
            taskbar_bg: 0xFF14171C,
            taskbar_fg: 0xFFE0E2E8,
            taskbar_active: 0xFF2D323C,
            font_family: str_fixed32("Sans"),
            font_size: 14,
            font_size_small: 11,
            font_size_large: 18,
            font_size_title: 24,
            border_radius: 4,
            border_width: 1,
            padding_small: 4,
            padding_medium: 8,
            padding_large: 16,
        }
    }
}

// ── Color Role ────────────────────────────────────────────────────────────────

/// Semantic roles for theme color lookups.
pub enum ColorRole {
    BgPrimary,
    BgSecondary,
    FgPrimary,
    Accent,
    Danger,
    Success,
    Warning,
    Info,
    BgTertiary,
    FgSecondary,
    FgDisabled,
    WindowBg,
    WindowBorder,
    TitlebarBg,
    TitlebarFg,
    ButtonBg,
    ButtonFg,
    InputBg,
    InputFg,
    TaskbarBg,
    TaskbarFg,
}

// ── Theme Manager ─────────────────────────────────────────────────────────────

/// Maximum number of available themes.
pub const MAX_THEMES: usize = 8;

/// Manages the current theme and provides themed color lookups.
pub struct ThemeManager {
    pub current: Theme,
    pub available: [Theme; MAX_THEMES],
    pub available_count: usize,
}

impl ThemeManager {
    /// Create a new theme manager with default themes loaded.
    pub fn new() -> Self {
        let mut available: [Theme; MAX_THEMES] = [
            Theme::dark(),
            Theme::light(),
            Theme::high_contrast(),
            Theme::solarized_dark(),
            Theme::nord(),
            Theme::catppuccin(),
            Theme::trainos(),
            // Slot 7 reserved — fill with a copy of dark
            Theme::dark(),
        ];

        ThemeManager {
            current: Theme::dark(),
            available,
            available_count: 7,
        }
    }

    /// Set the current theme by index (0 .. available_count).
    pub fn set_theme(&mut self, index: usize) -> bool {
        if index < self.available_count {
            // Copy theme data (no Clone on Theme, so manual copy)
            self.current = Self::copy_theme(&self.available[index]);
            true
        } else {
            false
        }
    }

    /// Toggle between dark and light themes.
    pub fn toggle_dark_light(&mut self) {
        let target_name = if self.current.is_dark { "Solarized Light" } else { "Catppuccin Mocha Dark" };
        for i in 0..self.available_count {
            let name_str = core::str::from_utf8(&self.available[i].name)
                .unwrap_or("");
            if let Some(full_name) = name_str.split('\0').next() {
                if full_name == target_name {
                    self.set_theme(i);
                    return;
                }
            }
        }
        // Fallback: find any dark/light
        for i in 0..self.available_count {
            if self.available[i].is_dark != self.current.is_dark {
                self.set_theme(i);
                return;
            }
        }
    }

    /// Get a color by semantic role.
    pub fn get_color(&self, role: ColorRole) -> Color {
        match role {
            ColorRole::BgPrimary => self.current.bg_primary,
            ColorRole::BgSecondary => self.current.bg_secondary,
            ColorRole::BgTertiary => self.current.bg_tertiary,
            ColorRole::FgPrimary => self.current.fg_primary,
            ColorRole::FgSecondary => self.current.fg_secondary,
            ColorRole::FgDisabled => self.current.fg_disabled,
            ColorRole::Accent => self.current.accent,
            ColorRole::Danger => self.current.danger,
            ColorRole::Success => self.current.success,
            ColorRole::Warning => self.current.warning,
            ColorRole::Info => self.current.info,
            ColorRole::WindowBg => self.current.window_bg,
            ColorRole::WindowBorder => self.current.window_border,
            ColorRole::TitlebarBg => self.current.titlebar_bg,
            ColorRole::TitlebarFg => self.current.titlebar_fg,
            ColorRole::ButtonBg => self.current.button_bg,
            ColorRole::ButtonFg => self.current.button_fg,
            ColorRole::InputBg => self.current.input_bg,
            ColorRole::InputFg => self.current.input_fg,
            ColorRole::TaskbarBg => self.current.taskbar_bg,
            ColorRole::TaskbarFg => self.current.taskbar_fg,
        }
    }

    /// Copy a theme (field-by-field, since Theme doesn't derive Clone).
    fn copy_theme(src: &Theme) -> Theme {
        let mut name = [0u8; 32];
        name.copy_from_slice(&src.name);
        let mut ff = [0u8; 32];
        ff.copy_from_slice(&src.font_family);
        Theme {
            name,
            is_dark: src.is_dark,
            bg_primary: src.bg_primary,
            bg_secondary: src.bg_secondary,
            bg_tertiary: src.bg_tertiary,
            fg_primary: src.fg_primary,
            fg_secondary: src.fg_secondary,
            fg_disabled: src.fg_disabled,
            accent: src.accent,
            accent_hover: src.accent_hover,
            accent_pressed: src.accent_pressed,
            danger: src.danger,
            warning: src.warning,
            success: src.success,
            info: src.info,
            window_bg: src.window_bg,
            window_border: src.window_border,
            titlebar_bg: src.titlebar_bg,
            titlebar_fg: src.titlebar_fg,
            titlebar_button_hover: src.titlebar_button_hover,
            titlebar_button_close: src.titlebar_button_close,
            button_bg: src.button_bg,
            button_fg: src.button_fg,
            button_hover: src.button_hover,
            input_bg: src.input_bg,
            input_fg: src.input_fg,
            input_border: src.input_border,
            input_focus_border: src.input_focus_border,
            taskbar_bg: src.taskbar_bg,
            taskbar_fg: src.taskbar_fg,
            taskbar_active: src.taskbar_active,
            font_family: ff,
            font_size: src.font_size,
            font_size_small: src.font_size_small,
            font_size_large: src.font_size_large,
            font_size_title: src.font_size_title,
            border_radius: src.border_radius,
            border_width: src.border_width,
            padding_small: src.padding_small,
            padding_medium: src.padding_medium,
            padding_large: src.padding_large,
        }
    }

    /// Get the name of the current theme as a string slice.
    pub fn theme_name(&self) -> &str {
        let name_str = core::str::from_utf8(&self.current.name).unwrap_or("Unknown");
        name_str.split('\0').next().unwrap_or("Unknown")
    }
}

// ── Global Theme Manager Instance ─────────────────────────────────────────────

static mut THEME_MANAGER: Option<ThemeManager> = None;

/// Initialize the global theme manager.
pub fn theme_init() {
    unsafe {
        if THEME_MANAGER.is_none() {
            THEME_MANAGER = Some(ThemeManager::new());
            let name = THEME_MANAGER.as_ref().unwrap().theme_name();
            crate::println!("  V39a: Theme engine loaded (default: {})", name);
        }
    }
}

/// Access the global theme manager.
pub fn theme_manager() -> Option<&'static mut ThemeManager> {
    unsafe { THEME_MANAGER.as_mut() }
}

/// Get the current theme.
pub fn current_theme() -> &'static Theme {
    unsafe {
        match THEME_MANAGER.as_ref() {
            Some(tm) => &tm.current,
            None => {
                // Fallback static theme
                static FALLBACK: Theme = Theme {
                    name: [0u8; 32],
                    is_dark: true,
                    bg_primary: 0xFF1E1E2E,
                    bg_secondary: 0xFF181825,
                    bg_tertiary: 0xFF313244,
                    fg_primary: 0xFFCDD6F4,
                    fg_secondary: 0xFFBAC2DE,
                    fg_disabled: 0xFF585B70,
                    accent: 0xFF89B4FA,
                    accent_hover: 0xFFB4D0FB,
                    accent_pressed: 0xFF74C7EC,
                    danger: 0xFFF38BA8,
                    warning: 0xFFFAB387,
                    success: 0xFFA6E3A1,
                    info: 0xFF89DCEB,
                    window_bg: 0xFF1E1E2E,
                    window_border: 0xFF45475A,
                    titlebar_bg: 0xFF181825,
                    titlebar_fg: 0xFFCDD6F4,
                    titlebar_button_hover: 0xFF45475A,
                    titlebar_button_close: 0xFFF38BA8,
                    button_bg: 0xFF89B4FA,
                    button_fg: 0xFF1E1E2E,
                    button_hover: 0xFFB4D0FB,
                    input_bg: 0xFF313244,
                    input_fg: 0xFFCDD6F4,
                    input_border: 0xFF45475A,
                    input_focus_border: 0xFF89B4FA,
                    taskbar_bg: 0xFF11111B,
                    taskbar_fg: 0xFFCDD6F4,
                    taskbar_active: 0xFF313244,
                    font_family: [0u8; 32],
                    font_size: 14,
                    font_size_small: 11,
                    font_size_large: 18,
                    font_size_title: 24,
                    border_radius: 6,
                    border_width: 1,
                    padding_small: 4,
                    padding_medium: 8,
                    padding_large: 16,
                };
                &FALLBACK
            }
        }
    }
}

/// Get color from the current theme by role.
pub fn theme_color(role: ColorRole) -> Color {
    match theme_manager() {
        Some(tm) => tm.get_color(role),
        None => {
            // Fallback colors
            match role {
                ColorRole::BgPrimary => 0xFF1E1E2E,
                ColorRole::FgPrimary => 0xFFCDD6F4,
                ColorRole::Accent => 0xFF89B4FA,
                ColorRole::Danger => 0xFFF38BA8,
                _ => 0xFFFFFFFF,
            }
        }
    }
}
