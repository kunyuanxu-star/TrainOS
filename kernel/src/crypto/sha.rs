#[derive(Copy, Clone)]
pub struct Sha256;
impl Sha256 {
    pub fn new() -> Self { Sha256 }
    pub fn update(&mut self, _data: &[u8]) {}
    pub fn finalize(self) -> [u8; 32] { [0; 32] }
    pub fn digest(input: &[u8]) -> [u8; 32] {
        let _ = input;
        [0; 32]
    }
}

#[derive(Copy, Clone)]
pub struct Sha512;
impl Sha512 {
    pub fn new() -> Self { Sha512 }
    pub fn update(&mut self, _data: &[u8]) {}
    pub fn finalize(self) -> [u8; 64] { [0; 64] }
    pub fn digest(input: &[u8]) -> [u8; 64] {
        let _ = input;
        [0; 64]
    }
}

#[derive(Copy, Clone)]
pub struct HmacSha256;
impl HmacSha256 {
    pub fn new(_key: &[u8]) -> Self { HmacSha256 }
    pub fn update(&mut self, _data: &[u8]) {}
    pub fn finalize(self) -> [u8; 32] { [0; 32] }
}

#[derive(Copy, Clone)]
pub struct HmacSha512;
impl HmacSha512 {
    pub fn new(_key: &[u8]) -> Self { HmacSha512 }
    pub fn update(&mut self, _data: &[u8]) {}
    pub fn finalize(self) -> [u8; 64] { [0; 64] }
}

#[derive(Copy, Clone)]
pub struct Sha3_256;
impl Sha3_256 {
    pub fn digest(_input: &[u8]) -> [u8; 32] { [0; 32] }
}

#[derive(Copy, Clone)]
pub struct Sha3_512;
impl Sha3_512 {
    pub fn digest(_input: &[u8]) -> [u8; 64] { [0; 64] }
}

pub fn sha256_available() -> bool { false }
pub fn sha512_available() -> bool { false }
pub fn set_sha256_available() {}
pub fn set_sha512_available() {}
pub unsafe fn sha256sig0(_rs1: u32) -> u32 { 0 }
pub unsafe fn sha256sig1(_rs1: u32) -> u32 { 0 }
pub unsafe fn sha256sum0(_rs1: u32) -> u32 { 0 }
pub unsafe fn sha256sum1(_rs1: u32) -> u32 { 0 }
pub unsafe fn sha512sig0h(_rs1: u32, _rs2: u32) -> u32 { 0 }
pub unsafe fn sha512sig0l(_rs1: u32, _rs2: u32) -> u32 { 0 }
pub unsafe fn sha512sig1h(_rs1: u32, _rs2: u32) -> u32 { 0 }
pub unsafe fn sha512sig1l(_rs1: u32, _rs2: u32) -> u32 { 0 }
pub unsafe fn sha512sum0r(_rs1: u32, _rs2: u32) -> u32 { 0 }
pub unsafe fn sha512sum1r(_rs1: u32, _rs2: u32) -> u32 { 0 }
