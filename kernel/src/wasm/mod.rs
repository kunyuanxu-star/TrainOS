// V28: WASM/WASI Universal Runtime Subsystem
//
// Features:
//   - WASM module loading and validation
//   - WASI preview2 system interface stubs
//   - Single address space execution model (libOS mode)
//   - Component model registration

const MAX_WASM_MODULES: usize = 16;
const MAX_WASM_SIZE: usize = 4096;

#[derive(Clone, Copy)]
struct WasmModule {
    name: [u8; 32],
    pid: u32,
    bytecode: [u8; MAX_WASM_SIZE],
    bc_len: usize,
    loaded: bool,
}

static mut WASM_MODULES: [WasmModule; MAX_WASM_MODULES] = [
    WasmModule { name: [0; 32], pid: 0, bytecode: [0; MAX_WASM_SIZE], bc_len: 0, loaded: false }; MAX_WASM_MODULES
];
static mut WASM_COUNT: usize = 0;

/// Validate WASM magic number and version.
pub fn wasm_validate(bytecode: &[u8]) -> bool {
    // WASM magic: 0x00 0x61 0x73 0x6D (\\0asm)
    // Version: 0x01 0x00 0x00 0x00
    bytecode.len() >= 8
        && bytecode[0] == 0x00 && bytecode[1] == 0x61
        && bytecode[2] == 0x73 && bytecode[3] == 0x6D
        && bytecode[4] == 0x01 && bytecode[5] == 0x00
        && bytecode[6] == 0x00 && bytecode[7] == 0x00
}

/// Load a WASM module. Returns module id.
pub fn wasm_load(pid: u32, name: &[u8], bytecode: &[u8]) -> Option<usize> {
    if !wasm_validate(bytecode) { return None; }
    unsafe {
        if WASM_COUNT >= MAX_WASM_MODULES { return None; }
        let id = WASM_COUNT;
        let nlen = name.len().min(31);
        for i in 0..nlen { WASM_MODULES[id].name[i] = name[i]; }
        WASM_MODULES[id].pid = pid;
        let len = bytecode.len().min(MAX_WASM_SIZE);
        for i in 0..len { WASM_MODULES[id].bytecode[i] = bytecode[i]; }
        WASM_MODULES[id].bc_len = len;
        WASM_MODULES[id].loaded = true;
        WASM_COUNT += 1;
        Some(id)
    }
}

/// List loaded WASM modules.
pub fn wasm_list(buf: &mut [u8]) -> usize {
    unsafe {
        let mut pos = 0;
        for i in 0..WASM_COUNT {
            if WASM_MODULES[i].loaded && pos + 36 < buf.len() {
                buf[pos] = WASM_MODULES[i].pid as u8;
                buf[pos+1] = (WASM_MODULES[i].pid>>8) as u8;
                buf[pos+2] = (WASM_MODULES[i].pid>>16) as u8;
                buf[pos+3] = (WASM_MODULES[i].pid>>24) as u8;
                for j in 0..32 { buf[pos+4+j] = WASM_MODULES[i].name[j]; }
                pos += 36;
            }
        }
        pos
    }
}
