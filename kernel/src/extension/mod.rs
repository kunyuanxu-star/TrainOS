// V24: Programmable kernel extension framework (eBPF-like)
//
// Architecture:
//   - Extension registration with bytecode verification
//   - Probe hooks: syscall entry/exit, timer, IPC
//   - Sandboxed execution with time/memory limits

const MAX_EXTENSIONS: usize = 8;
const MAX_BYTECODE: usize = 256;

#[derive(Clone, Copy)]
struct Extension {
    name: [u8; 16],
    pid: u32,
    bytecode: [u8; MAX_BYTECODE],
    bc_len: usize,
    hook_type: u8,
    active: bool,
}

static mut EXTENSIONS: [Extension; MAX_EXTENSIONS] = [
    Extension { name: [0; 16], pid: 0, bytecode: [0; MAX_BYTECODE], bc_len: 0, hook_type: 0, active: false }; MAX_EXTENSIONS
];
static mut EXT_COUNT: usize = 0;

// Hook types
pub const HOOK_SYSCALL_ENTER: u8 = 1;
pub const HOOK_SYSCALL_EXIT: u8 = 2;
pub const HOOK_TIMER: u8 = 3;
pub const HOOK_IPC_SEND: u8 = 4;

/// Register a kernel extension. Returns extension id.
pub fn register(pid: u32, hook_type: u8, bytecode: &[u8]) -> Option<usize> {
    unsafe {
        if EXT_COUNT >= MAX_EXTENSIONS { return None; }
        let id = EXT_COUNT;
        EXTENSIONS[id].pid = pid;
        EXTENSIONS[id].hook_type = hook_type;
        let len = bytecode.len().min(MAX_BYTECODE);
        for i in 0..len { EXTENSIONS[id].bytecode[i] = bytecode[i]; }
        EXTENSIONS[id].bc_len = len;
        EXTENSIONS[id].active = true;
        EXT_COUNT += 1;

        // Simple verifier: check for infinite loop bytecodes
        if verify_bytecode(&EXTENSIONS[id].bytecode[..len]) {
            Some(id)
        } else {
            EXTENSIONS[id].active = false;
            None
        }
    }
}

/// Basic bytecode verifier — checks for known-bad patterns.
fn verify_bytecode(bc: &[u8]) -> bool {
    if bc.is_empty() { return false; }
    // Reject "jmp -1" infinite loop pattern (0xEB 0xFD)
    for i in 0..bc.len().saturating_sub(1) {
        if bc[i] == 0xEB && bc[i+1] == 0xFD { return false; }
    }
    true
}

/// Unregister a kernel extension.
pub fn unregister(ext_id: usize) -> bool {
    unsafe {
        if ext_id < EXT_COUNT && EXTENSIONS[ext_id].active {
            EXTENSIONS[ext_id].active = false;
            return true;
        }
    }
    false
}

/// List registered extensions.
pub fn list(buf: &mut [u8]) -> usize {
    unsafe {
        let mut pos = 0;
        for i in 0..EXT_COUNT {
            if EXTENSIONS[i].active && pos + 20 < buf.len() {
                let len = EXTENSIONS[i].bc_len.min(16);
                buf[pos] = len as u8; pos += 1;
                for j in 0..len { buf[pos+j] = EXTENSIONS[i].name[j]; }
                pos += 16;
                buf[pos] = EXTENSIONS[i].hook_type; pos += 1;
                buf[pos] = EXTENSIONS[i].bc_len as u8; pos += 1;
                buf[pos] = (EXTENSIONS[i].bc_len>>8) as u8; pos += 1;
            }
        }
        pos
    }
}
