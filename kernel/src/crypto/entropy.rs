// V38a — Enhanced Entropy Source (Zkr Extension)

use crate::crypto::aes::AesContext;

const POOL_SIZE: usize = 8;
const CSR_SEED: usize = 0x015;

pub struct CryptoEntropy {
    seed_pool: [u64; POOL_SIZE],
    pool_index: usize,
    reseed_counter: u64,
    drbg_key: [u8; 32],
    drbg_value: [u8; 16],
    zkr_detected: bool,
}

impl CryptoEntropy {
    pub fn init() -> Self {
        let mut ent = CryptoEntropy {
            seed_pool: [0u64; POOL_SIZE],
            pool_index: 0,
            reseed_counter: 0,
            drbg_key: [0u8; 32],
            drbg_value: [0u8; 16],
            zkr_detected: false,
        };
        // Probe Zkr
        let probe_val: usize;
        unsafe { core::arch::asm!("csrrs {}, 0x015, x0", out(reg) probe_val); }
        if probe_val != usize::MAX && probe_val != 0 {
            ent.zkr_detected = true;
            for i in 0..(POOL_SIZE * 4) {
                let v: usize;
                unsafe { core::arch::asm!("csrrs {}, 0x015, x0", out(reg) v); }
                if v != usize::MAX && v != 0 {
                    let st = (v >> 30) & 0x3;
                    if st == 0 {
                        let e16 = (v & 0xFFFF) as u64;
                        let pi = i / 4;
                        ent.seed_pool[pi] ^= e16 << ((i % 4) * 16);
                    }
                }
            }
        }
        if !ent.zkr_detected {
            let tick: u64;
            let cycle: u64;
            unsafe { core::arch::asm!("rdtime {}", out(reg) tick); core::arch::asm!("rdcycle {}", out(reg) cycle); }
            let mut seed = tick.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ cycle;
            for i in 0..POOL_SIZE {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                ent.seed_pool[i] = seed ^ tick.wrapping_add(i as u64);
            }
        }
        ent.drbg_initialize();
        ent
    }

    fn drbg_initialize(&mut self) {
        let mut input = alloc::vec::Vec::new();
        for w in &self.seed_pool { input.extend_from_slice(&w.to_le_bytes()); }
        let hash = crate::crypto::sha::Sha256::digest(&input);
        self.drbg_key.copy_from_slice(&hash);
        self.drbg_value.copy_from_slice(&hash[16..32]);
        for i in 0..16 { if i < input.len() { self.drbg_value[i] ^= input[i]; } }
        self.reseed_counter = 1;
    }

    pub fn reseed(&mut self) {
        let tick: u64; let cycle: u64;
        unsafe { core::arch::asm!("rdtime {}", out(reg) tick); core::arch::asm!("rdcycle {}", out(reg) cycle); }
        let mut seed = tick.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ cycle;
        for i in 0..POOL_SIZE {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            self.seed_pool[i] ^= seed ^ tick.wrapping_add(i as u64);
        }
        let mut mix = alloc::vec::Vec::new();
        for w in &self.seed_pool { mix.extend_from_slice(&w.to_le_bytes()); }
        mix.extend_from_slice(&self.drbg_key);
        mix.extend_from_slice(&self.drbg_value);
        let hash = crate::crypto::sha::Sha256::digest(&mix);
        for i in 0..32 { self.drbg_key[i] ^= hash[i]; }
        for i in 0..16 { self.drbg_value[i] ^= hash[(i + 16) % 32]; }
        self.reseed_counter = 1;
    }

    pub fn drbg_generate(&mut self, len: usize) -> alloc::vec::Vec<u8> {
        let request = len.min(64000);
        if self.reseed_counter > 1000000 { self.reseed(); }
        let mut key_arr = [0u8; 32];
        key_arr.copy_from_slice(&self.drbg_key);
        let ctx = AesContext::new_256(&key_arr);
        let mut result = alloc::vec::Vec::with_capacity(request);
        let mut block = self.drbg_value;
        while result.len() < request {
            let encrypted = ctx.encrypt_block(&block);
            let remaining = request - result.len();
            let chunk = remaining.min(16);
            result.extend_from_slice(&encrypted[..chunk]);
            let mut carry = 1u16;
            for i in (0..16).rev() {
                let val = block[i] as u16 + carry;
                block[i] = val as u8; carry = val >> 8;
                if carry == 0 { break; }
            }
            self.reseed_counter = self.reseed_counter.wrapping_add(1);
        }
        self.drbg_value = block;
        let updated_key = ctx.encrypt_block(&block);
        for i in 0..16 { self.drbg_key[i] ^= updated_key[i]; }
        result
    }

    pub fn random_bytes(&mut self, buf: &mut [u8]) {
        let data = self.drbg_generate(buf.len());
        let len = data.len().min(buf.len());
        buf[..len].copy_from_slice(&data[..len]);
    }

    pub fn random_u64(&mut self) -> u64 {
        let mut buf = [0u8; 8]; self.random_bytes(&mut buf); u64::from_le_bytes(buf)
    }

    pub fn random_u32(&mut self) -> u32 {
        let mut buf = [0u8; 4]; self.random_bytes(&mut buf); u32::from_le_bytes(buf)
    }

    pub fn entropy_quality(&self) -> u32 { if self.zkr_detected { 248 } else { 64 } }
    pub fn is_zkr_available(&self) -> bool { self.zkr_detected }
    pub fn reseed_count(&self) -> u64 { self.reseed_counter }
}

pub fn random_bytes(buf: &mut [u8]) { let ent = crate::crypto::crypto_entropy(); ent.random_bytes(buf); }
pub fn random_u64() -> u64 { let ent = crate::crypto::crypto_entropy(); ent.random_u64() }
pub fn random_u32() -> u32 { let ent = crate::crypto::crypto_entropy(); ent.random_u32() }
