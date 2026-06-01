// V38a — RISC-V Zks (SM3/SM4) Extensions (software)

static mut SM3_HW_AVAILABLE: bool = false;
static mut SM4_HW_AVAILABLE: bool = false;

pub fn sm3_available() -> bool { unsafe { SM3_HW_AVAILABLE } }
pub fn sm4_available() -> bool { unsafe { SM4_HW_AVAILABLE } }
pub fn set_sm3_available() { unsafe { SM3_HW_AVAILABLE = true; } }
pub fn set_sm4_available() { unsafe { SM4_HW_AVAILABLE = true; } }

// SM3 Hash

pub struct Sm3 {
    state: [u32; 8], buf: [u8; 64], buf_len: usize, total_len: u64,
}

impl Sm3 {
    const IV: [u32; 8] = [0x7380166f, 0x4914b2b9, 0x172442d7, 0xda8a0600, 0xa96f30bc, 0x163138aa, 0xe38dee4d, 0xb0fb0e4e];

    fn t(j: usize) -> u32 { if j < 16 { 0x79cc4519 } else { 0x7a879d8a } }
    fn ff(j: usize, x: u32, y: u32, z: u32) -> u32 { if j < 16 { x ^ y ^ z } else { (x & y) | (x & z) | (y & z) } }
    fn gg(j: usize, x: u32, y: u32, z: u32) -> u32 { if j < 16 { x ^ y ^ z } else { (x & y) | ((!x) & z) } }
    fn p0(x: u32) -> u32 { x ^ x.rotate_left(9) ^ x.rotate_left(17) }
    fn p1(x: u32) -> u32 { x ^ x.rotate_left(15) ^ x.rotate_left(23) }

    pub fn new() -> Self { Sm3 { state: Self::IV, buf: [0u8; 64], buf_len: 0, total_len: 0 } }

    pub fn update(&mut self, data: &[u8]) {
        let mut offset = 0; let len = data.len();
        self.total_len += len as u64;
        if self.buf_len > 0 {
            let sp = 64 - self.buf_len; let tk = sp.min(len);
            self.buf[self.buf_len..self.buf_len + tk].copy_from_slice(&data[..tk]);
            self.buf_len += tk; offset += tk;
            if self.buf_len == 64 { self.compress_block(); self.buf_len = 0; }
        }
        while offset + 64 <= len {
            let blk: &[u8; 64] = data[offset..offset + 64].try_into().unwrap();
            self.buf.copy_from_slice(blk); self.compress_block(); offset += 64;
        }
        if offset < len { let rem = len - offset; self.buf[..rem].copy_from_slice(&data[offset..]); self.buf_len = rem; }
    }

    pub fn finalize(&mut self) -> [u8; 32] {
        let bit_len = self.total_len * 8;
        self.buf[self.buf_len] = 0x80; self.buf_len += 1;
        if self.buf_len > 56 {
            for i in self.buf_len..64 { self.buf[i] = 0; }
            self.compress_block(); self.buf_len = 0;
        }
        for i in self.buf_len..56 { self.buf[i] = 0; }
        self.buf[56..64].copy_from_slice(&bit_len.to_be_bytes()); self.compress_block();
        let mut hash = [0u8; 32];
        for i in 0..8 { hash[i*4..(i+1)*4].copy_from_slice(&self.state[i].to_be_bytes()); }
        hash
    }

    pub fn digest(data: &[u8]) -> [u8; 32] { let mut h = Sm3::new(); h.update(data); h.finalize() }

    pub fn hmac(key: &[u8], data: &[u8]) -> [u8; 32] {
        let mut k = [0u8; 64];
        if key.len() > 64 { let h = Sm3::digest(key); k[..32].copy_from_slice(&h); }
        else { k[..key.len()].copy_from_slice(key); }
        let mut ik = [0u8; 64]; let mut ok = [0u8; 64];
        for i in 0..64 { ik[i] = k[i] ^ 0x36; ok[i] = k[i] ^ 0x5C; }
        let mut inner = Sm3::new(); inner.update(&ik); inner.update(data); let ih = inner.finalize();
        let mut outer = Sm3::new(); outer.update(&ok); outer.update(&ih); outer.finalize()
    }

    fn compress_block(&mut self) {
        let mut w = [0u32; 68]; let mut wp = [0u32; 64];
        for i in 0..16 { w[i] = u32::from_be_bytes([self.buf[i*4], self.buf[i*4+1], self.buf[i*4+2], self.buf[i*4+3]]); }
        for i in 16..68 {
            w[i] = Self::p1(w[i-16] ^ w[i-9] ^ w[i-3].rotate_left(15)) ^ w[i-13].rotate_left(7) ^ w[i-6];
        }
        for i in 0..64 { wp[i] = w[i] ^ w[i+4]; }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) = (self.state[0], self.state[1], self.state[2], self.state[3], self.state[4], self.state[5], self.state[6], self.state[7]);
        for j in 0..64 {
            let ss1 = a.rotate_left(12).wrapping_add(e).wrapping_add(Self::t(j)).rotate_left(7);
            let ss2 = ss1 ^ a.rotate_left(12);
            let tt1 = Self::ff(j, a, b, c).wrapping_add(d).wrapping_add(ss2).wrapping_add(wp[j]);
            let tt2 = Self::gg(j, e, f, g).wrapping_add(h).wrapping_add(ss1).wrapping_add(w[j]);
            d = c; c = b.rotate_left(9); b = a; a = tt1;
            h = g; g = f.rotate_left(19); f = e; e = Self::p0(tt2);
        }
        self.state[0] ^= a; self.state[1] ^= b; self.state[2] ^= c; self.state[3] ^= d;
        self.state[4] ^= e; self.state[5] ^= f; self.state[6] ^= g; self.state[7] ^= h;
    }
}

// SM4 Block Cipher

const SM4_SBOX: [u8; 256] = [
    0xd6, 0x90, 0xe9, 0xfe, 0xcc, 0xe1, 0x3d, 0xb7, 0x16, 0xb6, 0x14, 0xc2, 0x28, 0xfb, 0x2c, 0x05,
    0x2b, 0x67, 0x9a, 0x76, 0x2a, 0xbe, 0x04, 0xc3, 0xaa, 0x44, 0x13, 0x26, 0x49, 0x86, 0x06, 0x99,
    0x9c, 0x42, 0x50, 0xf4, 0x91, 0xef, 0x98, 0x7a, 0x33, 0x54, 0x0b, 0x43, 0xed, 0xcf, 0xac, 0x62,
    0xe4, 0xb3, 0x1c, 0xa9, 0xc9, 0x08, 0xe8, 0x95, 0x80, 0xdf, 0x94, 0xfa, 0x75, 0x8f, 0x3f, 0xa6,
    0x47, 0x07, 0xa7, 0xfc, 0xf3, 0x73, 0x17, 0xba, 0x83, 0x59, 0x3c, 0x19, 0xe6, 0x85, 0x4f, 0xa8,
    0x68, 0x6b, 0x81, 0xb2, 0x71, 0x64, 0xda, 0x8b, 0xf8, 0xeb, 0x0f, 0x4b, 0x70, 0x56, 0x9d, 0x35,
    0x1e, 0x24, 0x0e, 0x5e, 0x63, 0x58, 0xd1, 0xa2, 0x25, 0x22, 0x7c, 0x3b, 0x01, 0x21, 0x78, 0x87,
    0xd4, 0x00, 0x46, 0x57, 0x9f, 0xd3, 0x27, 0x52, 0x4c, 0x36, 0x02, 0xe7, 0xa0, 0xc4, 0xc8, 0x9e,
    0xea, 0xbf, 0x8a, 0xd2, 0x40, 0xc7, 0x38, 0xb5, 0xa3, 0xf7, 0xf2, 0xce, 0xf9, 0x61, 0x15, 0xa1,
    0xe0, 0xae, 0x5d, 0xa4, 0x9b, 0x34, 0x1a, 0x55, 0xad, 0x93, 0x32, 0x30, 0xf5, 0x8c, 0xb1, 0xe3,
    0x1d, 0xf6, 0xe2, 0x2e, 0x82, 0x66, 0xca, 0x60, 0xc0, 0x29, 0x23, 0xab, 0x0d, 0x53, 0x4e, 0x6f,
    0xd5, 0xdb, 0x37, 0x45, 0xde, 0xfd, 0x8e, 0x2f, 0x03, 0xff, 0x6a, 0x72, 0x6d, 0x6c, 0x5b, 0x51,
    0x8d, 0x1b, 0xaf, 0x92, 0xbb, 0xdd, 0xbc, 0x7f, 0x11, 0xd9, 0x5c, 0x41, 0x1f, 0x10, 0x5a, 0xd8,
    0x0a, 0xc1, 0x31, 0x88, 0xa5, 0xcd, 0x7b, 0xbd, 0x2d, 0x74, 0xd0, 0x12, 0xb8, 0xe5, 0xb4, 0xb0,
    0x89, 0x69, 0x97, 0x4a, 0x0c, 0x96, 0x77, 0x7e, 0x65, 0xb9, 0xf1, 0x09, 0xc5, 0x6e, 0xc6, 0x84,
    0x18, 0xf0, 0x7d, 0xec, 0x3a, 0xdc, 0x4d, 0x20, 0x79, 0xee, 0x5f, 0x3e, 0xd7, 0xcb, 0x39, 0x48,
];

fn sm4_l(b: u32) -> u32 { b ^ b.rotate_left(2) ^ b.rotate_left(10) ^ b.rotate_left(18) ^ b.rotate_left(24) }
fn sm4_lp(b: u32) -> u32 { b ^ b.rotate_left(13) ^ b.rotate_left(23) }
fn sm4_t(inp: u32) -> u32 { let x = inp.to_be_bytes(); sm4_l(u32::from_be_bytes([SM4_SBOX[x[0] as usize], SM4_SBOX[x[1] as usize], SM4_SBOX[x[2] as usize], SM4_SBOX[x[3] as usize]])) }
fn sm4_tp(inp: u32) -> u32 { let x = inp.to_be_bytes(); sm4_lp(u32::from_be_bytes([SM4_SBOX[x[0] as usize], SM4_SBOX[x[1] as usize], SM4_SBOX[x[2] as usize], SM4_SBOX[x[3] as usize]])) }

const SM4_FK: [u32; 4] = [0xa3b1bac6, 0x56aa3350, 0x677d9197, 0xb27022dc];
const SM4_CK: [u32; 32] = [
    0x00070e15, 0x1c232a31, 0x383f464d, 0x545b6269, 0x70777e85, 0x8c939aa1, 0xa8afb6bd, 0xc4cbd2d9,
    0xe0e7eef5, 0xfc030a11, 0x181f262d, 0x343b4249, 0x50575e65, 0x6c737a81, 0x888f969d, 0xa4abb2b9,
    0xc0c7ced5, 0xdce3eaf1, 0xf8ff060d, 0x141b2229, 0x30373e45, 0x4c535a61, 0x686f767d, 0x848b9299,
    0xa0a7aeb5, 0xbcc3cad1, 0xd8dfe6ed, 0xf4fb0209, 0x10171e25, 0x2c333a41, 0x484f565d, 0x646b7279,
];

pub struct Sm4 { round_keys: [u32; 32] }

impl Sm4 {
    pub fn new(key: &[u8; 16]) -> Self {
        let mk = [u32::from_be_bytes([key[0], key[1], key[2], key[3]]), u32::from_be_bytes([key[4], key[5], key[6], key[7]]), u32::from_be_bytes([key[8], key[9], key[10], key[11]]), u32::from_be_bytes([key[12], key[13], key[14], key[15]])];
        let mut k = [0u32; 36];
        for i in 0..4 { k[i] = mk[i] ^ SM4_FK[i]; }
        let mut rk = [0u32; 32];
        for i in 0..32 { rk[i] = k[i+1] ^ k[i+2] ^ k[i+3] ^ SM4_CK[i]; rk[i] = k[i] ^ sm4_tp(rk[i]); k[i+4] = rk[i]; }
        Sm4 { round_keys: rk }
    }

    pub fn encrypt_block(&self, block: &[u8; 16]) -> [u8; 16] {
        let mut x = [u32::from_be_bytes([block[0], block[1], block[2], block[3]]), u32::from_be_bytes([block[4], block[5], block[6], block[7]]), u32::from_be_bytes([block[8], block[9], block[10], block[11]]), u32::from_be_bytes([block[12], block[13], block[14], block[15]])];
        for i in 0..32 {
            let t = x[1] ^ x[2] ^ x[3] ^ self.round_keys[i];
            let buf = x[0] ^ sm4_t(t); x[0] = x[1]; x[1] = x[2]; x[2] = x[3]; x[3] = buf;
        }
        let mut out = [0u8; 16]; out[0..4].copy_from_slice(&x[3].to_be_bytes()); out[4..8].copy_from_slice(&x[2].to_be_bytes()); out[8..12].copy_from_slice(&x[1].to_be_bytes()); out[12..16].copy_from_slice(&x[0].to_be_bytes());
        out
    }

    pub fn decrypt_block(&self, block: &[u8; 16]) -> [u8; 16] {
        let mut x = [u32::from_be_bytes([block[0], block[1], block[2], block[3]]), u32::from_be_bytes([block[4], block[5], block[6], block[7]]), u32::from_be_bytes([block[8], block[9], block[10], block[11]]), u32::from_be_bytes([block[12], block[13], block[14], block[15]])];
        for i in 0..32 {
            let t = x[1] ^ x[2] ^ x[3] ^ self.round_keys[31 - i];
            let buf = x[0] ^ sm4_t(t); x[0] = x[1]; x[1] = x[2]; x[2] = x[3]; x[3] = buf;
        }
        let mut out = [0u8; 16]; out[0..4].copy_from_slice(&x[3].to_be_bytes()); out[4..8].copy_from_slice(&x[2].to_be_bytes()); out[8..12].copy_from_slice(&x[1].to_be_bytes()); out[12..16].copy_from_slice(&x[0].to_be_bytes());
        out
    }

    pub fn encrypt_ctr(&self, data: &[u8], nonce: &[u8; 12]) -> alloc::vec::Vec<u8> {
        let mut result = alloc::vec::Vec::with_capacity(data.len());
        let mut counter = 1u32; let mut offset = 0;
        while offset < data.len() {
            let mut cb = [0u8; 16]; cb[..12].copy_from_slice(nonce); cb[12..16].copy_from_slice(&counter.to_be_bytes());
            let ks = self.encrypt_block(&cb);
            let chunk = (data.len() - offset).min(16);
            for i in 0..chunk { result.push(data[offset + i] ^ ks[i]); }
            counter = counter.wrapping_add(1); offset += chunk;
        }
        result
    }

    pub fn encrypt_cbc(&self, data: &[u8], iv: &[u8; 16]) -> alloc::vec::Vec<u8> {
        let padded = ((data.len() + 15) / 16) * 16;
        let mut result = alloc::vec::Vec::with_capacity(padded);
        let mut prev = *iv;
        for chunk in data.chunks(16) {
            let mut block = [0u8; 16]; let mut i = 0;
            for b in chunk { block[i] = *b; i += 1; }
            let pad = (16 - chunk.len()) as u8; while i < 16 { block[i] = pad; i += 1; }
            for j in 0..16 { block[j] ^= prev[j]; }
            let enc = self.encrypt_block(&block); result.extend_from_slice(&enc); prev = enc;
        }
        result
    }
}

// Instruction wrappers (software)

pub unsafe fn sm3p0(rs1: u32) -> u32 { rs1 ^ rs1.rotate_left(9) ^ rs1.rotate_left(17) }
pub unsafe fn sm3p1(rs1: u32) -> u32 { rs1 ^ rs1.rotate_left(15) ^ rs1.rotate_left(23) }
pub unsafe fn sm4ed(rs1: u32, rs2: u32, bs: u8) -> u32 { let b = rs2.to_be_bytes(); rs1 ^ sm4_l(SM4_SBOX[b[(bs & 3) as usize] as usize] as u32) }
pub unsafe fn sm4ks(rs1: u32, rs2: u32, bs: u8) -> u32 { let b = rs2.to_be_bytes(); rs1 ^ sm4_lp(SM4_SBOX[b[(bs & 3) as usize] as usize] as u32) }
