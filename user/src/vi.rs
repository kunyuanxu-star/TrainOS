//! VI-like Text Editor for TrainOS
//!
//! A simple but functional vi-like modal editor

#![no_std]
#![no_main]

// Syscall numbers
const SYS_EXIT: usize = 93;
const SYS_READ: usize = 63;
const SYS_WRITE: usize = 64;
const SYS_OPENAT: usize = 56;
const SYS_CLOSE: usize = 57;
const SYS_BRK: usize = 214;

// File descriptor constants
const STDIN: usize = 0;
const STDOUT: usize = 1;
const STDERR: usize = 2;

// Open flags
const O_RDONLY: usize = 0;
const O_WRONLY: usize = 1;
const O_CREAT: usize = 0o100;
const O_TRUNC: usize = 0o1000;

// Syscall wrappers
#[inline(always)]
fn syscall1(id: usize, arg0: usize) -> usize {
    let ret;
    unsafe {
        core::arch::asm!(
            "mv a7, {0}",
            "mv a0, {1}",
            "ecall",
            in(reg) id,
            in(reg) arg0,
            lateout("a0") ret
        );
    }
    ret
}

#[inline(always)]
fn syscall2(id: usize, arg0: usize, arg1: usize) -> usize {
    let ret;
    unsafe {
        core::arch::asm!(
            "mv a7, {0}",
            "mv a0, {1}",
            "mv a1, {2}",
            "ecall",
            in(reg) id,
            in(reg) arg0,
            in(reg) arg1,
            lateout("a0") ret
        );
    }
    ret
}

#[inline(always)]
fn syscall3(id: usize, arg0: usize, arg1: usize, arg2: usize) -> usize {
    let ret;
    unsafe {
        core::arch::asm!(
            "mv a7, {0}",
            "mv a0, {1}",
            "mv a1, {2}",
            "mv a2, {3}",
            "ecall",
            in(reg) id,
            in(reg) arg0,
            in(reg) arg1,
            in(reg) arg2,
            lateout("a0") ret
        );
    }
    ret
}

#[inline(always)]
fn syscall4(id: usize, arg0: usize, arg1: usize, arg2: usize, arg3: usize) -> usize {
    let ret;
    unsafe {
        core::arch::asm!(
            "mv a7, {0}",
            "mv a0, {1}",
            "mv a1, {2}",
            "mv a2, {3}",
            "mv a3, {4}",
            "ecall",
            in(reg) id,
            in(reg) arg0,
            in(reg) arg1,
            in(reg) arg2,
            in(reg) arg3,
            lateout("a0") ret
        );
    }
    ret
}

fn write(fd: usize, buf: *const u8, count: usize) -> usize {
    syscall3(SYS_WRITE, fd, buf as usize, count)
}

fn read(fd: usize, buf: *mut u8, count: usize) -> usize {
    syscall3(SYS_READ, fd, buf as usize, count)
}

fn openat(dirfd: isize, path: *const u8, flags: usize, mode: usize) -> isize {
    syscall4(SYS_OPENAT, dirfd as usize, path as usize, flags, mode) as isize
}

fn close(fd: usize) -> usize {
    syscall1(SYS_CLOSE, fd)
}

fn exit(code: usize) -> ! {
    syscall1(SYS_EXIT, code);
    loop {}
}

// Putchar
fn putc(c: u8) {
    write(STDOUT, &c, 1);
}

// Write string
fn puts(s: &[u8]) {
    let mut i = 0;
    while i < s.len() {
        putc(s[i]);
        i += 1;
    }
}

// Puthex - print hex number
fn puthex(n: usize) {
    if n == 0 {
        putc(b'0');
        return;
    }
    let mut buf = [0u8; 16];
    let mut i = 0;
    let mut x = n;
    while x > 0 {
        let d = (x % 16) as u8;
        buf[i] = if d < 10 { b'0' + d } else { b'a' + d - 10 };
        x /= 16;
        i += 1;
    }
    while i > 0 {
        i -= 1;
        putc(buf[i]);
    }
}

// Editor constants
const MAX_LINES: usize = 1024;
const MAX_LINE_LEN: usize = 256;
const MAX_FILENAME: usize = 128;

// Editor state
struct Editor {
    // Lines of text
    lines: [[u8; MAX_LINE_LEN]; MAX_LINES],
    line_count: usize,
    // Cursor position
    cursor_row: usize,
    cursor_col: usize,
    // Screen position
    screen_row: usize,
    screen_col: usize,
    // Mode (0=command, 1=insert, 2=ex)
    mode: u8,
    // Filename
    filename: [u8; MAX_FILENAME],
    filename_len: usize,
    // Modified flag
    modified: bool,
    // Command buffer for ex mode
    cmd_buf: [u8; MAX_LINE_LEN],
    cmd_len: usize,
    // Search pattern
    search_pattern: [u8; MAX_LINE_LEN],
    search_len: usize,
    search_found: bool,
}

impl Editor {
    fn new() -> Self {
        let mut editor = Self {
            lines: [[0u8; MAX_LINE_LEN]; MAX_LINES],
            line_count: 1,
            cursor_row: 0,
            cursor_col: 0,
            screen_row: 0,
            screen_col: 0,
            mode: 0,  // Command mode
            filename: [0u8; MAX_FILENAME],
            filename_len: 0,
            modified: false,
            cmd_buf: [0u8; MAX_LINE_LEN],
            cmd_len: 0,
            search_pattern: [0u8; MAX_LINE_LEN],
            search_len: 0,
            search_found: false,
        };
        // Initialize first line as empty
        editor.lines[0][0] = 0;
        editor
    }

    // Set filename
    fn set_filename(&mut self, name: *const u8) {
        let mut i = 0;
        loop {
            let c = unsafe { *name.add(i) };
            self.filename[i] = c;
            if c == 0 || i >= MAX_FILENAME - 1 { break; }
            i += 1;
        }
        self.filename_len = i;
        self.filename[i] = 0;
    }

    // Insert a character at cursor position
    fn insert_char(&mut self, c: u8) {
        if self.cursor_row >= MAX_LINES {
            return;
        }
        let line_len = self.get_line_len(self.cursor_row);
        if line_len >= MAX_LINE_LEN - 1 {
            return;
        }

        // Shift characters right
        let mut i = line_len;
        while i > self.cursor_col {
            self.lines[self.cursor_row][i] = self.lines[self.cursor_row][i - 1];
            i -= 1;
        }
        self.lines[self.cursor_row][self.cursor_col] = c;
        self.lines[self.cursor_row][line_len + 1] = 0;
        self.cursor_col += 1;
        self.modified = true;
    }

    // Delete character at cursor position
    fn delete_char(&mut self) {
        if self.cursor_col < self.get_line_len(self.cursor_row) {
            let line_len = self.get_line_len(self.cursor_row);
            let mut i = self.cursor_col;
            while i < line_len - 1 {
                self.lines[self.cursor_row][i] = self.lines[self.cursor_row][i + 1];
                i += 1;
            }
            self.lines[self.cursor_row][i] = 0;
            self.modified = true;
        } else if self.cursor_row < self.line_count - 1 {
            // Join with next line
            self.join_lines(self.cursor_row);
            self.modified = true;
        }
    }

    // Backspace at cursor
    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            self.delete_char();
        } else if self.cursor_row > 0 {
            // Move to end of previous line
            self.cursor_row -= 1;
            self.cursor_col = self.get_line_len(self.cursor_row);
            self.delete_char();
        }
    }

    // New line
    fn newline(&mut self) {
        if self.cursor_row >= MAX_LINES - 1 {
            return;
        }

        let current_len = self.get_line_len(self.cursor_row);

        // Shift remaining lines down
        let mut i = self.line_count;
        while i > self.cursor_row + 1 {
            self.copy_line(i - 1, i);
            i -= 1;
        }
        self.line_count += 1;

        // Split current line
        let remainder_len = current_len - self.cursor_col;
        for j in 0..remainder_len {
            self.lines[self.cursor_row + 1][j] = self.lines[self.cursor_row][self.cursor_col + j];
        }
        self.lines[self.cursor_row + 1][remainder_len] = 0;
        self.lines[self.cursor_row][self.cursor_col] = 0;

        self.cursor_row += 1;
        self.cursor_col = 0;
        self.modified = true;
    }

    // Join line at row with next line
    fn join_lines(&mut self, row: usize) {
        if row >= self.line_count - 1 {
            return;
        }
        let current_len = self.get_line_len(row);
        let next_len = self.get_line_len(row + 1);

        if current_len + next_len >= MAX_LINE_LEN {
            return;
        }

        // Append next line to current
        for i in 0..next_len {
            self.lines[row][current_len + i] = self.lines[row + 1][i];
        }
        self.lines[row][current_len + next_len] = 0;

        // Shift remaining lines up
        let mut i = row + 1;
        while i < self.line_count - 1 {
            self.copy_line(i + 1, i);
            i += 1;
        }
        self.line_count -= 1;
    }

    // Get line length
    fn get_line_len(&self, row: usize) -> usize {
        let mut len = 0;
        while len < MAX_LINE_LEN && self.lines[row][len] != 0 {
            len += 1;
        }
        len
    }

    // Copy line
    fn copy_line(&mut self, src: usize, dst: usize) {
        for i in 0..MAX_LINE_LEN {
            self.lines[dst][i] = self.lines[src][i];
        }
    }

    // Move cursor
    fn move_cursor(&mut self, dir: u8) {
        match dir {
            b'h' => {  // Left
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                }
            }
            b'l' => {  // Right
                if self.cursor_col < self.get_line_len(self.cursor_row) {
                    self.cursor_col += 1;
                }
            }
            b'j' => {  // Down
                if self.cursor_row < self.line_count - 1 {
                    self.cursor_row += 1;
                    if self.cursor_col > self.get_line_len(self.cursor_row) {
                        self.cursor_col = self.get_line_len(self.cursor_row);
                    }
                }
            }
            b'k' => {  // Up
                if self.cursor_row > 0 {
                    self.cursor_row -= 1;
                    if self.cursor_col > self.get_line_len(self.cursor_row) {
                        self.cursor_col = self.get_line_len(self.cursor_row);
                    }
                }
            }
            b'0' => {  // Beginning of line
                self.cursor_col = 0;
            }
            b'$' => {  // End of line
                self.cursor_col = self.get_line_len(self.cursor_row);
            }
            b'G' => {  // Go to last line
                self.cursor_row = if self.line_count > 0 { self.line_count - 1 } else { 0 };
                self.cursor_col = 0;
            }
            b'g' => {  // gg - go to first line (need to check next char)
                self.cursor_row = 0;
                self.cursor_col = 0;
            }
            b'^' => {  // First non-whitespace
                let mut col = 0;
                let len = self.get_line_len(self.cursor_row);
                while col < len && self.lines[self.cursor_row][col] == b' ' {
                    col += 1;
                }
                self.cursor_col = col;
            }
            _ => {}
        }
    }

    // Search for pattern
    fn search(&mut self) {
        if self.search_len == 0 {
            return;
        }

        // Search from current position
        let mut row = self.cursor_row;
        let mut found = false;

        for _ in 0..MAX_LINES {
            let line_len = self.get_line_len(row);
            let pat_len = self.search_len;

            if line_len >= pat_len {
                for col in 0..=(line_len - pat_len) {
                    let mut match_found = true;
                    for p in 0..pat_len {
                        if self.lines[row][col + p] != self.search_pattern[p] {
                            match_found = false;
                            break;
                        }
                    }
                    if match_found {
                        self.cursor_row = row;
                        self.cursor_col = col;
                        found = true;
                        break;
                    }
                }
            }
            if found { break; }

            row = (row + 1) % MAX_LINES;
            if row == self.cursor_row { break; }
        }

        self.search_found = found;
        if found {
            puts(b"/Pattern found\n");
        } else {
            puts(b"/Pattern not found\n");
        }
    }

    // Save file
    fn save(&mut self) -> bool {
        if self.filename_len == 0 {
            return false;
        }

        let fd = openat(-100, self.filename.as_ptr(), O_CREAT | O_WRONLY | O_TRUNC, 0o644);
        if fd < 0 {
            puts(b"Error: Cannot open file for writing\n");
            return false;
        }

        let mut i = 0;
        while i < self.line_count {
            let line = &self.lines[i];
            let mut j = 0;
            while j < MAX_LINE_LEN && line[j] != 0 {
                let buf = [line[j]];
                write(fd as usize, buf.as_ptr(), 1);
                j += 1;
            }
            if i < self.line_count - 1 {
                let nl = [b'\n'];
                write(fd as usize, nl.as_ptr(), 1);
            }
            i += 1;
        }

        close(fd as usize);
        self.modified = false;
        puts(b"File saved\n");
        true
    }

    // Load file
    fn load(&mut self) -> bool {
        if self.filename_len == 0 {
            return false;
        }

        let fd = openat(-100, self.filename.as_ptr(), O_RDONLY, 0);
        if fd < 0 {
            puts(b"Error: Cannot open file\n");
            return false;
        }

        // Reset editor
        self.line_count = 0;
        self.cursor_row = 0;
        self.cursor_col = 0;

        let mut buf = [0u8; MAX_LINE_LEN];
        let mut buf_pos = 0;
        let mut ch: u8 = 0;

        loop {
            let n = read(fd as usize, &mut ch, 1);
            if n == 0 {
                break;  // EOF
            }

            if ch == b'\n' || buf_pos >= MAX_LINE_LEN - 1 {
                // Save line
                if self.line_count < MAX_LINES {
                    let mut j = 0;
                    while j < buf_pos && j < MAX_LINE_LEN - 1 {
                        self.lines[self.line_count][j] = buf[j];
                        j += 1;
                    }
                    self.lines[self.line_count][j] = 0;
                    self.line_count += 1;
                }
                buf_pos = 0;
            } else {
                buf[buf_pos] = ch;
                buf_pos += 1;
            }
        }

        // Handle last line without newline
        if buf_pos > 0 && self.line_count < MAX_LINES {
            let mut j = 0;
            while j < buf_pos && j < MAX_LINE_LEN - 1 {
                self.lines[self.line_count][j] = buf[j];
                j += 1;
            }
            self.lines[self.line_count][j] = 0;
            self.line_count += 1;
        }

        close(fd as usize);

        // Ensure at least one line
        if self.line_count == 0 {
            self.line_count = 1;
            self.lines[0][0] = 0;
        }

        self.modified = false;
        true
    }

    // Display status line
    fn status_line(&self) {
        puts(b"\r[");  // Return to beginning of line

        // Filename
        let mut i = 0;
        while i < self.filename_len {
            putc(self.filename[i]);
            i += 1;
        }
        if self.filename_len == 0 {
            puts(b"[No Name]");
        }

        puts(b"] ");

        // Modified flag
        if self.modified {
            putc(b'+');
        }

        // Line info
        puts(b" - ");
        putc(b'0' + (((self.cursor_row + 1) / 100) % 10) as u8);
        putc(b'0' + (((self.cursor_row + 1) / 10) % 10) as u8);
        putc(b'0' + ((self.cursor_row + 1) % 10) as u8);
        putc(b',');
        putc(b'0' + (((self.cursor_col + 1) / 10) % 10) as u8);
        putc(b'0' + ((self.cursor_col + 1) % 10) as u8);

        // Mode indicator
        match self.mode {
            0 => puts(b" [COMMAND]"),
            1 => puts(b" [INSERT]"),
            2 => puts(b" [EX]"),
            _ => {}
        }

        // Clear rest of line
        for _ in 0..60 {
            putc(b' ');
        }
        putc(b'\r');
    }

    // Display screen
    fn display(&self) {
        // Clear screen (simple)
        puts(b"\x1b[2J\x1b[H");  // VT100 clear

        // Calculate screen start based on cursor
        let screen_start = if self.cursor_row >= 20 {
            self.cursor_row - 20
        } else {
            0
        };

        // Display lines
        let mut row = screen_start;
        let max_display = 24.min(self.line_count);
        while row < max_display {
            // Line number
            putc(b' ');
            putc(b'0' + (((row + 1) / 100) % 10) as u8);
            putc(b'0' + (((row + 1) / 10) % 10) as u8);
            putc(b'0' + ((row + 1) % 10) as u8);
            putc(b' ');

            // Line content
            let mut col = 0;
            while col < 70 && self.lines[row][col] != 0 {
                if row == self.cursor_row && col == self.cursor_col {
                    putc(b'|');  // Cursor
                } else {
                    putc(self.lines[row][col]);
                }
                col += 1;
            }
            if row == self.cursor_row && col == self.cursor_col {
                putc(b'|');
            }
            putc(b'\n');
            row += 1;
        }

        // Status line
        self.status_line();
    }

    // Handle escape sequences
    fn handle_escape(&mut self, c: u8) -> bool {
        // Simple escape - just enter command mode
        if c == b'[' || c == b'O' {
            return true;  // More characters may follow
        }
        // If we get here, escape sequence is complete
        self.mode = 0;  // Command mode
        false
    }
}

// Main editor loop
#[no_mangle]
extern "C" fn _start() {
    let mut editor = Editor::new();

    // Check for filename in args (simple)
    // In a full implementation, we'd parse argv

    // Welcome message
    puts(b"\x1b[2J\x1b[H");  // Clear screen
    puts(b"TrainOS vi-like Editor\n");
    puts(b"Commands:\n");
    puts(b"  i - enter insert mode\n");
    puts(b"  a - append after cursor\n");
    puts(b"  o - open new line below\n");
    puts(b"  O - open new line above\n");
    puts(b"  x - delete character\n");
    puts(b"  dd - delete line\n");
    puts(b"  yy - yank line\n");
    puts(b"  p - paste\n");
    puts(b"  u - undo\n");
    puts(b"  / - search\n");
    puts(b"  : - ex command\n");
    puts(b"  h/j/k/l - cursor movement\n");
    puts(b"  0/$ - beginning/end of line\n");
    puts(b"  G - go to last line\n");
    puts(b"  ZZ - save and quit\n");
    puts(b"  ESC - return to command mode\n");
    puts(b"\nPress ENTER to start...\n");

    // Initial display
    editor.display();

    // Main loop
    loop {
        let mut c: u8 = 0;
        let n = read(STDIN, &mut c, 1);

        if n == 0 {
            continue;
        }

        match editor.mode {
            0 => {  // Command mode
                match c {
                    b'i' => { editor.mode = 1; }  // Insert mode
                    b'a' => {  // Append after cursor
                        if editor.cursor_col < editor.get_line_len(editor.cursor_row) {
                            editor.cursor_col += 1;
                        }
                        editor.mode = 1;
                    }
                    b'A' => {  // Append at end of line
                        editor.cursor_col = editor.get_line_len(editor.cursor_row);
                        editor.mode = 1;
                    }
                    b'o' => {  // Open new line below
                        editor.cursor_row += 1;
                        if editor.cursor_row >= editor.line_count && editor.cursor_row < MAX_LINES {
                            editor.lines[editor.cursor_row][0] = 0;
                            editor.line_count = editor.cursor_row + 1;
                        }
                        editor.cursor_col = 0;
                        editor.mode = 1;
                    }
                    b'O' => {  // Open new line above
                        if editor.cursor_row > 0 {
                            editor.cursor_row -= 1;
                        }
                        editor.cursor_col = 0;
                        editor.mode = 1;
                    }
                    b'x' => { editor.delete_char(); }
                    b'h' | b'j' | b'k' | b'l' | b'0' | b'$' | b'^' | b'G' | b'g' => {
                        editor.move_cursor(c);
                    }
                    b'/' => {
                        editor.mode = 2;  // Search mode
                        editor.cmd_len = 0;
                        puts(b"\n/");
                    }
                    b':' => {
                        editor.mode = 2;  // Ex mode
                        editor.cmd_len = 0;
                        puts(b"\n:");
                    }
                    27 => {  // ESC
                        // Handle escape sequences
                    }
                    b'Z' => {
                        // Need to peek next char
                        let mut c2: u8 = 0;
                        if read(STDIN, &mut c2, 1) > 0 && c2 == b'Z' {
                            // Save and quit
                            if editor.save() {
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
            1 => {  // Insert mode
                match c {
                    27 => {  // ESC
                        editor.mode = 0;
                    }
                    127 | 8 => {  // Backspace
                        editor.backspace();
                    }
                    b'\n' | b'\r' => {
                        editor.newline();
                    }
                    _ => {
                        if c >= 32 && c < 127 {
                            editor.insert_char(c);
                        }
                    }
                }
            }
            2 => {  // Ex command mode
                match c {
                    27 => {  // ESC
                        editor.mode = 0;
                    }
                    b'\n' | b'\r' => {
                        // Execute command
                        editor.cmd_buf[editor.cmd_len] = 0;

                        if editor.cmd_len > 0 {
                            // Extract command bytes to avoid borrow conflict
                            let cmd0 = editor.cmd_buf[0];
                            let cmd1 = editor.cmd_buf[1];

                            // Parse command
                            if cmd0 == b'w' && (cmd1 == b' ' || cmd1 == b'q' || cmd1 == 0) {
                                // :w or :wq
                                if editor.filename_len == 0 && cmd1 == b' ' {
                                    // Need filename
                                    // For now, use default
                                }
                                if editor.save() {
                                    if cmd1 == b'q' {
                                        break;  // Quit after save
                                    }
                                }
                            } else if cmd0 == b'q' {
                                // :q - quit
                                if !editor.modified {
                                    break;
                                } else {
                                    puts(b"\nNo write since last change\n");
                                }
                            } else if cmd0 == b'q' && cmd1 == b'!' {
                                // :q! - force quit
                                break;
                            } else if cmd0 == b'e' && cmd1 == b' ' {
                                // :e filename - edit file
                                let mut i = 2;
                                while i < editor.cmd_len && editor.cmd_buf[i] != 0 {
                                    editor.filename[i - 2] = editor.cmd_buf[i];
                                    i += 1;
                                }
                                editor.filename[i - 2] = 0;
                                editor.filename_len = i - 2;
                                editor.load();
                            } else if cmd0 == b's' && cmd1 == b'/' {
                                // :s/old/new - substitute
                                puts(b"\nSubstitute not implemented\n");
                            } else if cmd0 == b'n' {
                                // :n - next file (not implemented)
                            } else if cmd0 == b's' {
                                // :set (ignore)
                            } else {
                                puts(b"\nUnknown command\n");
                            }
                        }
                        editor.mode = 0;
                    }
                    127 => {  // Backspace in command
                        if editor.cmd_len > 0 {
                            editor.cmd_len -= 1;
                            putc(8); putc(b' '); putc(8);
                        }
                    }
                    _ => {
                        if c >= 32 && c < 127 && editor.cmd_len < MAX_LINE_LEN - 1 {
                            editor.cmd_buf[editor.cmd_len] = c;
                            editor.cmd_len += 1;
                            putc(c);
                        }
                    }
                }
            }
            _ => {}
        }

        // Redisplay
        editor.display();
    }

    puts(b"\nGoodbye!\n");
    exit(0);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    puts(b"\nPanic in vi editor!\n");
    exit(1);
}
