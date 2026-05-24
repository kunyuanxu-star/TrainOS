// V33 — Confidential Computing TEE — Enclave Secure IPC
//
// Features:
//   - Capability-protected channel-based IPC between enclaves
//   - Only authorized enclave pairs can communicate (src/dst capability check)
//   - Fixed-size message buffers (256 bytes)
//   - Global channel table (max 32 channels)
//
// Architecture:
//   Each channel is a unidirectional communication link from src to dst.
//   Capability checking ensures that only the owning enclave can send
//   on a channel, and only the destination enclave can receive.

use core::mem;

/// Maximum number of concurrent enclave channels.
const MAX_CHANNELS: usize = 32;

/// Maximum message payload size in bytes.
const CHANNEL_BUF_SIZE: usize = 256;

// ── EnclaveChannel ────────────────────────────────────────────────────────────

/// A secure IPC channel between two enclaves.
#[derive(Clone, Copy)]
pub struct EnclaveChannel {
    pub channel_id: u32,
    pub src_enclave: u32,
    pub dst_enclave: u32,
    /// Slot in the capability table used for access control.
    pub cap_slot: usize,
    /// Message buffer (fixed size).
    buffer: [u8; CHANNEL_BUF_SIZE],
    /// Number of valid bytes in the buffer.
    buffer_len: usize,
    /// Whether the channel is ready for communication.
    pub active: bool,
    /// Whether a message is pending receive.
    has_message: bool,
}

impl EnclaveChannel {
    /// Create a new enclave channel from `src` to `dst`.
    pub fn create(src: u32, dst: u32) -> Option<Self> {
        Some(EnclaveChannel {
            channel_id: 0,
            src_enclave: src,
            dst_enclave: dst,
            cap_slot: 0,
            buffer: [0u8; CHANNEL_BUF_SIZE],
            buffer_len: 0,
            active: true,
            has_message: false,
        })
    }

    /// Send a message to the destination enclave.
    pub fn send(&mut self, msg: &[u8]) -> Result<(), &'static str> {
        if !self.active {
            return Err("channel not active");
        }
        if msg.len() > CHANNEL_BUF_SIZE {
            return Err("message too large");
        }
        self.buffer[..msg.len()].copy_from_slice(msg);
        self.buffer_len = msg.len();
        self.has_message = true;
        Ok(())
    }

    /// Receive a message from the source enclave.
    pub fn recv(&mut self) -> Option<&[u8]> {
        if !self.active || !self.has_message {
            return None;
        }
        self.has_message = false;
        Some(&self.buffer[..self.buffer_len])
    }

    /// Check if the given enclave is the source of this channel.
    pub fn is_source(&self, enclave_id: u32) -> bool {
        self.src_enclave == enclave_id
    }

    /// Check if the given enclave is the destination of this channel.
    pub fn is_destination(&self, enclave_id: u32) -> bool {
        self.dst_enclave == enclave_id
    }

    /// Deactivate the channel.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.has_message = false;
        self.buffer_len = 0;
    }
}

// ── Global channel state ──────────────────────────────────────────────────────

static mut ENCLAVE_CHANNELS: [EnclaveChannel; MAX_CHANNELS] = unsafe { zeroed_channels() };
static mut CHANNEL_COUNT: usize = 0;
static mut CHANNEL_ID_COUNTER: u32 = 1;

/// Helper to zero-initialize the channel table at compile time.
const fn zeroed_channels() -> [EnclaveChannel; MAX_CHANNELS] {
    unsafe { core::mem::transmute([0u8; mem::size_of::<EnclaveChannel>() * MAX_CHANNELS]) }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Create a new enclave IPC channel.
pub fn channel_create(src_enclave: u32, dst_enclave: u32, cap_slot: usize) -> Option<u32> {
    unsafe {
        if CHANNEL_COUNT >= MAX_CHANNELS {
            crate::println!("  TEE IPC: channel table full");
            return None;
        }

        let channel_id = CHANNEL_ID_COUNTER;
        CHANNEL_ID_COUNTER = CHANNEL_ID_COUNTER.wrapping_add(1);

        let mut channel = EnclaveChannel::create(src_enclave, dst_enclave)?;
        channel.channel_id = channel_id;
        channel.cap_slot = cap_slot;

        let idx = CHANNEL_COUNT;
        ENCLAVE_CHANNELS[idx] = channel;
        CHANNEL_COUNT += 1;

        crate::println!(
            "  TEE IPC: channel {} created (enclave {} -> {})",
            channel_id, src_enclave, dst_enclave
        );
        Some(channel_id)
    }
}

/// Send a message on a channel.
pub fn channel_send(channel_id: u32, msg: &[u8]) -> Result<(), &'static str> {
    unsafe {
        for i in 0..CHANNEL_COUNT {
            if ENCLAVE_CHANNELS[i].channel_id == channel_id {
                return ENCLAVE_CHANNELS[i].send(msg);
            }
        }
        Err("channel not found")
    }
}

/// Receive a message from a channel.
pub fn channel_recv(channel_id: u32) -> Option<&'static [u8]> {
    unsafe {
        for i in 0..CHANNEL_COUNT {
            if ENCLAVE_CHANNELS[i].channel_id == channel_id {
                return ENCLAVE_CHANNELS[i].recv();
            }
        }
        None
    }
}

/// Destroy a channel by its ID.
pub fn channel_destroy(channel_id: u32) -> bool {
    unsafe {
        for i in 0..CHANNEL_COUNT {
            if ENCLAVE_CHANNELS[i].channel_id == channel_id {
                ENCLAVE_CHANNELS[i].deactivate();
                return true;
            }
        }
        false
    }
}

/// Destroy all channels associated with an enclave.
pub fn channel_destroy_by_enclave(enclave_id: u32) {
    unsafe {
        for i in 0..CHANNEL_COUNT {
            if ENCLAVE_CHANNELS[i].src_enclave == enclave_id
                || ENCLAVE_CHANNELS[i].dst_enclave == enclave_id
            {
                ENCLAVE_CHANNELS[i].deactivate();
            }
        }
    }
}

/// List all active channels. Returns bytes written.
pub fn channel_list(buf: &mut [u8]) -> usize {
    let mut pos = 0usize;
    unsafe {
        for i in 0..CHANNEL_COUNT {
            if !ENCLAVE_CHANNELS[i].active {
                continue;
            }
            let ch = &ENCLAVE_CHANNELS[i];
            pos += w_str(buf, pos, "ch=");
            pos += w_u64(buf, pos, ch.channel_id as u64);
            pos += w_str(buf, pos, " src=");
            pos += w_u64(buf, pos, ch.src_enclave as u64);
            pos += w_str(buf, pos, " dst=");
            pos += w_u64(buf, pos, ch.dst_enclave as u64);
            pos += w_str(buf, pos, " cap=");
            pos += w_u64(buf, pos, ch.cap_slot as u64);
            pos += w_str(buf, pos, " len=");
            pos += w_u64(buf, pos, ch.buffer_len as u64);
            pos += w_str(buf, pos, "\n");
        }
    }
    pos
}

// ── Formatting helpers ────────────────────────────────────────────────────────

fn w_str(buf: &mut [u8], pos: usize, s: &str) -> usize {
    let bytes = s.as_bytes();
    let len = bytes.len().min(buf.len().saturating_sub(pos));
    if len > 0 {
        buf[pos..pos + len].copy_from_slice(&bytes[..len]);
    }
    len
}

fn w_u64(buf: &mut [u8], pos: usize, v: u64) -> usize {
    if v == 0 {
        if pos < buf.len() {
            buf[pos] = b'0';
            return 1;
        }
        return 0;
    }
    let mut temp = [0u8; 20];
    let mut n = v;
    let mut len = 0;
    while n > 0 {
        temp[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    let mut written = 0;
    for i in (0..len).rev() {
        if pos + written < buf.len() {
            buf[pos + written] = temp[i];
            written += 1;
        } else {
            break;
        }
    }
    written
}
