// V39b — Desktop File Manager Application
//
// Provides a graphical file manager with navigation, file operations,
// and context menus for the TrainOS desktop environment.

use super::framebuffer::Framebuffer;
use super::graphics::{
    self, draw_border, draw_text_centered, draw_text_wrapped,
    font_8x16, Color, DARK_GRAY, GRAY, LIGHT_GRAY, WHITE, Rect,
};
use super::widgets::{
    ButtonWidget, CheckBoxWidget, IconType, ListView, ListViewItem,
    StatusBar, StatusBarSection, TextBoxWidget, TreeView, TreeNode,
};

// ── Context Menu ─────────────────────────────────────────────────────────────

/// A right-click context menu with items.
pub struct ContextMenu {
    pub rect: Rect,
    pub items: [ContextMenuItem; 12],
    pub item_count: usize,
    pub visible: bool,
    pub selected_index: isize,
}

#[derive(Clone, Copy)]
pub struct ContextMenuItem {
    pub label: [u8; 32],
    pub label_len: usize,
    pub enabled: bool,
    pub action_id: u32,
}

impl ContextMenuItem {
    pub const fn new_empty() -> Self {
        ContextMenuItem {
            label: [0u8; 32],
            label_len: 0,
            enabled: false,
            action_id: 0,
        }
    }

    pub fn new(label: &str, action: u32) -> Self {
        let mut label_buf = [0u8; 32];
        let llen = core::cmp::min(label.len(), 32);
        for (i, b) in label.bytes().enumerate().take(llen) {
            label_buf[i] = b;
        }
        ContextMenuItem {
            label: label_buf,
            label_len: llen,
            enabled: true,
            action_id: action,
        }
    }

    pub fn label_str(&self) -> &str {
        core::str::from_utf8(&self.label[..self.label_len]).unwrap_or("")
    }
}

impl ContextMenu {
    pub fn new() -> Self {
        ContextMenu {
            rect: Rect::new(0, 0, 160, 0),
            items: [ContextMenuItem::new_empty(); 12],
            item_count: 0,
            visible: false,
            selected_index: -1,
        }
    }

    pub fn add_item(&mut self, label: &str, action: u32) -> bool {
        if self.item_count < 12 {
            self.items[self.item_count] = ContextMenuItem::new(label, action);
            self.item_count += 1;
            self.rect.height = self.item_count as u32 * 22;
            true
        } else {
            false
        }
    }

    pub fn show(&mut self, x: i32, y: i32) {
        self.rect.x = x;
        self.rect.y = y;
        self.rect.width = 160;
        self.rect.height = self.item_count as u32 * 22;
        self.visible = true;
        self.selected_index = -1;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Which item index is at a given y coordinate.
    pub fn item_at_y(&self, y: i32) -> Option<usize> {
        if !self.visible { return None; }
        let rel = y - self.rect.y;
        if rel < 0 { return None; }
        let idx = (rel / 22) as usize;
        if idx < self.item_count { Some(idx) } else { None }
    }

    pub fn render(&self, fb: &mut Framebuffer) {
        if !self.visible { return; }

        // Shadow
        let shadow = Rect::new(self.rect.x + 2, self.rect.y + 2, self.rect.width, self.rect.height);
        graphics::draw_shadow(fb, &shadow, 4, 30);

        // Background
        fb.fill_rect(self.rect.x as u32, self.rect.y as u32,
            self.rect.width, self.rect.height, 0xFFF8F8F8);
        draw_border(fb, &self.rect, 1, 0xFF888888);

        for i in 0..self.item_count {
            let item = &self.items[i];
            let iy = self.rect.y + (i as u32 * 22) as i32;

            if i == self.selected_index as usize {
                fb.fill_rect(self.rect.x as u32, iy as u32,
                    self.rect.width, 22, 0xFF4A90D9);
            }

            let iy_off = iy + 3;
            let color = if i == self.selected_index as usize {
                WHITE
            } else if item.enabled {
                0xFF000000
            } else {
                0xFF888888
            };
            let bg = if i == self.selected_index as usize {
                0xFF4A90D9
            } else {
                0xFFF8F8F8
            };

            draw_text_wrapped(fb,
                (self.rect.x + 8) as u32, iy_off as u32,
                self.rect.width - 16,
                item.label_str(), color, bg);

            // Separator line between items
            // (subtle visual dividing line)
            fb.fill_rect(
                (self.rect.x + 4) as u32,
                (iy + 22 - 1) as u32,
                self.rect.width - 8,
                1,
                0xFFE0E0E0,
            );
        }
    }
}

// ── File View Mode ───────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FileViewMode {
    Icons,
    List,
    Compact,
}

// ── File System Entry ────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct FsEntry {
    pub name: [u8; 64],
    pub name_len: usize,
    pub is_directory: bool,
    pub size: u64,
    pub modified_time: u64,
    pub permissions: u16,
}

impl FsEntry {
    pub fn new() -> Self {
        FsEntry {
            name: [0u8; 64],
            name_len: 0,
            is_directory: false,
            size: 0,
            modified_time: 0,
            permissions: 0,
        }
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }
}

// ── File Manager Toolbar ─────────────────────────────────────────────────────

pub struct FileManagerToolbar {
    pub rect: Rect,
    pub back_button: ButtonWidget,
    pub forward_button: ButtonWidget,
    pub up_button: ButtonWidget,
    pub refresh_button: ButtonWidget,
    pub home_button: ButtonWidget,
    pub search_button: ButtonWidget,
    pub new_folder_button: ButtonWidget,
    pub view_mode_button: ButtonWidget,
}

impl FileManagerToolbar {
    pub fn new(x: i32, y: i32, w: u32) -> Self {
        let btn_h = 28;
        FileManagerToolbar {
            rect: Rect::new(x, y, w, 36),
            back_button: ButtonWidget::new(x, y, 28, btn_h, "<"),
            forward_button: ButtonWidget::new(x + 30, y, 28, btn_h, ">"),
            up_button: ButtonWidget::new(x + 60, y, 28, btn_h, "^"),
            refresh_button: ButtonWidget::new(x + 90, y, 28, btn_h, "R"),
            home_button: ButtonWidget::new(x + 120, y, 28, btn_h, "H"),
            search_button: ButtonWidget::new(x + 150, y, 28, btn_h, "?"),
            new_folder_button: ButtonWidget::new(x + 180, y, 28, btn_h, "+"),
            view_mode_button: ButtonWidget::new((x + w as i32) - 30, y, 28, btn_h, "V"),
        }
    }

    pub fn render(&self, fb: &mut Framebuffer) {
        // Toolbar background
        fb.fill_rect(self.rect.x as u32, self.rect.y as u32,
            self.rect.width, self.rect.height, 0xFFE8E8E8);
        fb.fill_rect(self.rect.x as u32, (self.rect.bottom() - 1) as u32,
            self.rect.width, 1, 0xFFCCCCCC);

        // Render buttons
        use super::widgets::Widget;
        for btn in [
            &self.back_button,
            &self.forward_button,
            &self.up_button,
            &self.refresh_button,
            &self.home_button,
            &self.search_button,
            &self.new_folder_button,
            &self.view_mode_button,
        ] {
            super::widgets::draw_widget(fb, &Widget::Button(ButtonWidget {
                rect: btn.rect,
                text: btn.text,
                text_len: btn.text_len,
                is_pressed: btn.is_pressed,
                is_hovered: btn.is_hovered,
                color: 0xFFDDDDDD,
                hover_color: 0xFFEEEEEE,
                text_color: 0xFF333333,
                border_radius: 3,
            }));
        }
    }
}

// ── File Manager ─────────────────────────────────────────────────────────────

/// Graphical File Manager application.
pub struct FileManager {
    pub window_id: usize,
    /// Current directory path
    pub current_path: [u8; 256],
    pub path_len: usize,
    /// Address bar
    pub address_bar: TextBoxWidget,
    /// File list
    pub file_list: ListView,
    /// Sidebar (quick access folders)
    pub sidebar: TreeView,
    /// Status bar
    pub status_bar: StatusBar,
    /// Toolbar
    pub toolbar: FileManagerToolbar,
    /// Selected file name
    pub selected_file: [u8; 64],
    /// Right-click context menu
    pub context_menu: ContextMenu,
    /// View mode
    pub view_mode: FileViewMode,
    /// Navigation history
    pub history: [[u8; 256]; 16],
    pub history_count: usize,
    pub history_pos: isize,
    /// Directory entries cache
    pub entries: [FsEntry; 256],
    pub entry_count: usize,
    /// File operation state
    pub copy_source: [u8; 256],
    pub copy_source_len: usize,
    pub is_cut: bool,
    /// Window client area
    pub client_rect: Rect,
}

impl FileManager {
    pub fn new(window_id: usize) -> Self {
        // Default to root
        let mut path_buf = [0u8; 256];
        let root = b"/";
        for (i, b) in root.iter().enumerate() {
            path_buf[i] = *b;
        }

        let mut fm = FileManager {
            window_id,
            current_path: path_buf,
            path_len: 1,
            address_bar: TextBoxWidget::new(0, 0, 400, "/"),
            file_list: ListView::new(0, 0, 600, 400),
            sidebar: TreeView::new(0, 0, 180, 400),
            status_bar: StatusBar::new(0, 0, 800),
            toolbar: FileManagerToolbar::new(0, 0, 800),
            selected_file: [0u8; 64],
            context_menu: ContextMenu::new(),
            view_mode: FileViewMode::List,
            history: [[0u8; 256]; 16],
            history_count: 0,
            history_pos: -1,
            entries: [FsEntry::new(); 256],
            entry_count: 0,
            copy_source: [0u8; 256],
            copy_source_len: 0,
            is_cut: false,
            client_rect: Rect::new(0, 0, 800, 500),
        };

        // Build default sidebar items
        fm.build_sidebar();
        fm.status_bar.add_section("Ready", 50);
        fm.status_bar.add_section("", 25);
        fm.status_bar.add_section("", 25);
        fm.update_status_bar();

        // Default context menu items
        fm.context_menu.add_item("Open", 1);
        fm.context_menu.add_item("Copy", 2);
        fm.context_menu.add_item("Cut", 3);
        fm.context_menu.add_item("Paste", 4);
        fm.context_menu.add_item("Delete", 5);
        fm.context_menu.add_item("Rename", 6);
        fm.context_menu.add_item("New Folder", 7);
        fm.context_menu.add_item("Properties", 8);

        fm
    }

    fn build_sidebar(&mut self) {
        // Quick-access tree
        let home_idx = self.sidebar.add_child("Home").unwrap_or(0);
        self.sidebar.node_pool[home_idx].icon_type = IconType::Home;

        let docs_idx = self.sidebar.add_child("Documents").unwrap_or(0);
        self.sidebar.node_pool[docs_idx].icon_type = IconType::Documents;

        let dl_idx = self.sidebar.add_child("Downloads").unwrap_or(0);
        self.sidebar.node_pool[dl_idx].icon_type = IconType::Downloads;

        // Add as root children
        let mut root_children = [0usize; 8];
        root_children[0] = home_idx;
        root_children[1] = docs_idx;
        root_children[2] = dl_idx;
        root_children[3] = self.sidebar.add_child("/").unwrap_or(0);
        root_children[4] = self.sidebar.add_child("/tmp").unwrap_or(0);
        root_children[5] = self.sidebar.add_child("/proc").unwrap_or(0);
        root_children[6] = self.sidebar.add_child("/etc").unwrap_or(0);
        root_children[7] = self.sidebar.add_child("/home").unwrap_or(0);
        self.sidebar.root.child_count = 8;
        self.sidebar.root.children = root_children;
    }

    /// Set the client area rect (from window size).
    pub fn set_client_rect(&mut self, x: i32, y: i32, w: u32, h: u32) {
        self.client_rect = Rect::new(x, y, w, h);

        // Layout components
        self.toolbar.rect = Rect::new(x, y, w, 36);
        self.toolbar.back_button.rect = Rect::new(x, y, 28, 28);
        self.toolbar.forward_button.rect = Rect::new(x + 30, y, 28, 28);
        self.toolbar.up_button.rect = Rect::new(x + 60, y, 28, 28);
        self.toolbar.refresh_button.rect = Rect::new(x + 90, y, 28, 28);
        self.toolbar.home_button.rect = Rect::new(x + 120, y, 28, 28);
        self.toolbar.search_button.rect = Rect::new(x + 150, y, 28, 28);
        self.toolbar.new_folder_button.rect = Rect::new(x + 180, y, 28, 28);
        self.toolbar.view_mode_button.rect = Rect::new((x + w as i32) - 30, y, 28, 28);

        // Address bar
        self.address_bar.rect = Rect::new(x + 215, y + 2, w - 250, 24);
        for i in 0..self.path_len.min(512) {
            self.address_bar.buffer[i] = self.current_path[i];
        }
        self.address_bar.buffer_len = self.path_len;

        // Sidebar
        self.sidebar.rect = Rect::new(x, y + 37, 180, h - 61);

        // File list
        self.file_list.rect = Rect::new(x + 182, y + 37, w - 182, h - 61);

        // Status bar
        self.status_bar.rect = Rect::new(x, y + h as i32 - 24, w, 24);
    }

    /// Add a history entry.
    fn push_history(&mut self) {
        // If we're not at the end, truncate
        if self.history_pos < (self.history_count as isize) - 1 {
            self.history_count = (self.history_pos + 1) as usize;
        }
        // Add current path
        if self.history_count < 16 {
            let mut entry = [0u8; 256];
            for i in 0..self.path_len {
                entry[i] = self.current_path[i];
            }
            self.history[self.history_count] = entry;
            self.history_pos = self.history_count as isize;
            self.history_count += 1;
        }
    }

    /// Navigate to a directory.
    pub fn navigate_to(&mut self, path: &str) {
        let plen = core::cmp::min(path.len(), 255);
        for (i, b) in path.bytes().enumerate().take(plen) {
            self.current_path[i] = b;
        }
        self.path_len = plen;
        self.current_path[plen] = 0;
        for i in 0..self.path_len.min(512) {
    self.address_bar.buffer[i] = self.current_path[i];
}
self.address_bar.buffer_len = self.path_len;
        self.address_bar.buffer_len = plen;
        self.push_history();
        self.refresh();
    }

    /// Go up one directory level.
    pub fn go_up(&mut self) {
        let path_copy = self.current_path;
        let plen = self.path_len;
        let s = core::str::from_utf8(&path_copy[..plen]).unwrap_or("/");
        // Find last '/'
        if let Some(pos) = s[..s.len().saturating_sub(1)].rfind('/') {
            let parent = &s[..=pos];
            self.navigate_to(parent);
        }
    }

    /// Go back in navigation history.
    pub fn go_back(&mut self) {
        if self.history_pos > 0 {
            self.history_pos -= 1;
            let entry = &self.history[self.history_pos as usize];
            let len = entry.iter().position(|&c| c == 0).unwrap_or(256);
            let s = core::str::from_utf8(&entry[..len]).unwrap_or("/");
            let plen = core::cmp::min(s.len(), 255);
            for (i, b) in s.bytes().enumerate().take(plen) {
                self.current_path[i] = b;
            }
            self.path_len = plen;
            for i in 0..self.path_len.min(512) {
    self.address_bar.buffer[i] = self.current_path[i];
}
self.address_bar.buffer_len = self.path_len;
            self.address_bar.buffer_len = plen;
            self.refresh();
        }
    }

    /// Go forward in navigation history.
    pub fn go_forward(&mut self) {
        if (self.history_pos as usize) < self.history_count.saturating_sub(1) {
            self.history_pos += 1;
            let entry = &self.history[self.history_pos as usize];
            let len = entry.iter().position(|&c| c == 0).unwrap_or(256);
            let s = core::str::from_utf8(&entry[..len]).unwrap_or("/");
            let plen = core::cmp::min(s.len(), 255);
            for (i, b) in s.bytes().enumerate().take(plen) {
                self.current_path[i] = b;
            }
            self.path_len = plen;
            for i in 0..self.path_len.min(512) {
    self.address_bar.buffer[i] = self.current_path[i];
}
self.address_bar.buffer_len = self.path_len;
            self.address_bar.buffer_len = plen;
            self.refresh();
        }
    }

    /// Refresh current directory listing via VFS IPC.
    pub fn refresh(&mut self) {
        // Populate file list from current path
        // In a real implementation, this would IPC to the VFS service (EP 2)
        // For now, populate with placeholder entries
        self.file_list.clear();
        self.entry_count = 0;

        // Placeholder: add parent directory link
        let mut item = ListViewItem::new("..");
        item.icon_type = IconType::Folder;
        self.file_list.add_item("..");
        if self.entry_count < 256 {
            self.entries[self.entry_count] = FsEntry::new();
            self.entries[self.entry_count].name[0] = b'.';
            self.entries[self.entry_count].name[1] = b'.';
            self.entries[self.entry_count].name_len = 2;
            self.entries[self.entry_count].is_directory = true;
            self.entry_count += 1;
        }

        self.update_status_bar();
    }

    /// Open selected file or directory.
    pub fn open_selected(&mut self) {
        let sel = self.file_list.selected_index;
        if sel < 0 || sel as usize >= self.file_list.item_count { return; }
        let item = &self.file_list.items[sel as usize];
        let name = item.text_str();

        if name == ".." {
            self.go_up();
            return;
        }

        // Check if it's a directory (from cached entries)
        for i in 0..self.entry_count {
            if self.entries[i].name_str() == name {
                if self.entries[i].is_directory {
                    let mut new_path = [0u8; 256];
                    let mut plen = 0;
                    for j in 0..self.path_len {
                        new_path[plen] = self.current_path[j];
                        plen += 1;
                    }
                    if plen > 0 && new_path[plen - 1] != b'/' {
                        new_path[plen] = b'/';
                        plen += 1;
                    }
                    for (j, b) in name.bytes().enumerate() {
                        new_path[plen + j] = b;
                    }
                    plen += name.len();
                    let s = core::str::from_utf8(&new_path[..plen]).unwrap_or("/");
                    self.navigate_to(s);
                }
                return;
            }
        }
    }

    /// Delete selected file (marks as deleted, would IPC to VFS).
    pub fn delete_selected(&mut self) {
        let sel = self.file_list.selected_index;
        if sel < 0 || sel as usize >= self.file_list.item_count { return; }
        self.file_list.remove_item(sel as usize);
        self.update_status_bar();
    }

    /// Rename selected file.
    pub fn rename_selected(&mut self, new_name: &str) {
        let sel = self.file_list.selected_index;
        if sel < 0 || sel as usize >= self.file_list.item_count { return; }

        // Create the new item with updated name
        let mut new_item = ListViewItem::new(new_name);
        new_item.icon_type = IconType::File;
        self.file_list.items[sel as usize] = new_item;

        // Update entry cache
        let sel_u = sel as usize;
        if sel_u < self.entry_count {
            let nlen = core::cmp::min(new_name.len(), 64);
            for (i, b) in new_name.bytes().enumerate().take(nlen) {
                self.entries[sel_u].name[i] = b;
            }
            self.entries[sel_u].name_len = nlen;
        }

        self.update_status_bar();
    }

    /// Create new folder.
    pub fn new_folder(&mut self, name: &str) {
        self.file_list.add_item(name);
        if self.entry_count < 256 {
            let mut entry = FsEntry::new();
            let nlen = core::cmp::min(name.len(), 64);
            for (i, b) in name.bytes().enumerate().take(nlen) {
                entry.name[i] = b;
            }
            entry.name_len = nlen;
            entry.is_directory = true;
            self.entries[self.entry_count] = entry;
            self.entry_count += 1;
        }
        self.update_status_bar();
    }

    /// Copy file.
    pub fn copy_file(&mut self, _src: &str, _dst: &str) {
        // Would IPC to VFS service for actual copy
        // For now, just update status
        self.status_bar.set_section(0, "Copy complete");
    }

    /// Move file.
    pub fn move_file(&mut self, _src: &str, _dst: &str) {
        self.status_bar.set_section(0, "Move complete");
    }

    /// Get file info.
    pub fn get_file_info(&self, _path: &str) -> Option<FsEntry> {
        // Would query VFS service
        None
    }

    fn update_status_bar(&mut self) {
        let mut items_str = [0u8; 32];
        let item_count = self.file_list.item_count;
        let mut pos = 0;
        if item_count >= 100 {
            items_str[pos] = b'0' + (item_count / 100) as u8; pos += 1;
            items_str[pos] = b'0' + ((item_count / 10) % 10) as u8; pos += 1;
        } else if item_count >= 10 {
            items_str[pos] = b'0' + (item_count / 10) as u8; pos += 1;
        }
        items_str[pos] = b'0' + (item_count % 10) as u8; pos += 1;
        items_str[pos] = b' '; pos += 1;
        items_str[pos] = b'i'; pos += 1;
        items_str[pos] = b't'; pos += 1;
        items_str[pos] = b'e'; pos += 1;
        items_str[pos] = b'm'; pos += 1;
        items_str[pos] = b's'; pos += 1;

        let s = core::str::from_utf8(&items_str[..pos]).unwrap_or("");
        self.status_bar.set_section(1, s);
    }

    fn path_str(&self) -> &str {
        core::str::from_utf8(&self.current_path[..self.path_len]).unwrap_or("/")
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    /// Render the file manager.
    pub fn render(&self, fb: &mut Framebuffer) {
        // Client area background
        fb.fill_rect(
            self.client_rect.x as u32, self.client_rect.y as u32,
            self.client_rect.width, self.client_rect.height,
            0xFFF0F0F0,
        );

        // Toolbar
        self.toolbar.render(fb);

        // Address bar
        let addr_label = self.path_str();
        draw_text_wrapped(fb, (self.address_bar.rect.x - 60) as u32,
            (self.address_bar.rect.y + 4) as u32,
            56, "Path:", 0xFF333333, 0xFFE8E8E8);

        let fb_ref = fb as *mut Framebuffer;
        unsafe {
            let addr_buf = super::widgets::Widget::TextBox(
                super::widgets::TextBoxWidget {
                    rect: self.address_bar.rect,
                    buffer: self.address_bar.buffer,
                    buffer_len: self.address_bar.buffer_len,
                    cursor_pos: self.address_bar.buffer_len,
                    is_focused: self.address_bar.is_focused,
                    scroll_offset: 0,
                    placeholder: self.address_bar.placeholder,
                    placeholder_len: self.address_bar.placeholder_len,
                    border_color: self.address_bar.border_color,
                    bg_color: self.address_bar.bg_color,
                }
            );
            super::widgets::draw_widget(&mut *fb_ref, &addr_buf);
        }

        // Sidebar
        let sidebar_bg = Rect::new(
            self.sidebar.rect.x, self.sidebar.rect.y,
            self.sidebar.rect.width, self.sidebar.rect.height,
        );
        fb.fill_rect(sidebar_bg.x as u32, sidebar_bg.y as u32,
            sidebar_bg.width, sidebar_bg.height, 0xFFF5F5F5);
        super::widgets::draw_widget(fb, &super::widgets::Widget::TreeView(
            super::widgets::TreeView {
                rect: self.sidebar.rect,
                root: super::widgets::TreeNode::new("Quick Access"),
                node_pool: self.sidebar.node_pool,
                pool_count: self.sidebar.pool_count,
                selected_path: self.sidebar.selected_path,
                selection_depth: self.sidebar.selection_depth,
                scroll_offset: self.sidebar.scroll_offset,
                expanded_nodes: self.sidebar.expanded_nodes,
                bg_color: 0xFFF5F5F5,
                text_color: 0xFF333333,
                selection_color: 0xFF4A90D9,
                indent_width: 16,
                item_height: 20,
            }
        ));

        // Divider line between sidebar and file list
        fb.fill_rect(
            self.sidebar.rect.right() as u32,
            self.sidebar.rect.y as u32,
            1,
            self.sidebar.rect.height,
            0xFFCCCCCC,
        );

        // File list
        super::widgets::draw_widget(fb, &super::widgets::Widget::ListView(
            super::widgets::ListView {
                rect: self.file_list.rect,
                items: self.file_list.items,
                item_count: self.file_list.item_count,
                selected_index: self.file_list.selected_index,
                scroll_offset: self.file_list.scroll_offset,
                visible_count: self.file_list.visible_count,
                item_height: self.file_list.item_height,
                multi_select: self.file_list.multi_select,
                selected_indices: self.file_list.selected_indices,
                selection_count: self.file_list.selection_count,
                sort_column: self.file_list.sort_column,
                sort_ascending: self.file_list.sort_ascending,
                bg_color: 0xFFFFFFFF,
                text_color: 0xFF000000,
                selection_color: 0xFF4A90D9,
                alt_row_color: 0xFFF0F8FF,
                border_color: 0xFFCCCCCC,
            }
        ));

        // Status bar
        super::widgets::draw_widget(fb, &super::widgets::Widget::StatusBar(
            super::widgets::StatusBar {
                rect: self.status_bar.rect,
                sections: self.status_bar.sections,
                section_count: self.status_bar.section_count,
                color: 0xFF2D2D44,
                text_color: 0xFFFFFFFF,
                border_color: 0xFF404040,
            }
        ));

        // Context menu (if visible)
        self.context_menu.render(fb);
    }

    // ── Event Handling ─────────────────────────────────────────────────────

    /// Handle mouse click.
    pub fn handle_click(&mut self, x: i32, y: i32, button: u8) {
        // Right-click -> context menu
        if button == 2 {
            self.context_menu.show(x, y);
            return;
        }
        self.context_menu.hide();

        // Check toolbar buttons
        let toolbar = &self.toolbar;
        let p = graphics::Point::new(x, y);

        if toolbar.back_button.rect.contains(&p) {
            self.go_back();
            return;
        }
        if toolbar.forward_button.rect.contains(&p) {
            self.go_forward();
            return;
        }
        if toolbar.up_button.rect.contains(&p) {
            self.go_up();
            return;
        }
        if toolbar.refresh_button.rect.contains(&p) {
            self.refresh();
            return;
        }
        if toolbar.home_button.rect.contains(&p) {
            self.navigate_to("/home");
            return;
        }
        if toolbar.new_folder_button.rect.contains(&p) {
            self.new_folder("New Folder");
            return;
        }

        // File list selection
        if self.file_list.rect.contains(&p) {
            if let Some(idx) = self.file_list.item_at_y(y) {
                self.file_list.selected_index = idx as isize;
            }
            return;
        }

        // Sidebar click
        if self.sidebar.rect.contains(&p) {
            let rel_y = y - self.sidebar.rect.y;
            let item_idx = rel_y / 20;
            if item_idx >= 0 && (item_idx as usize) < self.sidebar.root.child_count {
                let child_idx = self.sidebar.root.children[item_idx as usize];
                if child_idx < self.sidebar.pool_count {
                    let node = &self.sidebar.node_pool[child_idx];
                    let target = node.label_str();
                    match target {
                        "/" => self.navigate_to("/"),
                        "/tmp" => self.navigate_to("/tmp"),
                        "/proc" => self.navigate_to("/proc"),
                        "/etc" => self.navigate_to("/etc"),
                        "/home" => self.navigate_to("/home"),
                        "Home" => self.navigate_to("/home"),
                        "Documents" => self.navigate_to("/home/documents"),
                        "Downloads" => self.navigate_to("/home/downloads"),
                        _ => {}
                    }
                }
            }
        }
    }

    /// Handle double click.
    pub fn handle_double_click(&mut self, _x: i32, _y: i32) {
        self.open_selected();
    }

    /// Handle keyboard input.
    pub fn handle_key(&mut self, keycode: u8, _modifier: u8) {
        match keycode {
            28 => { // Enter
                self.open_selected();
            }
            14 => { // Backspace
                self.go_up();
            }
            82 => { // Up arrow
                let new_idx = self.file_list.selected_index - 1;
                if new_idx >= 0 {
                    self.file_list.selected_index = new_idx;
                }
            }
            81 => { // Down arrow
                let new_idx = self.file_list.selected_index + 1;
                if (new_idx as usize) < self.file_list.item_count {
                    self.file_list.selected_index = new_idx;
                }
            }
            _ => {}
        }
    }
}
