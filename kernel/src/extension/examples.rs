// V24: Example kernel extension bytecodes
//
// These are const byte arrays representing compiled extension bytecode.
// Each instruction is 8 bytes: [opcode, reg1, reg2, reg3, imm32_le...]
//
// Instruction reference:
//   0x01 MOV  dst, imm32       — dst = imm32              (reg1=dst)
//   0x02 ADD  dst, src1, src2  — dst = src1 + src2        (reg1=dst, reg2=src1, reg3=src2)
//   0x03 SUB  dst, src1, src2  — dst = src1 - src2        (reg1=dst, reg2=src1, reg3=src2)
//   0x04 CMP  src1, src2       — set eq = (src1 == src2)  (reg2=src1, reg3=src2)
//   0x05 JMP  imm32            — pc += imm32 (signed)     (offset in bytes)
//   0x06 JE   imm32            — if eq: pc += imm32
//   0x07 JNE  imm32            — if !eq: pc += imm32
//   0x08 LOAD dst, imm32       — dst = scratch[imm32]     (reg1=dst)
//   0x09 STORE src1, imm32     — scratch[imm32] = src1    (reg2=src1)
//   0x0A PUSH src1             — push src1 onto stack     (reg2=src1)
//   0x0B POP  dst              — pop into dst             (reg1=dst)
//   0x0C RET                   — stop execution
//
// Hook context convention:
//   scratch[0..8]  = context0 (primary value)
//   scratch[8..16] = context1 (secondary value)
//   reg[30] = hook_type
//   reg[31] = context0
//
// Log output convention (checked after execution):
//   scratch[250] == 1  → log output available
//   scratch[240..248]  → log value 1 (v1)
//   scratch[248..256]  → log value 2 (v2)
//
// Persistent scratch (survives between hook fires):
//   scratch[128..192] — for counters and state

use crate::extension;

// ── Helper: encode a single instruction ────────────────────────────────────────
// Used at compile time via const functions. We define instruction constructors
// as simple functions returning [u8; 8] for readability.

/// MOV dst, imm32 — load immediate into register
const fn mov(dst: u8, imm: u32) -> [u8; 8] {
    [
        0x01, dst, 0x00, 0x00,
        imm as u8, (imm >> 8) as u8, (imm >> 16) as u8, (imm >> 24) as u8,
    ]
}

/// ADD dst, src1, src2 — dst = src1 + src2
const fn add(dst: u8, src1: u8, src2: u8) -> [u8; 8] {
    [0x02, dst, src1, src2, 0x00, 0x00, 0x00, 0x00]
}

/// SUB dst, src1, src2 — dst = src1 - src2
const fn sub(dst: u8, src1: u8, src2: u8) -> [u8; 8] {
    [0x03, dst, src1, src2, 0x00, 0x00, 0x00, 0x00]
}

/// CMP src1, src2 — set eq flag if src1 == src2
const fn cmp(src1: u8, src2: u8) -> [u8; 8] {
    [0x04, 0x00, src1, src2, 0x00, 0x00, 0x00, 0x00]
}

/// JMP imm32 — unconditional jump (signed byte offset)
const fn jmp(offset: u32) -> [u8; 8] {
    [
        0x05, 0x00, 0x00, 0x00,
        offset as u8, (offset >> 8) as u8, (offset >> 16) as u8, (offset >> 24) as u8,
    ]
}

/// JE imm32 — jump if equal
const fn je(offset: u32) -> [u8; 8] {
    [
        0x06, 0x00, 0x00, 0x00,
        offset as u8, (offset >> 8) as u8, (offset >> 16) as u8, (offset >> 24) as u8,
    ]
}

/// JNE imm32 — jump if not equal
const fn jne(offset: u32) -> [u8; 8] {
    [
        0x07, 0x00, 0x00, 0x00,
        offset as u8, (offset >> 8) as u8, (offset >> 16) as u8, (offset >> 24) as u8,
    ]
}

/// LOAD dst, offset — load u64 from scratch[offset]
const fn load(dst: u8, offset: u32) -> [u8; 8] {
    [
        0x08, dst, 0x00, 0x00,
        offset as u8, (offset >> 8) as u8, (offset >> 16) as u8, (offset >> 24) as u8,
    ]
}

/// STORE src, offset — store u64 to scratch[offset]
const fn store(src: u8, offset: u32) -> [u8; 8] {
    [
        0x09, 0x00, src, 0x00,
        offset as u8, (offset >> 8) as u8, (offset >> 16) as u8, (offset >> 24) as u8,
    ]
}

/// PUSH src — push register onto execution stack
const fn push(src: u8) -> [u8; 8] {
    [0x0A, 0x00, src, 0x00, 0x00, 0x00, 0x00, 0x00]
}

/// POP dst — pop from execution stack into register
const fn pop(dst: u8) -> [u8; 8] {
    [0x0B, dst, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
}

/// RET — stop execution
const fn ret() -> [u8; 8] {
    [0x0C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
}

// ── Concatenate instructions into a bytecode array ─────────────────────────────

/// Helper macro to concatenate instruction arrays into a single bytecode slice.
macro_rules! bytecode {
    ($($inst:expr),* $(,)?) => {{
        const BYTES: &[u8] = &{
            const INSTRS: &[[u8; 8]] = &[$($inst),*];
            const LEN: usize = INSTRS.len() * 8;
            const fn flatten(instrs: &[[u8; 8]]) -> [u8; LEN] {
                let mut result = [0u8; LEN];
                let mut i = 0;
                while i < instrs.len() {
                    let mut j = 0;
                    while j < 8 {
                        result[i * 8 + j] = instrs[i][j];
                        j += 1;
                    }
                    i += 1;
                }
                result
            }
            flatten(INSTRS)
        };
        BYTES
    }};
}

// ── Extension 1: Syscall Tracer (strace-like) ─────────────────────────────────
//
// Hook: SYSCALL_ENTER
// Logs every syscall number and PID to the kernel console.
// Context: scratch[0..8] = nr, scratch[8..16] = pid

pub const SYSCALL_TRACER_NAME: &[u8] = b"strace";
pub const SYSCALL_TRACER_BYTECODE: &[u8] = bytecode![
    // LOAD r1, 0      → r1 = syscall number from scratch[0]
    load(1, 0),
    // LOAD r2, 8      → r2 = pid from scratch[8]
    load(2, 8),
    // STORE r1, 240   → scratch[240] = nr (log v1)
    store(1, 240),
    // STORE r2, 248   → scratch[248] = pid (log v2)
    store(2, 248),
    // MOV r0, 1
    mov(0, 1),
    // STORE r0, 250   → scratch[250] = 1 (set log flag)
    store(0, 250),
    // RET
    ret(),
];

// ── Extension 2: Packet Counter (tcpdump-like) ─────────────────────────────────
//
// Hook: IPC_SEND
// Counts IPC send operations. Counter stored in scratch[128..136] persists
// across invocations and is exposed via the extension list API.
// Context: scratch[0..8] = ep_id, scratch[8..16] = sender_pid

pub const PACKET_COUNTER_NAME: &[u8] = b"pktcnt";
pub const PACKET_COUNTER_BYTECODE: &[u8] = bytecode![
    // LOAD r0, 128    → r0 = current count from scratch[128]
    load(0, 128),
    // MOV r1, 1
    mov(1, 1),
    // ADD r0, r0, r1  → r0++
    add(0, 0, 1),
    // STORE r0, 128   → save count back to scratch[128]
    store(0, 128),
    // STORE r0, 248   → scratch[248] = count (log v2)
    store(0, 248),
    // MOV r1, 1
    mov(1, 1),
    // STORE r1, 250   → scratch[250] = 1 (set log flag)
    store(1, 250),
    // RET
    ret(),
];

// ── Extension 3: Performance Monitor ──────────────────────────────────────────
//
// Hook: TIMER
// Tracks ticks elapsed between successive TIMER hook fires.
// Stores "previous tick" in scratch[136..144].
// Context: scratch[0..8] = tick_count

pub const PERF_MONITOR_NAME: &[u8] = b"perfmon";
pub const PERF_MONITOR_BYTECODE: &[u8] = bytecode![
    // LOAD r0, 0      → r0 = current tick from scratch[0]
    load(0, 0),
    // LOAD r1, 136    → r1 = previous tick from scratch[136]
    load(1, 136),
    // SUB r0, r0, r1  → r0 = elapsed = current - previous
    sub(0, 0, 1),
    // STORE r0, 240   → scratch[240] = elapsed ticks (log v1)
    store(0, 240),
    // LOAD r0, 0      → r0 = current tick (reload)
    load(0, 0),
    // STORE r0, 136   → scratch[136] = current tick (update "previous")
    store(0, 136),
    // MOV r1, 1
    mov(1, 1),
    // STORE r1, 250   → scratch[250] = 1 (set log flag)
    store(1, 250),
    // RET
    ret(),
];

// ── Registration Helpers ──────────────────────────────────────────────────────

/// Register the syscall tracer extension (must be called from a process context).
pub fn register_syscall_tracer() -> Option<usize> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    extension::register(
        pid,
        extension::HOOK_SYSCALL_ENTER,
        SYSCALL_TRACER_BYTECODE,
        SYSCALL_TRACER_NAME,
    )
}

/// Register the packet counter extension.
pub fn register_packet_counter() -> Option<usize> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    extension::register(
        pid,
        extension::HOOK_IPC_SEND,
        PACKET_COUNTER_BYTECODE,
        PACKET_COUNTER_NAME,
    )
}

/// Register the performance monitor extension.
pub fn register_perf_monitor() -> Option<usize> {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    extension::register(
        pid,
        extension::HOOK_TIMER,
        PERF_MONITOR_BYTECODE,
        PERF_MONITOR_NAME,
    )
}
