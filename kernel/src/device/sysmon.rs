// V39b — Desktop System Monitor
//
// Provides a graphical system monitor showing CPU usage, memory usage,
// process list, and performance counters for the TrainOS desktop environment.

use super::framebuffer::Framebuffer;
use super::graphics::{
    self, draw_border, draw_text_centered, draw_text_wrapped,
    font_8x16, Color, DARK_GRAY, GRAY, LIGHT_GRAY, WHITE, Rect, BLACK, GREEN, RED,
};
use super::widgets::{IconType, ListView, ListViewItem};

// ── Graph Label Format ───────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GraphLabelFormat {
    Percentage,
    Absolute { unit: [u8; 8], unit_len: usize },
    Raw,
}

// ── Line Graph ───────────────────────────────────────────────────────────────

pub struct LineGraph {
    pub rect: Rect,
    pub data: *const f32,
    pub data_len: usize,
    pub max_value: f32,
    pub color: Color,
    pub grid_color: Color,
    pub show_grid: bool,
    pub show_labels: bool,
    pub label_format: GraphLabelFormat,
    pub bg_color: Color,
    pub border_color: Color,
}

impl LineGraph {
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        LineGraph {
            rect: Rect::new(x, y, w, h),
            data: core::ptr::null(),
            data_len: 0,
            max_value: 100.0,
            color: 0xFF4A90D9,
            grid_color: 0xFF333344,
            show_grid: true,
            show_labels: true,
            label_format: GraphLabelFormat::Percentage,
            bg_color: 0xFF1E1E2E,
            border_color: 0xFF333344,
        }
    }

    /// Render the line graph using an external data slice.
    pub fn render_with_data(&self, fb: &mut Framebuffer, data: &[f32]) {
        // Background
        fb.fill_rect(self.rect.x as u32, self.rect.y as u32,
            self.rect.width, self.rect.height, self.bg_color);
        draw_border(fb, &self.rect, 1, self.border_color);

        if data.is_empty() || self.max_value <= 0.0 {
            return;
        }

        // Grid lines
        if self.show_grid {
            let grid_color = self.grid_color;
            // Horizontal grid lines at 25%, 50%, 75%
            for frac in [0.25, 0.5, 0.75] {
                let gy = self.rect.y + (self.rect.height as f32 * (1.0 - frac)) as i32;
                fb.fill_rect(
                    self.rect.x as u32, gy as u32,
                    self.rect.width, 1,
                    grid_color,
                );
            }

            // Labels
            if self.show_labels {
                for frac in [0.0, 0.25, 0.5, 0.75, 1.0] {
                    let gy = self.rect.y + (self.rect.height as f32 * (1.0 - frac)) as i32 - 8;
                    let val = self.max_value * frac;
                    let mut buf = [0u8; 16];
                    let len = format_graph_label(val, &self.label_format, &mut buf);
                    let label = core::str::from_utf8(&buf[..len]).unwrap_or("");
                    draw_text_wrapped(fb,
                        (self.rect.x + 2) as u32, gy.max(self.rect.y) as u32,
                        30, label, 0xFF888888, self.bg_color);
                }
            }
        }

        // Line data
        if data.len() < 2 { return; }

        let plot_left = if self.show_labels { 34 } else { 4 };
        let plot_w = self.rect.width as i32 - plot_left - 4;
        if plot_w <= 0 { return; }

        let plot_h = self.rect.height as i32 - 8;
        let plot_bottom = self.rect.bottom() - 4;
        let plot_top = self.rect.y + 4;

        for i in 1..data.len() {
            let x0 = self.rect.x + plot_left + ((i - 1) as i32 * plot_w / (data.len() as i32).max(1));
            let x1 = self.rect.x + plot_left + (i as i32 * plot_w / (data.len() as i32).max(1));
            let v0 = data[i - 1].max(0.0).min(self.max_value);
            let v1 = data[i].max(0.0).min(self.max_value);
            let y0 = plot_bottom - (plot_h as f32 * (v0 / self.max_value)) as i32;
            let y1 = plot_bottom - (plot_h as f32 * (v1 / self.max_value)) as i32;

            let y0c = y0.max(plot_top).min(plot_bottom);
            let y1c = y1.max(plot_top).min(plot_bottom);

            fb.draw_line(x0 as u32, y0c as u32, x1 as u32, y1c as u32, self.color);
        }

        // Fill under graph (simple rect fill approximation)
        if data.len() > 0 {
            let last_val = data[data.len() - 1].max(0.0).min(self.max_value);
            let last_y = plot_bottom - (plot_h as f32 * (last_val / self.max_value)) as i32;
            // Glowing dot at current value
            let dot_x = self.rect.x + plot_left + plot_w;
            fb.fill_rect(
                (dot_x - 2) as u32, (last_y - 2) as u32,
                4, 4, WHITE,
            );
        }
    }
}

// ── Bar Graph ────────────────────────────────────────────────────────────────

pub struct BarGraph {
    pub rect: Rect,
    pub values: [f32; 8],
    pub value_count: usize,
    pub labels: [[u8; 16]; 8],
    pub label_lens: [usize; 8],
    pub colors: [Color; 8],
    pub max_value: f32,
    pub show_values: bool,
    pub bg_color: Color,
    pub border_color: Color,
}

impl BarGraph {
    pub fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        BarGraph {
            rect: Rect::new(x, y, w, h),
            values: [0.0; 8],
            value_count: 0,
            labels: [[0u8; 16]; 8],
            label_lens: [0; 8],
            colors: [0xFF4A90D9; 8],
            max_value: 100.0,
            show_values: true,
            bg_color: 0xFF1E1E2E,
            border_color: 0xFF333344,
        }
    }

    pub fn set_label(&mut self, idx: usize, label: &str) {
        if idx < 8 {
            let llen = core::cmp::min(label.len(), 16);
            for (i, b) in label.bytes().enumerate().take(llen) {
                self.labels[idx][i] = b;
            }
            self.label_lens[idx] = llen;
        }
    }

    pub fn get_label(&self, idx: usize) -> &str {
        if idx < 8 {
            core::str::from_utf8(&self.labels[idx][..self.label_lens[idx]]).unwrap_or("")
        } else {
            ""
        }
    }

    /// Render the bar graph.
    pub fn render(&self, fb: &mut Framebuffer) {
        fb.fill_rect(self.rect.x as u32, self.rect.y as u32,
            self.rect.width, self.rect.height, self.bg_color);
        draw_border(fb, &self.rect, 1, self.border_color);

        if self.value_count == 0 || self.max_value <= 0.0 { return; }

        let bar_area_w = self.rect.width as i32 - 8;
        let bar_area_h = self.rect.height as i32 - 24;
        let bar_w = bar_area_w / self.value_count as i32 - 4;

        for i in 0..self.value_count {
            let bx = self.rect.x + 6 + i as i32 * (bar_w + 4);
            let bh = (bar_area_h as f32 * (self.values[i] / self.max_value)) as i32;
            let by = self.rect.bottom() - 20 - bh;

            // Draw bar
            fb.fill_rect(bx as u32, by as u32, bar_w as u32, bh as u32, self.colors[i]);

            // Value on top of bar
            if self.show_values && bh > 12 {
                let mut vb = [0u8; 16];
                let vlen = format_int(self.values[i] as u32, &mut vb);
                let val_str = core::str::from_utf8(&vb[..vlen]).unwrap_or("");
                draw_text_wrapped(fb, bx as u32, (by - 14) as u32,
                    bar_w as u32, val_str, WHITE, self.bg_color);
            }

            // Label below
            let label = self.get_label(i);
            draw_text_wrapped(fb, bx as u32, (self.rect.bottom() - 14) as u32,
                bar_w as u32, label, 0xFF888888, self.bg_color);
        }
    }
}

// ── Perf Label ───────────────────────────────────────────────────────────────

pub struct PerfLabel {
    pub name: [u8; 24],
    pub name_len: usize,
    pub value: u64,
    pub prev_value: u64,
    pub color: Color,
}

impl PerfLabel {
    pub fn new(name: &str) -> Self {
        let mut name_buf = [0u8; 24];
        let nlen = core::cmp::min(name.len(), 24);
        for (i, b) in name.bytes().enumerate().take(nlen) {
            name_buf[i] = b;
        }
        PerfLabel {
            name: name_buf,
            name_len: nlen,
            value: 0,
            prev_value: 0,
            color: 0xFF4A90D9,
        }
    }

    pub fn name_str(&self) -> &str {
        core::str::from_utf8(&self.name[..self.name_len]).unwrap_or("")
    }
}

// ── System Monitor ───────────────────────────────────────────────────────────

/// Graphical System Monitor application.
pub struct SystemMonitor {
    pub window_id: usize,
    /// CPU usage graph
    pub cpu_graph: LineGraph,
    pub cpu_usage: f32,
    pub cpu_history: [f32; 128],
    pub cpu_history_pos: usize,
    /// Memory usage graph
    pub mem_graph: BarGraph,
    pub mem_total: u64,
    pub mem_used: u64,
    pub mem_free: u64,
    /// Process list
    pub process_list: ListView,
    /// Performance counters
    pub perf_counters: [PerfLabel; 8],
    pub perf_count: usize,
    /// Update interval
    pub update_interval_ms: u64,
    pub last_update: u64,
    /// Tab selection
    pub active_tab: usize,
    /// Client area
    pub client_rect: Rect,
}

impl SystemMonitor {
    pub fn new(window_id: usize) -> Self {
        let mut cpu_history = [0.0f32; 128];
        for i in 0..128 {
            cpu_history[i] = 0.0;
        }

        let mut counters = [
            PerfLabel::new("CPU"),
            PerfLabel::new("Mem"),
            PerfLabel::new("Disk R"),
            PerfLabel::new("Disk W"),
            PerfLabel::new("Net In"),
            PerfLabel::new("Net Out"),
            PerfLabel::new("IPC"),
            PerfLabel::new("Ctx Sw"),
        ];
        counters[0].color = 0xFF4A90D9;
        counters[1].color = 0xFF44CC44;
        counters[2].color = 0xFFFFAA00;
        counters[3].color = 0xFFFF6600;
        counters[4].color = 0xFF3399FF;
        counters[5].color = 0xFF9933FF;
        counters[6].color = 0xFFFF3333;
        counters[7].color = 0xFF33CCCC;

        SystemMonitor {
            window_id,
            cpu_graph: LineGraph::new(20, 60, 460, 160),
            cpu_usage: 0.0,
            cpu_history,
            cpu_history_pos: 0,
            mem_graph: BarGraph::new(20, 250, 460, 120),
            mem_total: 0,
            mem_used: 0,
            mem_free: 0,
            process_list: ListView::new(20, 400, 460, 260),
            perf_counters: counters,
            perf_count: 8,
            update_interval_ms: 1000,
            last_update: 0,
            active_tab: 0,
            client_rect: Rect::new(0, 0, 500, 700),
        }
    }

    /// Set the client area rect.
    pub fn set_client_rect(&mut self, x: i32, y: i32, w: u32, h: u32) {
        self.client_rect = Rect::new(x, y, w, h);

        // CPu graph
        self.cpu_graph.rect = Rect::new(
            x + 20, y + 60,
            w - 40, (h - 80) / 3,
        );

        // Memory graph
        let one_third = (h - 80) / 3;
        self.mem_graph.rect = Rect::new(
            x + 20, y + 60 + one_third as i32 + 20,
            w - 40, one_third,
        );

        // Process list
        self.process_list.rect = Rect::new(
            x + 20, y + 60 + 2 * (one_third as i32 + 20),
            w - 40, one_third,
        );
        self.process_list.visible_count = (self.process_list.rect.height / 20) as usize;
    }

    /// Update all metrics (cpu, memory, processes).
    pub fn update(&mut self) {
        // Simulate CPU usage (would read from PMU/per-CPU stats)
        // In production, use PMU counters or /proc/perf
        let tick = unsafe { crate::trap::TICK_COUNT };
        let cpu_variation = ((tick % 100) as f32) / 100.0;
        self.cpu_usage = 30.0 + cpu_variation * 40.0;

        // Update CPU history
        self.cpu_history[self.cpu_history_pos % 128] = self.cpu_usage;
        self.cpu_history_pos = (self.cpu_history_pos + 1) % 128;

        // Simulate memory usage
        // In production, query buddy allocator stats
        self.mem_total = 512; // MB
        self.mem_used = 128 + ((tick % 50) as u64);
        self.mem_free = self.mem_total - self.mem_used;

        self.mem_graph.value_count = 3;
        self.mem_graph.values = [
            self.mem_used as f32,
            self.mem_free as f32,
            0.0,
            0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        self.mem_graph.colors = [
            0xFF4A90D9,
            0xFF44CC44,
            0xFF888888,
            0xFF888888, 0xFF888888, 0xFF888888, 0xFF888888, 0xFF888888,
        ];
        self.mem_graph.set_label(0, "Used");
        self.mem_graph.set_label(1, "Free");
        self.mem_graph.max_value = self.mem_total as f32;

        // Update non-previous values (prev_value for rate calculation)
        for i in 0..self.perf_count {
            self.perf_counters[i].prev_value = self.perf_counters[i].value;
        }
        self.perf_counters[0].value = self.cpu_usage as u64;
        self.perf_counters[1].value = self.mem_used;
        self.perf_counters[2].value = ((tick % 1000) * 50) as u64;
        self.perf_counters[3].value = ((tick % 500) * 30) as u64;
        self.perf_counters[4].value = ((tick % 2000) * 10) as u64;
        self.perf_counters[5].value = ((tick % 1500) * 8) as u64;
        self.perf_counters[6].value = (tick % 500) as u64;
        self.perf_counters[7].value = (tick % 300) as u64;

        // Update process list
        // In production, query process manager
        self.update_process_list();
    }

    fn update_process_list(&mut self) {
        self.process_list.clear();

        // In production, iterate through process table
        // For now, add placeholder entries
        let procs = [
            "init        (1)  RUN  0% ",
            "fs          (2)  RUN  2% ",
            "net         (3)  RUN  1% ",
            "sh          (4)  RUN  0% ",
            "tcp         (5)  SLEEP 0% ",
            "http        (6)  RUN  3% ",
            "echo        (7)  SLEEP 0% ",
            "proc        (8)  RUN  0% ",
            "sysmon      (9)  RUN  5% ",
            "gui        (10)  RUN  8% ",
            "term       (11)  RUN  2% ",
            "fileman    (12)  SLEEP 0% ",
        ];

        for &p in &procs {
            self.process_list.add_item(p);
        }
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    /// Render the system monitor.
    pub fn render(&self, fb: &mut Framebuffer) {
        // Background
        fb.fill_rect(
            self.client_rect.x as u32, self.client_rect.y as u32,
            self.client_rect.width, self.client_rect.height,
            0xFF1E1E2E,
        );

        // Title
        draw_text_wrapped(fb,
            (self.client_rect.x + 20) as u32,
            (self.client_rect.y + 10) as u32,
            200, "System Monitor", WHITE, 0xFF1E1E2E);

        let font = font_8x16();

        // CPU section label
        let cpu_buf = format_cpu_label(self.cpu_usage);
        let cpu_end = cpu_buf.iter().position(|&b| b == 0).unwrap_or(32);
        let cpu_str = core::str::from_utf8(&cpu_buf[..cpu_end]).unwrap_or("");
        draw_text_wrapped(fb,
            self.cpu_graph.rect.x as u32,
            (self.cpu_graph.rect.y - 18) as u32,
            200, cpu_str, 0xFF4A90D9, 0xFF1E1E2E);

        // CPU graph
        self.cpu_graph.render_with_data(fb, &self.cpu_history);

        // Memory section label
        let mut mem_buf = [0u8; 64];
        let mpos = format_mem_label(self.mem_used, self.mem_total, &mut mem_buf);
        let mem_label = core::str::from_utf8(&mem_buf[..mpos]).unwrap_or("");
        draw_text_wrapped(fb,
            self.mem_graph.rect.x as u32,
            (self.mem_graph.rect.y - 18) as u32,
            200, mem_label, 0xFF44CC44, 0xFF1E1E2E);

        // Memory graph
        self.mem_graph.render(fb);

        // Process list background
        let pl_bg = Rect::new(
            self.process_list.rect.x,
            self.process_list.rect.y - 20,
            self.process_list.rect.width,
            self.process_list.rect.height + 20,
        );
        fb.fill_rect(pl_bg.x as u32, pl_bg.y as u32, pl_bg.width, pl_bg.height, 0xFF252535);

        // Process list label
        draw_text_wrapped(fb,
            self.process_list.rect.x as u32,
            (self.process_list.rect.y - 16) as u32,
            100, "Processes", 0xFF888888, 0xFF252535);

        // Process list
        super::widgets::draw_widget(fb, &super::widgets::Widget::ListView(
            super::widgets::ListView {
                rect: self.process_list.rect,
                items: self.process_list.items,
                item_count: self.process_list.item_count,
                selected_index: self.process_list.selected_index,
                scroll_offset: self.process_list.scroll_offset,
                visible_count: self.process_list.visible_count,
                item_height: self.process_list.item_height,
                multi_select: self.process_list.multi_select,
                selected_indices: self.process_list.selected_indices,
                selection_count: self.process_list.selection_count,
                sort_column: self.process_list.sort_column,
                sort_ascending: self.process_list.sort_ascending,
                bg_color: 0xFF252535,
                text_color: 0xFFD0D0D0,
                selection_color: 0xFF4A525A,
                alt_row_color: 0xFF2A2A3A,
                border_color: 0xFF333344,
            }
        ));

        // Perf counters row at bottom
        let perf_y = self.process_list.rect.bottom() + 10;
        let mut cx = self.client_rect.x + 20;
        let perf_count_u32 = if self.perf_count > 0 { self.perf_count as u32 } else { 1 };
        let perf_w = ((self.client_rect.width - 40) / perf_count_u32) as i32;

        for i in 0..self.perf_count {
            let mut perf_buf = [0u8; 48];
            let plen = format_perf_counter(&self.perf_counters[i], &mut perf_buf);
            let label = core::str::from_utf8(&perf_buf[..plen]).unwrap_or("");
            draw_text_wrapped(fb, cx as u32, perf_y as u32,
                perf_w as u32, label,
                self.perf_counters[i].color, 0xFF1E1E2E);
            cx += perf_w;
        }
    }

    // ── Actions ─────────────────────────────────────────────────────────────

    /// Kill a process.
    pub fn kill_process(&mut self, _pid: u32) {
        // Would call sys_kill via IPC
    }

    /// Change process priority.
    pub fn set_priority(&mut self, _pid: u32, _priority: u8) {
        // Would call sys_sched_setparam via IPC
    }

    /// Handle click.
    pub fn handle_click(&mut self, x: i32, y: i32, _button: u8) {
        if self.process_list.rect.contains(&graphics::Point::new(x, y)) {
            if let Some(idx) = self.process_list.item_at_y(y) {
                self.process_list.selected_index = idx as isize;
            }
        }
    }
}

// ── Formatting helpers ──────────────────────────────────────────────────────

fn format_graph_label(val: f32, fmt: &GraphLabelFormat, buf: &mut [u8; 16]) -> usize {
    match fmt {
        GraphLabelFormat::Percentage => {
            let pct = val as u32;
            if pct >= 100 {
                let s = b"100%";
                let l = core::cmp::min(s.len(), buf.len());
                for i in 0..l { buf[i] = s[i]; }
                l
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
        }
        GraphLabelFormat::Absolute { unit: ref unit_arr, unit_len: ref unit_len_ref } => {
            let ival = val as u32;
            let ilen = format_int(ival, buf);
            let ulen = core::cmp::min(*unit_len_ref, buf.len().saturating_sub(ilen));
            for i in 0..ulen {
                buf[ilen + i] = unit_arr[i];
            }
            ilen + ulen
        }
        GraphLabelFormat::Raw => {
            format_int(val as u32, buf)
        }
    }
}

fn format_int(mut val: u32, buf: &mut [u8; 16]) -> usize {
    if val == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut digits = [0u8; 16];
    let mut dlen = 0;
    while val > 0 && dlen < 16 {
        digits[dlen] = b'0' + (val % 10) as u8;
        val /= 10;
        dlen += 1;
    }
    let mut pos = 0;
    for i in (0..dlen).rev() {
        buf[pos] = digits[i];
        pos += 1;
    }
    pos
}

fn format_cpu_label(cpu: f32) -> [u8; 32] {
    let mut buf = [0u8; 32];
    let pct = cpu as u32;
    let s = b"CPU Usage: ";
    let mut pos = 0;
    for &b in s {
        if pos < 32 { buf[pos] = b; pos += 1; }
    }
    if pct >= 100 {
        buf[pos] = b'1'; pos += 1;
        buf[pos] = b'0'; pos += 1;
        buf[pos] = b'0'; pos += 1;
    } else if pct >= 10 {
        buf[pos] = b'0' + (pct / 10) as u8; pos += 1;
        buf[pos] = b'0' + (pct % 10) as u8; pos += 1;
    } else {
        buf[pos] = b'0' + pct as u8; pos += 1;
    }
    buf[pos] = b'%'; pos += 1;
    buf
}

fn format_mem_label(used: u64, total: u64, buf: &mut [u8; 64]) -> usize {
    let prefix = b"Memory: ";
    let mut pos = 0;
    for &b in prefix {
        if pos < 64 { buf[pos] = b; pos += 1; }
    }
    // Format used
    if used == 0 {
        buf[pos] = b'0'; pos += 1;
    } else {
        let mut digits = [0u8; 20];
        let mut dlen = 0;
        let mut v = used;
        while v > 0 && dlen < 20 {
            digits[dlen] = b'0' + (v % 10) as u8;
            v /= 10;
            dlen += 1;
        }
        for i in (0..dlen).rev() {
            if pos < 64 { buf[pos] = digits[i]; pos += 1; }
        }
    }
    let mid = b"/";
    for &b in mid {
        if pos < 64 { buf[pos] = b; pos += 1; }
    }
    // Format total
    if total == 0 {
        buf[pos] = b'0'; pos += 1;
    } else {
        let mut digits = [0u8; 20];
        let mut dlen = 0;
        let mut v = total;
        while v > 0 && dlen < 20 {
            digits[dlen] = b'0' + (v % 10) as u8;
            v /= 10;
            dlen += 1;
        }
        for i in (0..dlen).rev() {
            if pos < 64 { buf[pos] = digits[i]; pos += 1; }
        }
    }
    let suffix = b" MB";
    for &b in suffix {
        if pos < 64 { buf[pos] = b; pos += 1; }
    }
    pos
}

fn format_perf_counter(c: &PerfLabel, buf: &mut [u8; 48]) -> usize {
    let mut pos = 0;
    // Name
    for i in 0..c.name_len {
        if pos < 48 { buf[pos] = c.name[i]; pos += 1; }
    }
    let sep = b": ";
    for &b in sep {
        if pos < 48 { buf[pos] = b; pos += 1; }
    }
    // Value
    let v = c.value;
    if v == 0 {
        buf[pos] = b'0'; pos += 1;
    } else {
        let mut digits = [0u8; 20];
        let mut dlen = 0;
        let mut tmp = v;
        while tmp > 0 && dlen < 20 {
            digits[dlen] = b'0' + (tmp % 10) as u8;
            tmp /= 10;
            dlen += 1;
        }
        for i in (0..dlen).rev() {
            if pos < 48 { buf[pos] = digits[i]; pos += 1; }
        }
    }
    let delta = b" (D";
    for &b in delta {
        if pos < 48 { buf[pos] = b; pos += 1; }
    }
    // Delta
    let diff = c.value.saturating_sub(c.prev_value);
    if diff == 0 {
        buf[pos] = b'0'; pos += 1;
    } else {
        let mut digits = [0u8; 20];
        let mut dlen = 0;
        let mut tmp = diff;
        while tmp > 0 && dlen < 20 {
            digits[dlen] = b'0' + (tmp % 10) as u8;
            tmp /= 10;
            dlen += 1;
        }
        for i in (0..dlen).rev() {
            if pos < 48 { buf[pos] = digits[i]; pos += 1; }
        }
    }
    buf[pos] = b')'; pos += 1;
    pos
}
