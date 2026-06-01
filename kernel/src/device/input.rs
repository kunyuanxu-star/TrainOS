// V37b — Input Handling
//
// Provides input event types, a keyboard state tracker,
// scancode-to-ASCII conversion, and an input event queue
// for the GUI subsystem.

// ── Event Types ────────────────────────────────────────────────────────────

/// Input event types for the GUI subsystem.
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum InputEvent {
    /// A key was pressed.
    KeyDown { keycode: u8, modifier: u8 },
    /// A key was released.
    KeyUp { keycode: u8, modifier: u8 },
    /// The mouse was moved.
    MouseMove { x: i32, y: i32 },
    /// A mouse button was pressed.
    MouseDown { x: i32, y: i32, button: u8 },
    /// A mouse button was released.
    MouseUp { x: i32, y: i32, button: u8 },
    /// Scroll wheel moved.
    MouseWheel { delta: i32 },
    /// Display was resized.
    Resize { width: u32, height: u32 },
}

impl InputEvent {
    /// Serialize the event into a byte buffer.
    /// Format: [type:1] [data...]
    /// Returns the number of bytes written.
    pub fn serialize(&self, buf: &mut [u8; 16]) -> usize {
        match *self {
            InputEvent::KeyDown { keycode, modifier } => {
                buf[0] = 1;
                buf[1] = keycode;
                buf[2] = modifier;
                3
            }
            InputEvent::KeyUp { keycode, modifier } => {
                buf[0] = 2;
                buf[1] = keycode;
                buf[2] = modifier;
                3
            }
            InputEvent::MouseMove { x, y } => {
                buf[0] = 3;
                buf[1..5].copy_from_slice(&(x as u32).to_le_bytes());
                buf[5..9].copy_from_slice(&(y as u32).to_le_bytes());
                9
            }
            InputEvent::MouseDown { x, y, button } => {
                buf[0] = 4;
                buf[1] = button;
                buf[2..6].copy_from_slice(&(x as u32).to_le_bytes());
                buf[6..10].copy_from_slice(&(y as u32).to_le_bytes());
                10
            }
            InputEvent::MouseUp { x, y, button } => {
                buf[0] = 5;
                buf[1] = button;
                buf[2..6].copy_from_slice(&(x as u32).to_le_bytes());
                buf[6..10].copy_from_slice(&(y as u32).to_le_bytes());
                10
            }
            InputEvent::MouseWheel { delta } => {
                buf[0] = 6;
                buf[1..5].copy_from_slice(&(delta as u32).to_le_bytes());
                5
            }
            InputEvent::Resize { width, height } => {
                buf[0] = 7;
                buf[1..5].copy_from_slice(&width.to_le_bytes());
                buf[5..9].copy_from_slice(&height.to_le_bytes());
                9
            }
        }
    }
}

// ── Key Codes (USB HID scancode subset) ────────────────────────────────────

// These match the standard USB HID key usage codes commonly
// used by QEMU's virtio-input or PS/2 keyboard emulation.

pub const KEY_NONE: u8 = 0;
pub const KEY_ESCAPE: u8 = 1;
pub const KEY_1: u8 = 2;
pub const KEY_2: u8 = 3;
pub const KEY_3: u8 = 4;
pub const KEY_4: u8 = 5;
pub const KEY_5: u8 = 6;
pub const KEY_6: u8 = 7;
pub const KEY_7: u8 = 8;
pub const KEY_8: u8 = 9;
pub const KEY_9: u8 = 10;
pub const KEY_0: u8 = 11;
pub const KEY_MINUS: u8 = 12;
pub const KEY_EQUAL: u8 = 13;
pub const KEY_BACKSPACE: u8 = 14;
pub const KEY_TAB: u8 = 15;
pub const KEY_Q: u8 = 16;
pub const KEY_W: u8 = 17;
pub const KEY_E: u8 = 18;
pub const KEY_R: u8 = 19;
pub const KEY_T: u8 = 20;
pub const KEY_Y: u8 = 21;
pub const KEY_U: u8 = 22;
pub const KEY_I: u8 = 23;
pub const KEY_O: u8 = 24;
pub const KEY_P: u8 = 25;
pub const KEY_BRACKET_LEFT: u8 = 26;
pub const KEY_BRACKET_RIGHT: u8 = 27;
pub const KEY_ENTER: u8 = 28;
pub const KEY_CTRL: u8 = 29;
pub const KEY_A: u8 = 30;
pub const KEY_S: u8 = 31;
pub const KEY_D: u8 = 32;
pub const KEY_F: u8 = 33;
pub const KEY_G: u8 = 34;
pub const KEY_H: u8 = 35;
pub const KEY_J: u8 = 36;
pub const KEY_K: u8 = 37;
pub const KEY_L: u8 = 38;
pub const KEY_SEMICOLON: u8 = 39;
pub const KEY_QUOTE: u8 = 40;
pub const KEY_BACKTICK: u8 = 41;
pub const KEY_SHIFT: u8 = 42;
pub const KEY_BACKSLASH: u8 = 43;
pub const KEY_Z: u8 = 44;
pub const KEY_X: u8 = 45;
pub const KEY_C: u8 = 46;
pub const KEY_V: u8 = 47;
pub const KEY_B: u8 = 48;
pub const KEY_N: u8 = 49;
pub const KEY_M: u8 = 50;
pub const KEY_COMMA: u8 = 51;
pub const KEY_PERIOD: u8 = 52;
pub const KEY_SLASH: u8 = 53;
pub const KEY_SHIFT_R: u8 = 54;
pub const KEY_KP_MULTIPLY: u8 = 55;
pub const KEY_ALT: u8 = 56;
pub const KEY_SPACE: u8 = 57;
pub const KEY_CAPS: u8 = 58;
pub const KEY_F1: u8 = 59;
pub const KEY_F2: u8 = 60;
pub const KEY_F3: u8 = 61;
pub const KEY_F4: u8 = 62;
pub const KEY_F5: u8 = 63;
pub const KEY_F6: u8 = 64;
pub const KEY_F7: u8 = 65;
pub const KEY_F8: u8 = 66;
pub const KEY_F9: u8 = 67;
pub const KEY_F10: u8 = 68;
pub const KEY_F11: u8 = 69;
pub const KEY_F12: u8 = 70;
pub const KEY_UP: u8 = 82;
pub const KEY_DOWN: u8 = 81;
pub const KEY_LEFT: u8 = 80;
pub const KEY_RIGHT: u8 = 79;

// ── Modifier Bits ──────────────────────────────────────────────────────────

pub const MOD_NONE: u8 = 0;
pub const MOD_SHIFT: u8 = 1;
pub const MOD_CTRL: u8 = 2;
pub const MOD_ALT: u8 = 4;
pub const MOD_CAPS: u8 = 8;

// ── Mouse Buttons ──────────────────────────────────────────────────────────

pub const MOUSE_LEFT: u8 = 1;
pub const MOUSE_RIGHT: u8 = 2;
pub const MOUSE_MIDDLE: u8 = 3;

// ── Input Event Queue ──────────────────────────────────────────────────────

/// Maximum number of buffered input events.
const INPUT_QUEUE_SIZE: usize = 256;

/// A circular buffer of input events.
pub struct InputQueue {
    events: [u64; INPUT_QUEUE_SIZE],
    head: usize,
    tail: usize,
}

impl InputQueue {
    /// Create a new empty input queue.
    pub const fn new() -> Self {
        InputQueue {
            events: [0u64; INPUT_QUEUE_SIZE],
            head: 0,
            tail: 0,
        }
    }

    /// Push an event into the queue.
    pub fn push(&mut self, event: InputEvent) {
        let next = (self.tail + 1) % INPUT_QUEUE_SIZE;
        if next == self.head {
            // Queue full — drop oldest
            self.head = (self.head + 1) % INPUT_QUEUE_SIZE;
        }
        // Pack event into a u64 for storage
        let packed = match event {
            InputEvent::KeyDown { keycode, modifier } => {
                1u64 | (keycode as u64) << 8 | (modifier as u64) << 16
            }
            InputEvent::KeyUp { keycode, modifier } => {
                2u64 | (keycode as u64) << 8 | (modifier as u64) << 16
            }
            InputEvent::MouseMove { x, y } => {
                3u64 | ((x as u64) & 0xFFFFFFFF) << 16 | ((y as u64) & 0xFFFFFFFF) << 48
            }
            _ => 0, // Other events not packed (simplification)
        };
        self.events[self.tail] = packed;
        self.tail = next;
    }

    /// Pop an event from the queue.
    pub fn pop(&mut self) -> Option<InputEvent> {
        if self.head == self.tail {
            return None;
        }
        let packed = self.events[self.head];
        self.head = (self.head + 1) % INPUT_QUEUE_SIZE;

        let event_type = packed & 0xFF;
        match event_type {
            1 => {
                let keycode = (packed >> 8) as u8;
                let modifier = (packed >> 16) as u8;
                Some(InputEvent::KeyDown { keycode, modifier })
            }
            2 => {
                let keycode = (packed >> 8) as u8;
                let modifier = (packed >> 16) as u8;
                Some(InputEvent::KeyUp { keycode, modifier })
            }
            3 => {
                let x = ((packed >> 16) & 0xFFFFFFFF) as i32;
                let y = ((packed >> 48) & 0xFFFFFFFF) as i32;
                Some(InputEvent::MouseMove { x, y })
            }
            _ => None,
        }
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    /// Return the number of events in the queue.
    pub fn len(&self) -> usize {
        if self.tail >= self.head {
            self.tail - self.head
        } else {
            INPUT_QUEUE_SIZE - self.head + self.tail
        }
    }
}

// ── Keyboard State ─────────────────────────────────────────────────────────

/// Tracks the current state of the keyboard (modifiers, keys down).
pub struct KeyboardState {
    shift_pressed: bool,
    ctrl_pressed: bool,
    alt_pressed: bool,
    caps_locked: bool,
    keys_down: [bool; 256],
}

impl KeyboardState {
    /// Create a new keyboard state tracker.
    pub const fn new() -> Self {
        KeyboardState {
            shift_pressed: false,
            ctrl_pressed: false,
            alt_pressed: false,
            caps_locked: false,
            keys_down: [false; 256],
        }
    }

    /// Handle a key-down event and return the current modifier byte.
    pub fn key_down(&mut self, keycode: u8) -> u8 {
        self.keys_down[keycode as usize] = true;
        match keycode {
            KEY_SHIFT | KEY_SHIFT_R => self.shift_pressed = true,
            KEY_CTRL => self.ctrl_pressed = true,
            KEY_ALT => self.alt_pressed = true,
            KEY_CAPS => {
                self.caps_locked = !self.caps_locked;
            }
            _ => {}
        }
        self.modifier()
    }

    /// Handle a key-up event and return the current modifier byte.
    pub fn key_up(&mut self, keycode: u8) -> u8 {
        self.keys_down[keycode as usize] = false;
        match keycode {
            KEY_SHIFT | KEY_SHIFT_R => self.shift_pressed = false,
            KEY_CTRL => self.ctrl_pressed = false,
            KEY_ALT => self.alt_pressed = false,
            _ => {}
        }
        self.modifier()
    }

    /// Return the current modifier mask.
    pub fn modifier(&self) -> u8 {
        let mut m = MOD_NONE;
        if self.shift_pressed { m |= MOD_SHIFT; }
        if self.ctrl_pressed { m |= MOD_CTRL; }
        if self.alt_pressed { m |= MOD_ALT; }
        if self.caps_locked { m |= MOD_CAPS; }
        m
    }

    /// Check if a specific key is currently held down.
    pub fn is_key_down(&self, keycode: u8) -> bool {
        self.keys_down[keycode as usize]
    }
}

// ── Scancode-to-ASCII Conversion ───────────────────────────────────────────

/// Convert a keycode to an ASCII character, considering shift state.
pub fn keycode_to_ascii(keycode: u8, shift: bool) -> Option<char> {
    match keycode {
        KEY_SPACE => Some(' '),
        KEY_0 => Some(if shift { ')' } else { '0' }),
        KEY_1 => Some(if shift { '!' } else { '1' }),
        KEY_2 => Some(if shift { '@' } else { '2' }),
        KEY_3 => Some(if shift { '#' } else { '3' }),
        KEY_4 => Some(if shift { '$' } else { '4' }),
        KEY_5 => Some(if shift { '%' } else { '5' }),
        KEY_6 => Some(if shift { '^' } else { '6' }),
        KEY_7 => Some(if shift { '&' } else { '7' }),
        KEY_8 => Some(if shift { '*' } else { '8' }),
        KEY_9 => Some(if shift { '(' } else { '9' }),
        KEY_MINUS => Some(if shift { '_' } else { '-' }),
        KEY_EQUAL => Some(if shift { '+' } else { '=' }),
        KEY_BRACKET_LEFT => Some(if shift { '{' } else { '[' }),
        KEY_BRACKET_RIGHT => Some(if shift { '}' } else { ']' }),
        KEY_SEMICOLON => Some(if shift { ':' } else { ';' }),
        KEY_QUOTE => Some(if shift { '"' } else { '\'' }),
        KEY_BACKTICK => Some(if shift { '~' } else { '`' }),
        KEY_BACKSLASH => Some(if shift { '|' } else { '\\' }),
        KEY_COMMA => Some(if shift { '<' } else { ',' }),
        KEY_PERIOD => Some(if shift { '>' } else { '.' }),
        KEY_SLASH => Some(if shift { '?' } else { '/' }),
        KEY_TAB => Some('\t'),
        KEY_ENTER => Some('\n'),
        KEY_BACKSPACE => Some(0x7f as char), // DEL marker

        // Letters
        KEY_A => Some(if shift ^ false { 'A' } else { 'a' }),
        KEY_B => Some(if shift { 'B' } else { 'b' }),
        KEY_C => Some(if shift { 'C' } else { 'c' }),
        KEY_D => Some(if shift { 'D' } else { 'd' }),
        KEY_E => Some(if shift { 'E' } else { 'e' }),
        KEY_F => Some(if shift { 'F' } else { 'f' }),
        KEY_G => Some(if shift { 'G' } else { 'g' }),
        KEY_H => Some(if shift { 'H' } else { 'h' }),
        KEY_I => Some(if shift { 'I' } else { 'i' }),
        KEY_J => Some(if shift { 'J' } else { 'j' }),
        KEY_K => Some(if shift { 'K' } else { 'k' }),
        KEY_L => Some(if shift { 'L' } else { 'l' }),
        KEY_M => Some(if shift { 'M' } else { 'm' }),
        KEY_N => Some(if shift { 'N' } else { 'n' }),
        KEY_O => Some(if shift { 'O' } else { 'o' }),
        KEY_P => Some(if shift { 'P' } else { 'p' }),
        KEY_Q => Some(if shift { 'Q' } else { 'q' }),
        KEY_R => Some(if shift { 'R' } else { 'r' }),
        KEY_S => Some(if shift { 'S' } else { 's' }),
        KEY_T => Some(if shift { 'T' } else { 't' }),
        KEY_U => Some(if shift { 'U' } else { 'u' }),
        KEY_V => Some(if shift { 'V' } else { 'v' }),
        KEY_W => Some(if shift { 'W' } else { 'w' }),
        KEY_X => Some(if shift { 'X' } else { 'x' }),
        KEY_Y => Some(if shift { 'Y' } else { 'y' }),
        KEY_Z => Some(if shift { 'Z' } else { 'z' }),

        _ => None,
    }
}

/// Convert a keycode to an ASCII character with caps lock awareness.
pub fn keycode_to_ascii_caps(keycode: u8, shift: bool, caps: bool) -> Option<char> {
    let shifted = shift ^ caps;
    keycode_to_ascii(keycode, shifted)
}

// ── Global Input State ─────────────────────────────────────────────────────

/// Global input event queue.
static mut INPUT_QUEUE: InputQueue = InputQueue::new();

/// Global keyboard state.
static mut KEYBOARD_STATE: KeyboardState = KeyboardState::new();

/// Push an input event to the global queue.
pub fn input_push(event: InputEvent) {
    unsafe {
        INPUT_QUEUE.push(event);
    }
}

/// Pop an input event from the global queue.
pub fn input_pop() -> Option<InputEvent> {
    unsafe { INPUT_QUEUE.pop() }
}

/// Check if input is available.
pub fn input_available() -> bool {
    unsafe { !INPUT_QUEUE.is_empty() }
}

/// Get current modifier state.
pub fn current_modifier() -> u8 {
    unsafe { KEYBOARD_STATE.modifier() }
}

/// Process a key-down event (updates keyboard state and queues the event).
pub fn process_key_down(keycode: u8) {
    unsafe {
        let modifier = KEYBOARD_STATE.key_down(keycode);
        INPUT_QUEUE.push(InputEvent::KeyDown { keycode, modifier });
    }
}

/// Process a key-up event.
pub fn process_key_up(keycode: u8) {
    unsafe {
        let modifier = KEYBOARD_STATE.key_up(keycode);
        INPUT_QUEUE.push(InputEvent::KeyUp { keycode, modifier });
    }
}

/// Process a mouse-move event.
pub fn process_mouse_move(x: i32, y: i32) {
    input_push(InputEvent::MouseMove { x, y });
}

/// Process a mouse-button-down event.
pub fn process_mouse_down(x: i32, y: i32, button: u8) {
    input_push(InputEvent::MouseDown { x, y, button });
}

/// Process a mouse-button-up event.
pub fn process_mouse_up(x: i32, y: i32, button: u8) {
    input_push(InputEvent::MouseUp { x, y, button });
}
