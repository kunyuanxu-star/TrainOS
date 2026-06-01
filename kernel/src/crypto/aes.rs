// V38a — RISC-V Zkne AES Extension (software implementation)

/// Whether Zkne AES extension is available.
static mut AES_HW_AVAILABLE: bool = false;

pub fn aes_available() -> bool {
    unsafe { AES_HW_AVAILABLE }
}

pub fn set_aes_available() {
    unsafe { AES_HW_AVAILABLE = true; }
}

const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

const INV_SBOX: [u8; 256] = [
    0x52, 0x09, 0x6a, 0xd5, 0x30, 0x36, 0xa5, 0x38, 0xbf, 0x40, 0xa3, 0x9e, 0x81, 0xf3, 0xd7, 0xfb,
    0x7c, 0xe3, 0x39, 0x82, 0x9b, 0x2f, 0xff, 0x87, 0x34, 0x8e, 0x43, 0x44, 0xc4, 0xde, 0xe9, 0xcb,
    0x54, 0x7b, 0x94, 0x32, 0xa6, 0xc2, 0x23, 0x3d, 0xee, 0x4c, 0x95, 0x0b, 0x42, 0xfa, 0xc3, 0x4e,
    0x08, 0x2e, 0xa1, 0x66, 0x28, 0xd9, 0x24, 0xb2, 0x76, 0x5b, 0xa2, 0x49, 0x6d, 0x8b, 0xd1, 0x25,
    0x72, 0xf8, 0xf6, 0x64, 0x86, 0x68, 0x98, 0x16, 0xd4, 0xa4, 0x5c, 0xcc, 0x5d, 0x65, 0xb6, 0x92,
    0x6c, 0x70, 0x48, 0x50, 0xfd, 0xed, 0xb9, 0xda, 0x5e, 0x15, 0x46, 0x57, 0xa7, 0x8d, 0x9d, 0x84,
    0x90, 0xd8, 0xab, 0x00, 0x8c, 0xbc, 0xd3, 0x0a, 0xf7, 0xe4, 0x58, 0x05, 0xb8, 0xb3, 0x45, 0x06,
    0xd0, 0x2c, 0x1e, 0x8f, 0xca, 0x3f, 0x0f, 0x02, 0xc1, 0xaf, 0xbd, 0x03, 0x01, 0x13, 0x8a, 0x6b,
    0x3a, 0x91, 0x11, 0x41, 0x4f, 0x67, 0xdc, 0xea, 0x97, 0xf2, 0xcf, 0xce, 0xf0, 0xb4, 0xe6, 0x73,
    0x96, 0xac, 0x74, 0x22, 0xe7, 0xad, 0x35, 0x85, 0xe2, 0xf9, 0x37, 0xe8, 0x1c, 0x75, 0xdf, 0x6e,
    0x47, 0xf1, 0x1a, 0x71, 0x1d, 0x29, 0xc5, 0x89, 0x6f, 0xb7, 0x62, 0x0e, 0xaa, 0x18, 0xbe, 0x1b,
    0xfc, 0x56, 0x3e, 0x4b, 0xc6, 0xd2, 0x79, 0x20, 0x9a, 0xdb, 0xc0, 0xfe, 0x78, 0xcd, 0x5a, 0xf4,
    0x1f, 0xdd, 0xa8, 0x33, 0x88, 0x07, 0xc7, 0x31, 0xb1, 0x12, 0x10, 0x59, 0x27, 0x80, 0xec, 0x5f,
    0x60, 0x51, 0x7f, 0xa9, 0x19, 0xb5, 0x4a, 0x0d, 0x2d, 0xe5, 0x7a, 0x9f, 0x93, 0xc9, 0x9c, 0xef,
    0xa0, 0xe0, 0x3b, 0x4d, 0xae, 0x2a, 0xf5, 0xb0, 0xc8, 0xeb, 0xbb, 0x3c, 0x83, 0x53, 0x99, 0x61,
    0x17, 0x2b, 0x04, 0x7e, 0xba, 0x77, 0xd6, 0x26, 0xe1, 0x69, 0x14, 0x63, 0x55, 0x21, 0x0c, 0x7d,
];

fn sub_word(w: u32) -> u32 {
    u32::from_be_bytes([SBOX[(w >> 24) as usize], SBOX[(w >> 16) as usize], SBOX[(w >> 8) as usize], SBOX[w as usize]])
}

fn rot_word(w: u32) -> u32 {
    (w << 8) | (w >> 24)
}

const RCON: [u32; 11] = [0, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

pub struct AesContext {
    rounds: usize,
    round_keys: [u32; 60],
    inv_round_keys: [u32; 60],
}

impl AesContext {
    pub fn new_128(key: &[u8; 16]) -> Self {
        let mut ctx = AesContext { rounds: 10, round_keys: [0u32; 60], inv_round_keys: [0u32; 60] };
        ctx.round_keys[0] = u32::from_be_bytes([key[0], key[1], key[2], key[3]]);
        ctx.round_keys[1] = u32::from_be_bytes([key[4], key[5], key[6], key[7]]);
        ctx.round_keys[2] = u32::from_be_bytes([key[8], key[9], key[10], key[11]]);
        ctx.round_keys[3] = u32::from_be_bytes([key[12], key[13], key[14], key[15]]);
        ctx.expand_key_128();
        ctx
    }

    pub fn new_256(key: &[u8; 32]) -> Self {
        let mut ctx = AesContext { rounds: 14, round_keys: [0u32; 60], inv_round_keys: [0u32; 60] };
        for i in 0..8 {
            ctx.round_keys[i] = u32::from_be_bytes([key[i*4], key[i*4+1], key[i*4+2], key[i*4+3]]);
        }
        ctx.expand_key_256();
        ctx
    }

    fn expand_key_128(&mut self) {
        let rk = &mut self.round_keys;
        for i in 4..44 {
            let mut t = rk[i - 1];
            if i % 4 == 0 {
                t = sub_word(rot_word(t)) ^ RCON[i / 4];
            }
            rk[i] = rk[i - 4] ^ t;
        }
        for i in 0..=10 {
            let src = i * 4;
            let dst = (10 - i) * 4;
            if i == 0 || i == 10 {
                self.inv_round_keys[dst..dst+4].copy_from_slice(&rk[src..src+4]);
            } else {
                let mut w = [rk[src], rk[src+1], rk[src+2], rk[src+3]];
                inv_mix_columns_word(&mut w);
                self.inv_round_keys[dst..dst+4].copy_from_slice(&w);
            }
        }
    }

    fn expand_key_256(&mut self) {
        let rk = &mut self.round_keys;
        for i in 8..60 {
            let mut t = rk[i - 1];
            if i % 8 == 0 {
                t = sub_word(rot_word(t)) ^ RCON[i / 8];
            } else if i % 8 == 4 {
                t = sub_word(t);
            }
            rk[i] = rk[i - 8] ^ t;
        }
        for i in 0..=14 {
            let src = i * 4;
            let dst = (14 - i) * 4;
            if i == 0 || i == 14 {
                self.inv_round_keys[dst..dst+4].copy_from_slice(&rk[src..src+4]);
            } else {
                let mut w = [rk[src], rk[src+1], rk[src+2], rk[src+3]];
                inv_mix_columns_word(&mut w);
                self.inv_round_keys[dst..dst+4].copy_from_slice(&w);
            }
        }
    }

    pub fn encrypt_block(&self, block: &[u8; 16]) -> [u8; 16] {
        let mut s = [
            u32::from_be_bytes([block[0], block[1], block[2], block[3]]),
            u32::from_be_bytes([block[4], block[5], block[6], block[7]]),
            u32::from_be_bytes([block[8], block[9], block[10], block[11]]),
            u32::from_be_bytes([block[12], block[13], block[14], block[15]]),
        ];
        for c in 0..4 { s[c] ^= self.round_keys[c]; }
        for round in 1..self.rounds {
            for c in 0..4 {
                let sb = |b: usize| SBOX[(s[c] >> (24 - b * 8)) as usize & 0xFF];
                s[c] = u32::from_be_bytes([sb(0), sb(1), sb(2), sb(3)]);
            }
            let t0 = s[0]; let t1 = s[1]; let t2 = s[2]; let t3 = s[3];
            s[0] = t0; s[1] = (t1 << 8) | (t1 >> 24); s[2] = (t2 << 16) | (t2 >> 16); s[3] = (t3 << 24) | (t3 >> 8);
            mix_columns(&mut s);
            let rk = round * 4;
            for c in 0..4 { s[c] ^= self.round_keys[rk + c]; }
        }
        {
            let round = self.rounds;
            for c in 0..4 {
                let sb = |b: usize| SBOX[(s[c] >> (24 - b * 8)) as usize & 0xFF];
                s[c] = u32::from_be_bytes([sb(0), sb(1), sb(2), sb(3)]);
            }
            let t0 = s[0]; let t1 = s[1]; let t2 = s[2]; let t3 = s[3];
            s[0] = t0; s[1] = (t1 << 8) | (t1 >> 24); s[2] = (t2 << 16) | (t2 >> 16); s[3] = (t3 << 24) | (t3 >> 8);
            let rk = round * 4;
            for c in 0..4 { s[c] ^= self.round_keys[rk + c]; }
        }
        let mut out = [0u8; 16];
        for c in 0..4 { out[c*4..(c+1)*4].copy_from_slice(&s[c].to_be_bytes()); }
        out
    }

    pub fn decrypt_block(&self, block: &[u8; 16]) -> [u8; 16] {
        let mut s = [
            u32::from_be_bytes([block[0], block[1], block[2], block[3]]),
            u32::from_be_bytes([block[4], block[5], block[6], block[7]]),
            u32::from_be_bytes([block[8], block[9], block[10], block[11]]),
            u32::from_be_bytes([block[12], block[13], block[14], block[15]]),
        ];
        for c in 0..4 { s[c] ^= self.inv_round_keys[c]; }
        for round in 1..self.rounds {
            let t0 = s[0]; let t1 = s[1]; let t2 = s[2]; let t3 = s[3];
            s[0] = t0; s[1] = (t1 >> 8) | (t1 << 24); s[2] = (t2 >> 16) | (t2 << 16); s[3] = (t3 >> 24) | (t3 << 8);
            for c in 0..4 {
                let isb = |b: usize| INV_SBOX[(s[c] >> (24 - b * 8)) as usize & 0xFF];
                s[c] = u32::from_be_bytes([isb(0), isb(1), isb(2), isb(3)]);
            }
            let rk = round * 4;
            for c in 0..4 { s[c] ^= self.inv_round_keys[rk + c]; }
            inv_mix_columns(&mut s);
        }
        {
            let t0 = s[0]; let t1 = s[1]; let t2 = s[2]; let t3 = s[3];
            s[0] = t0; s[1] = (t1 >> 8) | (t1 << 24); s[2] = (t2 >> 16) | (t2 << 16); s[3] = (t3 >> 24) | (t3 << 8);
            for c in 0..4 {
                let isb = |b: usize| INV_SBOX[(s[c] >> (24 - b * 8)) as usize & 0xFF];
                s[c] = u32::from_be_bytes([isb(0), isb(1), isb(2), isb(3)]);
            }
            let rk = self.rounds * 4;
            for c in 0..4 { s[c] ^= self.inv_round_keys[rk + c]; }
        }
        let mut out = [0u8; 16];
        for c in 0..4 { out[c*4..(c+1)*4].copy_from_slice(&s[c].to_be_bytes()); }
        out
    }

    pub fn encrypt_ctr(&self, plaintext: &[u8], nonce: &[u8; 12]) -> alloc::vec::Vec<u8> {
        let mut result = alloc::vec::Vec::with_capacity(plaintext.len());
        let mut counter = 1u32;
        let mut offset = 0;
        while offset < plaintext.len() {
            let mut cb = [0u8; 16];
            cb[..12].copy_from_slice(nonce);
            cb[12..16].copy_from_slice(&counter.to_be_bytes());
            let keystream = self.encrypt_block(&cb);
            let remaining = plaintext.len() - offset;
            let chunk_size = remaining.min(16);
            for i in 0..chunk_size { result.push(plaintext[offset + i] ^ keystream[i]); }
            counter = counter.wrapping_add(1);
            offset += chunk_size;
        }
        result
    }

    pub fn decrypt_ctr(&self, ciphertext: &[u8], nonce: &[u8; 12]) -> alloc::vec::Vec<u8> {
        self.encrypt_ctr(ciphertext, nonce)
    }

    pub fn encrypt_gcm(&self, plaintext: &[u8], nonce: &[u8; 12], aad: &[u8]) -> (alloc::vec::Vec<u8>, [u8; 16]) {
        let ciphertext = self.encrypt_ctr(plaintext, nonce);
        let h_words: [u32; 4] = [self.round_keys[0], self.round_keys[1], self.round_keys[2], self.round_keys[3]];
        let tag = ghash_authentication(&h_words, &ciphertext, aad);
        let mut cb = [0u8; 16];
        cb[..12].copy_from_slice(nonce);
        cb[15] = 1;
        let tag_mask = self.encrypt_block(&cb);
        let mut final_tag = [0u8; 16];
        for i in 0..16 { final_tag[i] = tag[i] ^ tag_mask[i]; }
        (ciphertext, final_tag)
    }

    pub fn decrypt_gcm(&self, ciphertext: &[u8], nonce: &[u8; 12], aad: &[u8], tag: &[u8; 16]) -> Option<alloc::vec::Vec<u8>> {
        let h_words: [u32; 4] = [self.round_keys[0], self.round_keys[1], self.round_keys[2], self.round_keys[3]];
        let computed_tag = ghash_authentication(&h_words, ciphertext, aad);
        let mut cb = [0u8; 16];
        cb[..12].copy_from_slice(nonce);
        cb[15] = 1;
        let tag_mask = self.encrypt_block(&cb);
        let mut expected_tag = [0u8; 16];
        for i in 0..16 { expected_tag[i] = computed_tag[i] ^ tag_mask[i]; }
        let mut diff = 0u8;
        for i in 0..16 { diff |= tag[i] ^ expected_tag[i]; }
        if diff != 0 { return None; }
        Some(self.encrypt_ctr(ciphertext, nonce))
    }

    pub fn encrypt_cbc(&self, data: &[u8], iv: &[u8; 16]) -> alloc::vec::Vec<u8> {
        let padded_len = ((data.len() + 15) / 16) * 16;
        let mut result = alloc::vec::Vec::with_capacity(padded_len);
        let mut prev = *iv;
        for chunk in data.chunks(16) {
            let mut block = [0u8; 16];
            let mut i = 0;
            for b in chunk { block[i] = *b; i += 1; }
            let pad_val = (16 - chunk.len()) as u8;
            while i < 16 { block[i] = pad_val; i += 1; }
            for j in 0..16 { block[j] ^= prev[j]; }
            let encrypted = self.encrypt_block(&block);
            result.extend_from_slice(&encrypted);
            prev = encrypted;
        }
        result
    }

    pub fn decrypt_cbc(&self, data: &[u8], iv: &[u8; 16]) -> alloc::vec::Vec<u8> {
        let mut result = alloc::vec::Vec::new();
        let mut prev = *iv;
        for chunk in data.chunks(16) {
            if chunk.len() < 16 { break; }
            let mut block = [0u8; 16];
            block.copy_from_slice(&chunk[..16]);
            let decrypted = self.decrypt_block(&block);
            let mut plain_block = [0u8; 16];
            for j in 0..16 { plain_block[j] = decrypted[j] ^ prev[j]; }
            result.extend_from_slice(&plain_block);
            prev = block;
        }
        if let Some(&last) = result.last() {
            if last > 0 && last <= 16 {
                let pad_len = last as usize;
                if pad_len <= result.len() {
                    let mut valid = true;
                    for i in result.len() - pad_len..result.len() {
                        if result[i] != last { valid = false; break; }
                    }
                    if valid { result.truncate(result.len() - pad_len); }
                }
            }
        }
        result
    }
}

fn gf_mul(a: u8, b: u8) -> u8 {
    let mut result = 0u8; let mut x = a; let mut y = b;
    for _ in 0..8 {
        if y & 1 != 0 { result ^= x; }
        let carry = x & 0x80; x <<= 1; if carry != 0 { x ^= 0x1b; } y >>= 1;
    }
    result
}

fn mix_columns(state: &mut [u32; 4]) {
    for c in 0..4 {
        let s = state[c].to_be_bytes();
        let a0 = gf_mul(s[0], 2) ^ gf_mul(s[1], 3) ^ s[2] ^ s[3];
        let a1 = s[0] ^ gf_mul(s[1], 2) ^ gf_mul(s[2], 3) ^ s[3];
        let a2 = s[0] ^ s[1] ^ gf_mul(s[2], 2) ^ gf_mul(s[3], 3);
        let a3 = gf_mul(s[0], 3) ^ s[1] ^ s[2] ^ gf_mul(s[3], 2);
        state[c] = u32::from_be_bytes([a0, a1, a2, a3]);
    }
}

fn inv_mix_columns(state: &mut [u32; 4]) {
    for c in 0..4 {
        let s = state[c].to_be_bytes();
        let a0 = gf_mul(s[0], 14) ^ gf_mul(s[1], 11) ^ gf_mul(s[2], 13) ^ gf_mul(s[3], 9);
        let a1 = gf_mul(s[0], 9) ^ gf_mul(s[1], 14) ^ gf_mul(s[2], 11) ^ gf_mul(s[3], 13);
        let a2 = gf_mul(s[0], 13) ^ gf_mul(s[1], 9) ^ gf_mul(s[2], 14) ^ gf_mul(s[3], 11);
        let a3 = gf_mul(s[0], 11) ^ gf_mul(s[1], 13) ^ gf_mul(s[2], 9) ^ gf_mul(s[3], 14);
        state[c] = u32::from_be_bytes([a0, a1, a2, a3]);
    }
}

fn inv_mix_columns_word(w: &mut [u32; 4]) { inv_mix_columns(w); }

fn gf128_mul(x: &[u8; 16], y: &[u8; 16]) -> [u8; 16] {
    let mut z = [0u8; 16]; let mut v = *y;
    for i in 0..128 {
        let byte_idx = i / 8; let bit_idx = 7 - (i % 8);
        if (x[byte_idx] >> bit_idx) & 1 != 0 { for j in 0..16 { z[j] ^= v[j]; } }
        let lsb = v[15] & 1;
        for j in (1..16).rev() { v[j] = (v[j] >> 1) | (v[j - 1] << 7); }
        v[0] >>= 1;
        if lsb != 0 { v[0] ^= 0xe1; }
    }
    z
}

fn ghash_authentication(h_words: &[u32; 4], ciphertext: &[u8], aad: &[u8]) -> [u8; 16] {
    let mut h = [0u8; 16];
    for i in 0..4 { h[i*4..(i+1)*4].copy_from_slice(&h_words[i].to_be_bytes()); }
    let mut y = [0u8; 16];
    let mut block = [0u8; 16];
    for chunk in aad.chunks(16) {
        block = [0u8; 16]; let len = chunk.len(); block[..len].copy_from_slice(chunk);
        for j in 0..16 { y[j] ^= block[j]; } y = gf128_mul(&y, &h);
    }
    for chunk in ciphertext.chunks(16) {
        block = [0u8; 16]; let len = chunk.len(); block[..len].copy_from_slice(chunk);
        for j in 0..16 { y[j] ^= block[j]; } y = gf128_mul(&y, &h);
    }
    let aad_bits = (aad.len() as u64) * 8;
    let ct_bits = (ciphertext.len() as u64) * 8;
    let mut len_block = [0u8; 16];
    len_block[..8].copy_from_slice(&aad_bits.to_be_bytes());
    len_block[8..16].copy_from_slice(&ct_bits.to_be_bytes());
    for j in 0..16 { y[j] ^= len_block[j]; }
    y = gf128_mul(&y, &h);
    y
}

pub unsafe fn aes32esi(rs1: u32, rs2: u32, _bs: u8) -> u32 { rs1 ^ rs2 }
pub unsafe fn aes32esmi(rs1: u32, rs2: u32, _bs: u8) -> u32 { rs1 ^ rs2 }
pub unsafe fn aes32dsi(rs1: u32, rs2: u32, _bs: u8) -> u32 { rs1 ^ rs2 }
pub unsafe fn aes32dsmi(rs1: u32, rs2: u32, _bs: u8) -> u32 { rs1 ^ rs2 }
pub unsafe fn aes64ks1i(rs1: u64, _rnum: u8) -> u64 { rs1 }
pub unsafe fn aes64ks2(rs1: u64, rs2: u64) -> u64 { rs1 ^ rs2 }
