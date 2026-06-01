// V37a — Secure Storage for TEE Enclaves
//
// Features:
//   - Data sealing: encrypt and bind data to a specific enclave measurement
//   - Data unsealing: only the exact same enclave can decrypt sealed data
//   - Anti-rollback: each sealed blob carries creation time and access count
//   - Measurement binding: cryptographic binding between blob and enclave
//   - Per-enclave blob listing and management
//
// Architecture:
//   SecureStorage manages a fixed-size pool of SealedBlob entries. Each blob
//   is cryptographically bound to the SHA-512 measurement of the enclave that
//   created it. Unsealing requires an enclave with an identical measurement.
//
//   AES-GCM encryption is simulated using XOR-based stream cipher with
//   measurement-derived keys for no_std compatibility. In production, this
//   would use hardware AES-GCM or a dedicated crypto engine.
//
// Reference: RISC-V TEE secure storage, TCG TPM sealed storage semantics

use crate::security::tee::{ApTeeEnclave, sha256_digest};

// ── Constants ───────────────────────────────────────────────────────────────

/// Maximum number of sealed blobs in storage.
pub const MAX_SEALED_BLOBS: usize = 64;

/// Maximum size of sealed blob data (encrypted payload).
pub const SEALED_DATA_SIZE: usize = 256;

/// IV (nonce) length for encryption.
pub const IV_LEN: usize = 16;

/// Authentication tag length.
pub const TAG_LEN: usize = 16;

// ── SealedBlob ──────────────────────────────────────────────────────────────

/// A sealed data blob bound to a specific enclave measurement.
///
/// Only the enclave with the exact matching measurement can unseal this blob.
#[derive(Clone, Copy)]
pub struct SealedBlob {
    /// Enclave ID that created this blob.
    pub enclave_id: u32,
    /// SHA-512 measurement of the enclave this blob is bound to.
    pub measurement: [u8; 64],
    /// Encrypted data payload.
    pub data: [u8; SEALED_DATA_SIZE],
    /// Length of the payload data.
    pub data_len: usize,
    /// AES-GCM IV (nonce) — 16 bytes.
    pub iv: [u8; IV_LEN],
    /// AES-GCM authentication tag — 16 bytes.
    pub tag: [u8; TAG_LEN],
    /// Tick count when this blob was created.
    pub creation_time: u64,
    /// Number of times this blob has been accessed.
    pub access_count: u64,
}

impl SealedBlob {
    /// Create a new empty sealed blob.
    fn new() -> Self {
        SealedBlob {
            enclave_id: 0,
            measurement: [0u8; 64],
            data: [0u8; SEALED_DATA_SIZE],
            data_len: 0,
            iv: [0u8; IV_LEN],
            tag: [0u8; TAG_LEN],
            creation_time: 0,
            access_count: 0,
        }
    }
}

// ── SecureStorage ───────────────────────────────────────────────────────────

/// Secure storage for TEE enclaves.
///
/// Data is sealed to a specific enclave measurement. Only the exact same
/// enclave (same SHA-512 measurement) can unseal the data.
pub struct SecureStorage {
    /// Pool of sealed blobs.
    sealed_data: [SealedBlob; MAX_SEALED_BLOBS],
    /// Number of currently stored blobs.
    count: usize,
}

impl SecureStorage {
    /// Create a new empty secure storage.
    pub const fn new() -> Self {
        SecureStorage {
            sealed_data: unsafe { core::mem::transmute([0u8; core::mem::size_of::<SealedBlob>() * MAX_SEALED_BLOBS]) },
            count: 0,
        }
    }

    /// Seal data — encrypt and bind to an enclave's measurement.
    ///
    /// The data is encrypted using a key derived from the enclave's measurement.
    /// After sealing, only an enclave with the same measurement can unseal it.
    ///
    /// Returns the blob ID (index in the storage pool) on success.
    pub fn seal(&mut self, enclave: &ApTeeEnclave, data: &[u8]) -> Result<usize, &'static str> {
        if self.count >= MAX_SEALED_BLOBS {
            return Err("secure storage full");
        }
        if data.is_empty() || data.len() > SEALED_DATA_SIZE {
            return Err("invalid data length");
        }

        let idx = self.count;
        let blob = &mut self.sealed_data[idx];

        blob.enclave_id = enclave.enclave_id;
        blob.measurement = enclave.measurement;
        blob.data_len = data.len();
        blob.creation_time = unsafe { crate::trap::TICK_COUNT as u64 };
        blob.access_count = 0;

        // Generate IV from creation time mixed with measurement
        let ts = blob.creation_time;
        for i in 0..IV_LEN {
            blob.iv[i] = ((ts >> (i % 8) * 8) as u8)
                .wrapping_add(enclave.measurement[i % 64])
                .wrapping_mul(13);
        }

        // Derive sealing key from enclave measurement
        let sealing_key = derive_sealing_key(&enclave.measurement, &blob.iv);

        // Encrypt data using XOR-based stream cipher with the sealing key
        for i in 0..data.len() {
            blob.data[i] = data[i] ^ sealing_key[i % sealing_key.len()];
        }

        // Compute authentication tag (simplified: XOR of encrypted data + measurement)
        let mut tag = [0u8; TAG_LEN];
        for i in 0..data.len().min(TAG_LEN) {
            tag[i] = blob.data[i] ^ enclave.measurement[i % 64];
        }
        blob.tag = tag;

        self.count += 1;

        crate::println!(
            "  TEE secure storage: sealed {} bytes for enclave {} (blob {})",
            data.len(), enclave.enclave_id, idx,
        );

        Ok(idx)
    }

    /// Unseal data — only the exact same enclave can decrypt.
    ///
    /// Returns the decrypted data if the enclave's measurement matches
    /// the blob's binding, and the authentication tag is valid.
    pub fn unseal(&self, enclave: &ApTeeEnclave, blob_id: usize) -> Option<&[u8]> {
        if blob_id >= self.count {
            return None;
        }

        let blob = &self.sealed_data[blob_id];

        // Verify measurement binding
        if blob.measurement != enclave.measurement {
            crate::println!(
                "  TEE secure storage: measurement mismatch for blob {}",
                blob_id,
            );
            return None;
        }

        // Verify enclave ID
        if blob.enclave_id != enclave.enclave_id {
            return None;
        }

        // Derive sealing key from enclave measurement + IV
        let sealing_key = derive_sealing_key(&enclave.measurement, &blob.iv);

        // Verify authentication tag
        let mut expected_tag = [0u8; TAG_LEN];
        for i in 0..blob.data_len.min(TAG_LEN) {
            expected_tag[i] = blob.data[i] ^ enclave.measurement[i % 64];
        }
        if expected_tag != blob.tag {
            crate::println!(
                "  TEE secure storage: auth tag mismatch for blob {}",
                blob_id,
            );
            return None;
        }

        // Decrypt: XOR again with the sealing key (XOR is symmetric)
        // We use a static buffer for the decrypted data
        unsafe {
            DECRYPT_BUF = [0u8; SEALED_DATA_SIZE];
            for i in 0..blob.data_len {
                DECRYPT_BUF[i] = blob.data[i] ^ sealing_key[i % sealing_key.len()];
            }

            // Update access count (simulate mutation via raw pointer)
            let blob_ptr = &self.sealed_data[blob_id] as *const SealedBlob as *mut SealedBlob;
            (*blob_ptr).access_count = blob.access_count.wrapping_add(1);

            Some(&DECRYPT_BUF[..blob.data_len])
        }
    }

    /// Delete a sealed blob by its ID.
    pub fn delete(&mut self, blob_id: usize) {
        if blob_id >= self.count {
            return;
        }

        // Shift remaining blobs down
        for i in blob_id..self.count - 1 {
            self.sealed_data[i] = self.sealed_data[i + 1];
        }
        self.count -= 1;

        crate::println!("  TEE secure storage: deleted blob {}", blob_id);
    }

    /// Get a list of blob IDs belonging to a specific enclave.
    pub fn list_for_enclave(&self, enclave_id: u32) -> alloc::vec::Vec<usize> {
        let mut ids = alloc::vec::Vec::new();
        for i in 0..self.count {
            if self.sealed_data[i].enclave_id == enclave_id {
                ids.push(i);
            }
        }
        ids
    }

    /// Check if a blob is sealed to a specific measurement.
    pub fn verify_binding(&self, blob_id: usize, measurement: &[u8; 64]) -> bool {
        if blob_id >= self.count {
            return false;
        }
        self.sealed_data[blob_id].measurement == *measurement
    }

    /// Get the number of stored blobs.
    pub fn blob_count(&self) -> usize {
        self.count
    }

    /// Get the maximum capacity.
    pub fn capacity(&self) -> usize {
        MAX_SEALED_BLOBS
    }

    /// Format secure storage status into a buffer. Returns bytes written.
    pub fn format_status(&self, buf: &mut [u8]) -> usize {
        let mut pos = 0usize;

        pos += w_str(buf, pos, "SecureStorage: ");
        pos += w_u64(buf, pos, self.count as u64);
        pos += w_str(buf, pos, "/");
        pos += w_u64(buf, pos, MAX_SEALED_BLOBS as u64);
        pos += w_str(buf, pos, " blobs\n");

        for i in 0..self.count {
            let blob = &self.sealed_data[i];
            pos += w_str(buf, pos, "  [");
            pos += w_u64(buf, pos, i as u64);
            pos += w_str(buf, pos, "] enclave=");
            pos += w_u64(buf, pos, blob.enclave_id as u64);
            pos += w_str(buf, pos, " len=");
            pos += w_u64(buf, pos, blob.data_len as u64);
            pos += w_str(buf, pos, " accesses=");
            pos += w_u64(buf, pos, blob.access_count);
            pos += w_str(buf, pos, "\n");
        }

        pos
    }
}

// ── Decryption buffer ───────────────────────────────────────────────────────

/// Static buffer for decrypted data (single-threaded TEE context).
static mut DECRYPT_BUF: [u8; SEALED_DATA_SIZE] = [0u8; SEALED_DATA_SIZE];

// ── Key derivation ──────────────────────────────────────────────────────────

/// Derive a sealing key from enclave measurement and IV.
///
/// Uses SHA-256 as a KDF: key = SHA256(measurement || iv || domain_separator)
fn derive_sealing_key(measurement: &[u8; 64], iv: &[u8; IV_LEN]) -> [u8; 32] {
    let mut input = alloc::vec::Vec::new();
    input.extend_from_slice(measurement);
    input.extend_from_slice(iv);
    // Domain separation: prevents key reuse across different purposes
    input.extend_from_slice(b"TEE-SEALING-v1");
    sha256_digest(&input)
}

// ── Global secure storage ───────────────────────────────────────────────────

static mut SECURE_STORAGE: SecureStorage = SecureStorage::new();

/// Initialize secure storage.
pub fn secure_storage_init() {
    crate::println!(
        "  TEE secure storage: initialized ({} blobs, {} bytes each)",
        MAX_SEALED_BLOBS,
        SEALED_DATA_SIZE,
    );
}

/// Get a mutable reference to the global secure storage.
pub fn secure_storage() -> &'static mut SecureStorage {
    unsafe { &mut SECURE_STORAGE }
}

/// Seal data using the global secure storage.
pub fn storage_seal(enclave: &ApTeeEnclave, data: &[u8]) -> Result<usize, &'static str> {
    unsafe { SECURE_STORAGE.seal(enclave, data) }
}

/// Unseal data using the global secure storage.
pub fn storage_unseal(enclave: &ApTeeEnclave, blob_id: usize) -> Option<&'static [u8]> {
    unsafe { SECURE_STORAGE.unseal(enclave, blob_id) }
}

/// Delete a blob from the global secure storage.
pub fn storage_delete(blob_id: usize) {
    unsafe { SECURE_STORAGE.delete(blob_id); }
}

// ── Formatting helpers ──────────────────────────────────────────────────────

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
