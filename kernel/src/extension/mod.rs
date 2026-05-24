// V24: Programmable kernel extension framework (eBPF-like)
//
// Architecture:
//   - Extension registration with bytecode verification
//   - Probe hooks: syscall entry/exit, timer, IPC
//   - Sandboxed execution with time/memory limits
//   - 32 x u64 virtual register set per extension
//   - 256-byte scratch buffer per extension (persistent between hook fires)
//   - Cycle budget (1000 cycles) enforced per hook invocation

pub mod examples;

// ── Constants ──────────────────────────────────────────────────────────────────

pub(crate) const MAX_EXTENSIONS: usize = 8;
pub const MAX_BYTECODE: usize = 512;
pub const SCRATCH_SIZE: usize = 256;
const CYCLE_BUDGET: u32 = 1000;
const REG_COUNT: usize = 32;
const STACK_DEPTH: usize = 32; // Execution stack for PUSH/POP

// ── Instruction Opcodes ────────────────────────────────────────────────────────

const OP_MOV: u8   = 0x01; // MOV dst, imm32       — dst = imm32
const OP_ADD: u8   = 0x02; // ADD dst, src1, src2  — dst = src1 + src2
const OP_SUB: u8   = 0x03; // SUB dst, src1, src2  — dst = src1 - src2
const OP_CMP: u8   = 0x04; // CMP src1, src2       — set eq = (src1 == src2)
const OP_JMP: u8   = 0x05; // JMP imm32            — pc += imm32 (signed offset)
const OP_JE: u8    = 0x06; // JE imm32             — if eq: pc += imm32
const OP_JNE: u8   = 0x07; // JNE imm32            — if !eq: pc += imm32
const OP_LOAD: u8  = 0x08; // LOAD dst, imm32      — dst = scratch[imm32 as usize]
const OP_STORE: u8 = 0x09; // STORE src1, imm32    — scratch[imm32 as usize] = src1
const OP_PUSH: u8  = 0x0A; // PUSH src1            — push src1 onto execution stack
const OP_POP: u8   = 0x0B; // POP dst              — pop from execution stack into dst
const OP_RET: u8   = 0x0C; // RET                  — stop execution

// ── Hook Types ─────────────────────────────────────────────────────────────────

pub const HOOK_SYSCALL_ENTER: u8 = 1;
pub const HOOK_SYSCALL_EXIT: u8 = 2;
pub const HOOK_TIMER: u8 = 3;
pub const HOOK_IPC_SEND: u8 = 4;

// ── Scratch Buffer Layout (convention) ────────────────────────────────────────
//
// Before execution, the kernel writes hook context to the first 16 bytes:
//   scratch[0..8]  = context0 (hook-dependent primary value, little-endian u64)
//   scratch[8..16] = context1 (hook-dependent secondary value, little-endian u64)
//
// Registers are initialized before execution:
//   reg[30] = hook_type
//   reg[31] = context0
//
// After execution, the kernel checks the log-output area:
//   scratch[250] == 1  → log output available
//   scratch[240..248]  → log value 1 (u64)
//   scratch[248..256]  → log value 2 (u64)
//
// Persistent scratch regions (for counters, state):
//   scratch[128..192] — reserved for extension state

// ── Execution Result ───────────────────────────────────────────────────────────

pub(crate) enum ExecResult {
    Ok,
    Timeout,
    Error(&'static str),
}

// ── Extension Structure ───────────────────────────────────────────────────────

/// Instruction encoding: 8 bytes per instruction.
///
/// Byte layout:
///   [0] opcode
///   [1] reg1 (dst for most operations)
///   [2] reg2 (src1 for most operations)
///   [3] reg3 (src2 for most operations)
///   [4..7] imm32 (little-endian)
///
/// PC advances by 8 after each instruction (except jumps).
/// Maximum 64 instructions (512 bytes / 8).

#[derive(Clone, Copy)]
struct Extension {
    name: [u8; 16],             // null-terminated or zero-padded
    pid: u32,                   // owning process
    bytecode: [u8; MAX_BYTECODE],
    bc_len: usize,
    hook_type: u8,
    active: bool,
    scratch: [u8; SCRATCH_SIZE], // persistent scratch buffer (zeroed on register)
}

static mut EXTENSIONS: [Extension; MAX_EXTENSIONS] = [
    Extension {
        name: [0; 16], pid: 0, bytecode: [0; MAX_BYTECODE], bc_len: 0,
        hook_type: 0, active: false, scratch: [0; SCRATCH_SIZE],
    }; MAX_EXTENSIONS
];
static mut EXT_COUNT: usize = 0;

// ── Bytecode Verification ─────────────────────────────────────────────────────

/// Enhanced bytecode verifier.
///
/// Checks performed:
///   - Valid length (8-512 bytes, multiple of 8)
///   - All opcodes are valid
///   - All register fields are in range 0-31
///   - LOAD/STORE scratch offsets are within bounds (0-248, since we read 8 bytes)
///   - JMP/JE/JNE targets are forward only (no loops), aligned, and in-bounds
///   - Bytecode ends with RET
///   - No back-edges via DFS visited-set on reachable instructions
fn verify_bytecode(bc: &[u8]) -> Result<(), &'static str> {
    if bc.is_empty() {
        return Err("empty bytecode");
    }
    if bc.len() < 8 {
        return Err("bytecode too short (min 8 bytes)");
    }
    if bc.len() > MAX_BYTECODE {
        return Err("bytecode exceeds maximum length");
    }
    if bc.len() % 8 != 0 {
        return Err("bytecode length not multiple of 8");
    }

    let max_inst = bc.len() / 8; // max index + 1
    let mut visited = [false; 64]; // 512/8 = 64 max instructions

    // DFS from pc=0 with explicit stack (arrays, no heap)
    let mut stack: [usize; 64] = [0; 64];
    stack[0] = 0;
    let mut sp: usize = 1;

    while sp > 0 {
        sp -= 1;
        let pc = stack[sp];
        let idx = pc / 8;

        if idx >= max_inst {
            return Err("pc out of bounds");
        }
        if visited[idx] {
            return Err("back-edge detected (potential infinite loop)");
        }
        visited[idx] = true;

        let op = bc[pc];
        let r1 = bc[pc + 1];
        let r2 = bc[pc + 2];
        let r3 = bc[pc + 3];
        let imm_bytes: [u8; 4] = [
            bc[pc + 4], bc[pc + 5], bc[pc + 6], bc[pc + 7],
        ];
        let imm = u32::from_le_bytes(imm_bytes);

        match op {
            OP_RET => {
                // End of path — nothing to push
            }

            OP_JMP => {
                // Unconditional jump: only follow the target
                let offset = imm as i32 as isize;
                let target = pc as isize + offset;
                if offset <= 0 {
                    return Err("JMP: back-edge (non-positive offset)");
                }
                if target as usize % 8 != 0 {
                    return Err("JMP: misaligned target");
                }
                if target as usize >= bc.len() {
                    return Err("JMP: target out of bounds");
                }
                let tidx = target as usize / 8;
                if !visited[tidx] {
                    stack[sp] = target as usize;
                    sp += 1;
                }
            }

            OP_JE | OP_JNE => {
                // Conditional jump: follow both fall-through and target
                let offset = imm as i32 as isize;
                let target = pc as isize + offset;
                if offset <= 0 {
                    return Err("conditional jump: back-edge (non-positive offset)");
                }
                if target as usize % 8 != 0 {
                    return Err("conditional jump: misaligned target");
                }
                if target as usize >= bc.len() {
                    return Err("conditional jump: target out of bounds");
                }
                // Fall-through
                if idx + 1 < max_inst && !visited[idx + 1] {
                    stack[sp] = pc + 8;
                    sp += 1;
                }
                // Jump target
                let tidx = target as usize / 8;
                if !visited[tidx] {
                    stack[sp] = target as usize;
                    sp += 1;
                }
            }

            OP_MOV => {
                if r1 >= 32 { return Err("MOV: invalid dst register"); }
                // Fall-through
                if idx + 1 < max_inst && !visited[idx + 1] {
                    stack[sp] = pc + 8;
                    sp += 1;
                }
            }

            OP_ADD | OP_SUB => {
                if r1 >= 32 { return Err("ADD/SUB: invalid dst register"); }
                if r2 >= 32 { return Err("ADD/SUB: invalid src1 register"); }
                if r3 >= 32 { return Err("ADD/SUB: invalid src2 register"); }
                if idx + 1 < max_inst && !visited[idx + 1] {
                    stack[sp] = pc + 8;
                    sp += 1;
                }
            }

            OP_CMP => {
                if r2 >= 32 { return Err("CMP: invalid src1 register"); }
                if r3 >= 32 { return Err("CMP: invalid src2 register"); }
                if idx + 1 < max_inst && !visited[idx + 1] {
                    stack[sp] = pc + 8;
                    sp += 1;
                }
            }

            OP_LOAD => {
                if r1 >= 32 { return Err("LOAD: invalid dst register"); }
                let offset = imm as usize;
                if offset + 8 > SCRATCH_SIZE {
                    return Err("LOAD: scratch access out of bounds");
                }
                if idx + 1 < max_inst && !visited[idx + 1] {
                    stack[sp] = pc + 8;
                    sp += 1;
                }
            }

            OP_STORE => {
                if r2 >= 32 { return Err("STORE: invalid src register"); }
                let offset = imm as usize;
                if offset + 8 > SCRATCH_SIZE {
                    return Err("STORE: scratch access out of bounds");
                }
                if idx + 1 < max_inst && !visited[idx + 1] {
                    stack[sp] = pc + 8;
                    sp += 1;
                }
            }

            OP_PUSH => {
                if r2 >= 32 { return Err("PUSH: invalid src register"); }
                if idx + 1 < max_inst && !visited[idx + 1] {
                    stack[sp] = pc + 8;
                    sp += 1;
                }
            }

            OP_POP => {
                if r1 >= 32 { return Err("POP: invalid dst register"); }
                if idx + 1 < max_inst && !visited[idx + 1] {
                    stack[sp] = pc + 8;
                    sp += 1;
                }
            }

            _ => {
                return Err("unknown opcode");
            }
        }

        if sp >= 64 {
            return Err("control flow too complex for verifier");
        }
    }

    // Verify the last instruction (even if unreachable) is RET
    if bc[bc.len() - 8] != OP_RET {
        return Err("bytecode must end with RET instruction");
    }

    Ok(())
}

// ── Sandboxed Execution ────────────────────────────────────────────────────────

/// Execute an extension's bytecode with sandboxed virtual machine.
///
/// The extension gets:
///   - 32 x u64 virtual registers (reg[30]=hook_type, reg[31]=context0)
///   - Scratch buffer with hook context written to scratch[0..16]
///   - 1000 cycle budget
///   - 32-element execution stack for PUSH/POP
fn execute_extension(ext: &mut Extension, hook_type: u8, context0: u64, context1: u64) -> ExecResult {
    let mut regs = [0u64; REG_COUNT];
    let mut stack = [0u64; STACK_DEPTH];
    let mut sp: usize = 0;
    let mut pc: usize = 0;
    let mut cycles: u32 = 0;
    let mut eq_flag: bool = false;

    // Set up registers with context
    regs[30] = hook_type as u64;
    regs[31] = context0;

    // Write hook context into scratch buffer
    ext.scratch[0..8].copy_from_slice(&context0.to_le_bytes());
    ext.scratch[8..16].copy_from_slice(&context1.to_le_bytes());

    let bc = &ext.bytecode[..ext.bc_len];

    loop {
        if cycles >= CYCLE_BUDGET {
            return ExecResult::Timeout;
        }
        if pc + 8 > ext.bc_len {
            return ExecResult::Error("pc out of bounds at runtime");
        }

        let op = bc[pc];
        let r1 = bc[pc + 1] as usize;
        let r2 = bc[pc + 2] as usize;
        let r3 = bc[pc + 3] as usize;
        let imm_bytes: [u8; 4] = [
            bc[pc + 4], bc[pc + 5], bc[pc + 6], bc[pc + 7],
        ];
        let imm = u32::from_le_bytes(imm_bytes);
        let imm_signed = imm as i32 as isize;

        cycles += 1;

        match op {
            OP_MOV => {
                regs[r1] = imm as u64;
                pc += 8;
            }
            OP_ADD => {
                regs[r1] = regs[r2].wrapping_add(regs[r3]);
                pc += 8;
            }
            OP_SUB => {
                regs[r1] = regs[r2].wrapping_sub(regs[r3]);
                pc += 8;
            }
            OP_CMP => {
                eq_flag = regs[r2] == regs[r3];
                pc += 8;
            }
            OP_JMP => {
                let target = pc as isize + imm_signed;
                if target < 0 || target as usize >= ext.bc_len || target as usize % 8 != 0 {
                    return ExecResult::Error("JMP: invalid target at runtime");
                }
                pc = target as usize;
            }
            OP_JE => {
                if eq_flag {
                    let target = pc as isize + imm_signed;
                    if target < 0 || target as usize >= ext.bc_len || target as usize % 8 != 0 {
                        return ExecResult::Error("JE: invalid target at runtime");
                    }
                    pc = target as usize;
                } else {
                    pc += 8;
                }
            }
            OP_JNE => {
                if !eq_flag {
                    let target = pc as isize + imm_signed;
                    if target < 0 || target as usize >= ext.bc_len || target as usize % 8 != 0 {
                        return ExecResult::Error("JNE: invalid target at runtime");
                    }
                    pc = target as usize;
                } else {
                    pc += 8;
                }
            }
            OP_LOAD => {
                let offset = imm as usize;
                if offset + 8 > SCRATCH_SIZE {
                    return ExecResult::Error("LOAD: scratch out of bounds at runtime");
                }
                let mut bytes = [0u8; 8];
                bytes.copy_from_slice(&ext.scratch[offset..offset + 8]);
                regs[r1] = u64::from_le_bytes(bytes);
                pc += 8;
            }
            OP_STORE => {
                let offset = imm as usize;
                if offset + 8 > SCRATCH_SIZE {
                    return ExecResult::Error("STORE: scratch out of bounds at runtime");
                }
                let bytes = regs[r2].to_le_bytes();
                ext.scratch[offset..offset + 8].copy_from_slice(&bytes);
                pc += 8;
            }
            OP_PUSH => {
                if sp >= STACK_DEPTH {
                    return ExecResult::Error("execution stack overflow");
                }
                stack[sp] = regs[r2];
                sp += 1;
                pc += 8;
            }
            OP_POP => {
                if sp == 0 {
                    return ExecResult::Error("execution stack underflow");
                }
                sp -= 1;
                regs[r1] = stack[sp];
                pc += 8;
            }
            OP_RET => {
                break;
            }
            _ => {
                return ExecResult::Error("unknown opcode at runtime");
            }
        }
    }

    ExecResult::Ok
}

// ── Registration ───────────────────────────────────────────────────────────────

/// Register a kernel extension. Verifies bytecode, then copies it into the
/// extension table. Returns the extension ID on success.
pub fn register(pid: u32, hook_type: u8, bytecode: &[u8], name: &[u8]) -> Option<usize> {
    // Verify bytecode before registering
    if let Err(e) = verify_bytecode(bytecode) {
        crate::println!("  EXT: bytecode verification failed: {}", e);
        return None;
    }

    unsafe {
        if EXT_COUNT >= MAX_EXTENSIONS {
            return None;
        }
        let id = EXT_COUNT;
        EXTENSIONS[id].pid = pid;
        EXTENSIONS[id].hook_type = hook_type;

        let len = bytecode.len().min(MAX_BYTECODE);
        for i in 0..len {
            EXTENSIONS[id].bytecode[i] = bytecode[i];
        }
        EXTENSIONS[id].bc_len = len;

        // Copy name (up to 15 bytes + null terminator)
        let name_len = name.len().min(15);
        for i in 0..name_len {
            EXTENSIONS[id].name[i] = name[i];
        }
        EXTENSIONS[id].name[name_len] = 0; // ensure null termination

        EXTENSIONS[id].active = true;
        EXTENSIONS[id].scratch = [0; SCRATCH_SIZE]; // zero-initialize scratch buffer

        EXT_COUNT += 1;
        Some(id)
    }
}

/// Unregister a kernel extension by ID.
pub fn unregister(ext_id: usize) -> bool {
    unsafe {
        if ext_id < EXT_COUNT && EXTENSIONS[ext_id].active {
            EXTENSIONS[ext_id].active = false;
            return true;
        }
    }
    false
}

/// List registered extensions into a binary buffer.
///
/// Format per active extension:
///   [name:16][hook_type:1][bc_len:2][status_str:8] = 27 bytes
/// Returns total bytes written.
pub fn list(buf: &mut [u8]) -> usize {
    unsafe {
        let mut pos = 0;
        for i in 0..EXT_COUNT {
            if !EXTENSIONS[i].active {
                continue;
            }
            if pos + 27 > buf.len() {
                break;
            }
            // Name (16 bytes)
            for j in 0..16 {
                buf[pos + j] = EXTENSIONS[i].name[j];
            }
            pos += 16;
            // Hook type (1 byte)
            buf[pos] = EXTENSIONS[i].hook_type;
            pos += 1;
            // Bytecode length (2 bytes, little-endian)
            buf[pos] = EXTENSIONS[i].bc_len as u8;
            buf[pos + 1] = (EXTENSIONS[i].bc_len >> 8) as u8;
            pos += 2;
            // Status / counter from scratch[128..136] as u64 (8 bytes)
            let counter = u64::from_le_bytes(EXTENSIONS[i].scratch[128..136].try_into().unwrap_or([0u8; 8]));
            for b in 0..8 {
                buf[pos + b] = (counter >> (b * 8)) as u8;
            }
            pos += 8;
        }
        pos
    }
}

// ── Hook Execution ────────────────────────────────────────────────────────────

/// Run a hook: iterate all active extensions matching the hook type and execute
/// their bytecode in the sandboxed interpreter.
///
/// Parameters:
///   - `hook_type`: one of HOOK_SYSCALL_ENTER, HOOK_SYSCALL_EXIT, HOOK_TIMER, HOOK_IPC_SEND
///   - `context0`: primary context value (stored in reg[31] and scratch[0..8])
///   - `context1`: secondary context value (stored in scratch[8..16])
pub fn run_hook(hook_type: u8, context0: u64, context1: u64) {
    unsafe {
        for i in 0..EXT_COUNT {
            if !EXTENSIONS[i].active || EXTENSIONS[i].hook_type != hook_type {
                continue;
            }

            let result = execute_extension(&mut EXTENSIONS[i], hook_type, context0, context1);

            match result {
                ExecResult::Ok => {
                    // Check log-output flag at scratch[250]
                    if EXTENSIONS[i].scratch[250] == 1 {
                        let v1 = u64::from_le_bytes(
                            EXTENSIONS[i].scratch[240..248].try_into().unwrap_or([0u8; 8]),
                        );
                        let v2 = u64::from_le_bytes(
                            EXTENSIONS[i].scratch[248..256].try_into().unwrap_or([0u8; 8]),
                        );
                        // Build a readable name from the byte array
                        let name_end = EXTENSIONS[i].name.iter().position(|&c| c == 0).unwrap_or(16);
                        let name_str = core::str::from_utf8(&EXTENSIONS[i].name[..name_end])
                            .unwrap_or("ext");
                        crate::println!(
                            "  [EXT:{}] hook={} v1={} v2={}",
                            name_str, hook_type, v1, v2
                        );
                    }
                }
                ExecResult::Timeout => {
                    let name_end = EXTENSIONS[i].name.iter().position(|&c| c == 0).unwrap_or(16);
                    let name_str = core::str::from_utf8(&EXTENSIONS[i].name[..name_end])
                        .unwrap_or("ext");
                    crate::println!(
                        "  [EXT:{}] TIMEOUT (1000 cycles exceeded) — disabled",
                        name_str
                    );
                    EXTENSIONS[i].active = false;
                }
                ExecResult::Error(e) => {
                    let name_end = EXTENSIONS[i].name.iter().position(|&c| c == 0).unwrap_or(16);
                    let name_str = core::str::from_utf8(&EXTENSIONS[i].name[..name_end])
                        .unwrap_or("ext");
                    crate::println!(
                        "  [EXT:{}] runtime error: {} — disabled",
                        name_str, e
                    );
                    EXTENSIONS[i].active = false;
                }
            }
        }
    }
}

// ── V32: Hybrid eBPF+WASM helpers ────────────────────────────────────────

/// Execute a single extension (by ID) without iterating all hooks.
/// Returns true if the extension executed successfully and remains active.
pub fn execute_single(ext_id: usize, hook_type: u8, context0: u64, context1: u64) -> bool {
    unsafe {
        if ext_id >= EXT_COUNT || !EXTENSIONS[ext_id].active {
            return false;
        }
        let result = execute_extension(&mut EXTENSIONS[ext_id], hook_type, context0, context1);
        match result {
            ExecResult::Ok => true,
            ExecResult::Timeout => {
                let name_end = EXTENSIONS[ext_id].name.iter().position(|&c| c == 0).unwrap_or(16);
                let name_str = core::str::from_utf8(&EXTENSIONS[ext_id].name[..name_end])
                    .unwrap_or("ext");
                crate::println!("  [EXT:{}] TIMEOUT (1000 cycles) — disabled by hybrid", name_str);
                EXTENSIONS[ext_id].active = false;
                false
            }
            ExecResult::Error(e) => {
                let name_end = EXTENSIONS[ext_id].name.iter().position(|&c| c == 0).unwrap_or(16);
                let name_str = core::str::from_utf8(&EXTENSIONS[ext_id].name[..name_end])
                    .unwrap_or("ext");
                crate::println!("  [EXT:{}] runtime error: {} — disabled by hybrid", name_str, e);
                EXTENSIONS[ext_id].active = false;
                false
            }
        }
    }
}

/// Read a single byte from an extension's scratch buffer.
pub fn read_scratch(ext_id: usize, offset: usize) -> u8 {
    unsafe {
        if ext_id < EXT_COUNT && offset < SCRATCH_SIZE {
            EXTENSIONS[ext_id].scratch[offset]
        } else {
            0
        }
    }
}

/// Read a little-endian u32 from an extension's scratch buffer.
pub fn read_scratch_u32(ext_id: usize, offset: usize) -> u32 {
    unsafe {
        if ext_id < EXT_COUNT && offset + 4 <= SCRATCH_SIZE {
            u32::from_le_bytes([
                EXTENSIONS[ext_id].scratch[offset],
                EXTENSIONS[ext_id].scratch[offset + 1],
                EXTENSIONS[ext_id].scratch[offset + 2],
                EXTENSIONS[ext_id].scratch[offset + 3],
            ])
        } else {
            0
        }
    }
}
