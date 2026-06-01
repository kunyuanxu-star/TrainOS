// V38a — RISC-V Crypto Extensions (Zk/Zkn/Zks)

pub mod aes;
pub mod sha;
pub mod sm;
pub mod entropy;

use entropy::CryptoEntropy;

static mut CRYPTO_ENTROPY: Option<CryptoEntropy> = None;

pub fn crypto_init() {
    crate::println!("  Crypto: using software implementations (no Zk* extensions detected)");
    unsafe {
        CRYPTO_ENTROPY = Some(CryptoEntropy::init());
    }
    crate::println!("  Crypto: entropy source initialized (Zkr enhanced)");
}

pub fn crypto_entropy() -> &'static mut CryptoEntropy {
    unsafe { CRYPTO_ENTROPY.as_mut().expect("crypto entropy not initialized") }
}
