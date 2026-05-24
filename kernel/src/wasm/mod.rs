// V28: WASM/WASI Universal Runtime Subsystem
//
// Features:
//   - WASM module loading and validation
//   - Stack-based WASM interpreter with cycle budget (10000 insn/invocation)
//   - Linear memory (64KB per module, up to 256KB via memory.grow)
//   - Function export/import with host function table
//   - WASI preview2 system interface stubs (in wasi.rs)
//   - Single address space execution model (libOS mode, in libos.rs)
//
// Architecture:
//   - 8 module slots, 4KB bytecode per module
//   - Heap-allocated linear memory per module (64KB min, 256KB max)
//   - 256-slot value stack (i64 entries, i32 stored in low 32 bits)
//   - 32-frame call stack with 16 locals per frame
//   - 32-entry control stack for structured control flow (block/loop/if)
//   - 16-entry host function table for native Rust callbacks

use alloc::alloc::{alloc, alloc_zeroed, dealloc, Layout};

pub mod wasi;
pub mod libos;
pub mod hostcall;
pub mod hybrid;

// ── Constants ─────────────────────────────────────────────────────────────

pub(crate) const MAX_WASM_MODULES: usize = 8;
const MAX_WASM_SIZE: usize = 4096;
pub(crate) const MEM_PAGE_SIZE: usize = 65536;   // 64KB per WASM memory page
const MAX_MEM_PAGES: usize = 4;       // 256KB max
const VALUE_STACK_SIZE: usize = 256;
const CALL_STACK_SIZE: usize = 32;
const MAX_LOCALS: usize = 16;
const CYCLE_BUDGET: u32 = 10000;
const MAX_FUNCTIONS: usize = 64;
const MAX_TYPES: usize = 16;
const MAX_EXPORTS: usize = 32;
const MAX_HOST_FUNCS: usize = 16;
const MAX_IMPORTS: usize = 16;

// ── V32: Performance Statistics ──────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct WasmPerfStats {
    pub invocations: u64,
    pub total_cycles: u64,
    pub avg_cycles: u64,
    pub max_cycles: u64,
    pub syscall_forward_count: u64,
}

impl WasmPerfStats {
    pub const fn new() -> Self {
        WasmPerfStats {
            invocations: 0,
            total_cycles: 0,
            avg_cycles: 0,
            max_cycles: 0,
            syscall_forward_count: 0,
        }
    }

    pub fn record_invocation(&mut self, cycles: u64) {
        self.invocations += 1;
        self.total_cycles = self.total_cycles.wrapping_add(cycles);
        if cycles > self.max_cycles {
            self.max_cycles = cycles;
        }
        self.avg_cycles = if self.invocations > 0 {
            self.total_cycles / self.invocations
        } else {
            0
        };
    }

    pub fn reset(&mut self) {
        self.invocations = 0;
        self.total_cycles = 0;
        self.avg_cycles = 0;
        self.max_cycles = 0;
        self.syscall_forward_count = 0;
    }
}


// ── Control frame kinds ───────────────────────────────────────────────────

const CTRL_FUNC: u8 = 0;
const CTRL_BLOCK: u8 = 1;
const CTRL_LOOP: u8 = 2;
const CTRL_IF: u8 = 3;

// ── Module Data ───────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub(crate) struct WasmModule {
    name: [u8; 32],
    pid: u32,
    bytecode: [u8; MAX_WASM_SIZE],
    bc_len: usize,
    loaded: bool,
    // ── Type section ──
    num_types: usize,
    /// Number of parameters per type signature
    type_params: [u8; MAX_TYPES],
    /// Number of results per type signature
    type_results: [u8; MAX_TYPES],
    // ── Import section ──
    num_imports: usize,
    import_names: [[u8; 32]; MAX_IMPORTS],
    import_types: [u8; MAX_IMPORTS],
    // ── Function section (defined functions only) ──
    num_functions: usize,
    func_types: [u8; MAX_FUNCTIONS],
    // ── Code section ──
    code_offsets: [usize; MAX_FUNCTIONS],
    code_sizes: [usize; MAX_FUNCTIONS],
    num_locals: [u8; MAX_FUNCTIONS],
    // ── Export section ──
    num_exports: usize,
    export_names: [[u8; 32]; MAX_EXPORTS],
    /// function index (imported or defined) for each export
    export_funcs: [u8; MAX_EXPORTS],
    // ── Memory (heap-allocated) ──
    mem_ptr: usize,
    mem_pages: usize,
    mem_max_pages: usize,
}

// ── Frame for the call stack ──────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Frame {
    func_idx: usize,
    pc: usize,
    sp: usize,
    locals: [i64; MAX_LOCALS],
}

// ── Control frame ─────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct CtrlFrame {
    kind: u8,
    pc_cont: usize,
    sp: usize,
}

// ── Host function table ───────────────────────────────────────────────────

/// Host function type: receives a slice of i64 arguments, returns one i64.
type HostFunc = fn(&[i64]) -> i64;

#[derive(Clone, Copy)]
struct HostFuncEntry {
    name: [u8; 32],
    name_len: u8,
    func: Option<HostFunc>,
}

impl HostFuncEntry {
    const fn empty() -> Self {
        HostFuncEntry { name: [0u8; 32], name_len: 0, func: None }
    }
}

// ── Global state ──────────────────────────────────────────────────────────

pub(crate) static mut WASM_MODULES: [WasmModule; MAX_WASM_MODULES] = [
    WasmModule {
        name: [0; 32],
        pid: 0,
        bytecode: [0; MAX_WASM_SIZE],
        bc_len: 0,
        loaded: false,
        num_types: 0,
        type_params: [0; MAX_TYPES],
        type_results: [0; MAX_TYPES],
        num_imports: 0,
        import_names: [[0; 32]; MAX_IMPORTS],
        import_types: [0; MAX_IMPORTS],
        num_functions: 0,
        func_types: [0; MAX_FUNCTIONS],
        code_offsets: [0; MAX_FUNCTIONS],
        code_sizes: [0; MAX_FUNCTIONS],
        num_locals: [0; MAX_FUNCTIONS],
        num_exports: 0,
        export_names: [[0; 32]; MAX_EXPORTS],
        export_funcs: [0; MAX_EXPORTS],
        mem_ptr: 0,
        mem_pages: 0,
        mem_max_pages: 0,
    }; MAX_WASM_MODULES
];
static mut WASM_COUNT: usize = 0;

static mut HOST_FUNCS: [HostFuncEntry; MAX_HOST_FUNCS] = [
    HostFuncEntry::empty(); MAX_HOST_FUNCS
];
static mut HOST_FUNC_COUNT: usize = 0;

// V32: Per-module performance statistics (indexed by module ID)
static mut WASM_PERF_STATS: [WasmPerfStats; MAX_WASM_MODULES] = [
    WasmPerfStats::new(); MAX_WASM_MODULES
];


// ══════════════════════════════════════════════════════════════════════════
//  LEB128 Helpers
// ══════════════════════════════════════════════════════════════════════════

/// Decode an unsigned LEB128 value, advancing `offset`.
fn leb128_u(buf: &[u8], offset: &mut usize) -> u32 {
    let mut result: u32 = 0;
    let mut shift: u32 = 0;
    loop {
        let byte = buf[*offset];
        *offset += 1;
        result |= ((byte & 0x7F) as u32) << shift;
        if byte & 0x80 == 0 {
            return result;
        }
        shift += 7;
    }
}

/// Decode a signed LEB128 (32-bit) value, advancing `offset`.
fn leb128_s(buf: &[u8], offset: &mut usize) -> i32 {
    let mut result: i32 = 0;
    let mut shift: u32 = 0;
    loop {
        let byte = buf[*offset];
        *offset += 1;
        result |= ((byte & 0x7F) as i32) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            if shift < 32 && (byte & 0x40) != 0 {
                result |= !0i32 << shift;
            }
            return result;
        }
    }
}

/// Decode a signed LEB128 (64-bit) value, advancing `offset`.
fn leb128_s64(buf: &[u8], offset: &mut usize) -> i64 {
    let mut result: i64 = 0;
    let mut shift: u32 = 0;
    loop {
        let byte = buf[*offset];
        *offset += 1;
        result |= ((byte & 0x7F) as i64) << shift;
        shift += 7;
        if byte & 0x80 == 0 {
            if shift < 64 && (byte & 0x40) != 0 {
                result |= !0i64 << shift;
            }
            return result;
        }
    }
}

/// Number of LEB128-encoded bytes starting at `offset` (unsigned).
fn leb128_size(buf: &[u8], mut offset: usize) -> usize {
    let start = offset;
    while offset < buf.len() && buf[offset] & 0x80 != 0 {
        offset += 1;
    }
    offset - start + 1
}

/// Number of LEB128-encoded bytes starting at `offset` (signed, same as unsigned).
fn leb128_s_size(buf: &[u8], offset: usize) -> usize {
    leb128_size(buf, offset)
}

// ══════════════════════════════════════════════════════════════════════════
//  Section Parsers
// ══════════════════════════════════════════════════════════════════════════

/// Parse the WASM type section (section ID 1).
/// Each entry: 0x60 param_count(LEB) [param_types...] result_count(LEB) [result_types...]
fn parse_type_section(module_id: usize, bc: &[u8], mut offset: usize, size: usize) {
    let end = offset + size;
    let mut count = 0usize;
    while offset < end && count < MAX_TYPES {
        let _func_type = bc[offset]; offset += 1; // 0x60
        if _func_type != 0x60 { break; }
        let nparams = leb128_u(bc, &mut offset) as u8;
        unsafe { WASM_MODULES[module_id].type_params[count] = nparams; }
        for _ in 0..nparams {
            offset += 1; // skip param type byte
        }
        let nresults = leb128_u(bc, &mut offset) as u8;
        unsafe { WASM_MODULES[module_id].type_results[count] = nresults; }
        for _ in 0..nresults {
            offset += 1; // skip result type byte
        }
        count += 1;
    }
    unsafe { WASM_MODULES[module_id].num_types = count; }
}

/// Parse the WASM import section (section ID 2).
/// Each entry: mod_str(LEB+bytes) field_str(LEB+bytes) kind(1byte) [type_idx]
fn parse_import_section(module_id: usize, bc: &[u8], mut offset: usize, size: usize) {
    let end = offset + size;
    let mut count = 0usize;
    while offset < end && count < MAX_IMPORTS {
        let mod_len = leb128_u(bc, &mut offset) as usize;
        offset += mod_len; // skip module string
        let field_len = leb128_u(bc, &mut offset) as usize;
        let field_start = offset;
        offset += field_len;
        // Store import name
        {
            let name_len = field_len.min(31);
            unsafe {
                for j in 0..name_len {
                    WASM_MODULES[module_id].import_names[count][j] = bc[field_start + j];
                }
                WASM_MODULES[module_id].import_names[count][name_len] = 0;
            }
        }
        let kind = bc[offset]; offset += 1;
        if kind == 0 {
            // Function import: read type index
            let type_idx = leb128_u(bc, &mut offset) as u8;
            unsafe {
                WASM_MODULES[module_id].import_types[count] = type_idx;
            }
        }
        count += 1;
    }
    unsafe { WASM_MODULES[module_id].num_imports = count; }
}

/// Parse the WASM function section (section ID 3).
/// Each entry: type_index(LEB)
fn parse_function_section(module_id: usize, bc: &[u8], mut offset: usize, size: usize) {
    let end = offset + size;
    let mut count = 0usize;
    while offset < end && count < MAX_FUNCTIONS {
        let type_idx = leb128_u(bc, &mut offset) as u8;
        unsafe { WASM_MODULES[module_id].func_types[count] = type_idx; }
        count += 1;
    }
    unsafe { WASM_MODULES[module_id].num_functions = count; }
}

/// Parse the WASM export section (section ID 7).
/// Each entry: name_str(LEB+bytes) kind(1byte) index(LEB)
fn parse_export_section(module_id: usize, bc: &[u8], mut offset: usize, size: usize) {
    let end = offset + size;
    let mut count = 0usize;
    while offset < end && count < MAX_EXPORTS {
        let name_len = leb128_u(bc, &mut offset) as usize;
        let name_start = offset;
        offset += name_len;
        {
            let clen = name_len.min(31);
            unsafe {
                for j in 0..clen {
                    WASM_MODULES[module_id].export_names[count][j] = bc[name_start + j];
                }
                WASM_MODULES[module_id].export_names[count][clen] = 0;
            }
        }
        let _kind = bc[offset]; offset += 1;
        let idx = leb128_u(bc, &mut offset) as u8;
        unsafe {
            WASM_MODULES[module_id].export_funcs[count] = idx;
        }
        count += 1;
    }
    unsafe { WASM_MODULES[module_id].num_exports = count; }
}

/// Parse the WASM code section (section ID 10).
/// Each entry: body_size(LEB) local_decls... code_body
/// local_decls: count(LEB) type(1byte) repeated
fn parse_code_section(module_id: usize, bc: &[u8], mut offset: usize, size: usize) {
    let end = offset + size;
    let mut func_idx = 0usize;
    while offset < end && func_idx < MAX_FUNCTIONS {
        let _body_size = leb128_u(bc, &mut offset) as usize;
        let body_start = offset;

        // Parse local declarations
        let num_locals_decls = leb128_u(bc, &mut offset) as u32;
        let mut total_locals: u8 = 0;
        for _ in 0..num_locals_decls {
            let count = leb128_u(bc, &mut offset) as u8;
            let _ty = bc[offset]; offset += 1; // value type (0x7F=i32, 0x7E=i64)
            total_locals = total_locals.saturating_add(count);
        }
        let code_start = offset;
        unsafe {
            WASM_MODULES[module_id].code_offsets[func_idx] = code_start;
            WASM_MODULES[module_id].num_locals[func_idx] = total_locals;
            // body_size includes local decls + code; let's calculate actual size
            WASM_MODULES[module_id].code_sizes[func_idx] = end.min(body_start + _body_size as usize) - code_start;
        }
        func_idx += 1;
        // Ensure we advance past the code body
        offset = end.min(body_start + _body_size as usize);
    }
}

/// Parse all sections of a loaded WASM module.
fn parse_module(module_id: usize) {
    let bc;
    let bc_len;
    unsafe {
        bc = &WASM_MODULES[module_id].bytecode;
        bc_len = WASM_MODULES[module_id].bc_len;
    }
    let mut offset = 8usize; // skip magic (4) + version (4)

    while offset < bc_len {
        let section_id = bc[offset];
        offset += 1;
        let section_size = leb128_u(bc, &mut offset) as usize;
        let section_end = offset + section_size;

        match section_id {
            1 => parse_type_section(module_id, bc, offset, section_size),
            2 => parse_import_section(module_id, bc, offset, section_size),
            3 => parse_function_section(module_id, bc, offset, section_size),
            7 => parse_export_section(module_id, bc, offset, section_size),
            10 => parse_code_section(module_id, bc, offset, section_size),
            _ => {} // skip unknown sections
        }

        offset = section_end;
    }
}

// ══════════════════════════════════════════════════════════════════════════
//  Memory Management
// ══════════════════════════════════════════════════════════════════════════

const MEM_LAYOUT: Layout = match Layout::from_size_align(MEM_PAGE_SIZE, 16) {
    Ok(l) => l,
    Err(_) => panic!(), // unreachable
};

/// Allocate initial linear memory for a module (1 page = 64KB).
fn alloc_module_memory(module_id: usize) -> bool {
    unsafe {
        let ptr = alloc_zeroed(MEM_LAYOUT);
        if ptr.is_null() { return false; }
        WASM_MODULES[module_id].mem_ptr = ptr as usize;
        WASM_MODULES[module_id].mem_pages = 1;
        WASM_MODULES[module_id].mem_max_pages = MAX_MEM_PAGES;
        true
    }
}

/// Free a module's linear memory.
fn free_module_memory(module_id: usize) {
    unsafe {
        let ptr = WASM_MODULES[module_id].mem_ptr;
        if ptr != 0 {
            dealloc(ptr as *mut u8, MEM_LAYOUT);
            WASM_MODULES[module_id].mem_ptr = 0;
            WASM_MODULES[module_id].mem_pages = 0;
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════
//  Bytecode Scanning Helpers
// ══════════════════════════════════════════════════════════════════════════

/// Skip past a LEB128 immediate at `pc`, advance pc.
fn skip_leb128(bc: &[u8], pc: &mut usize) {
    while *pc < bc.len() && bc[*pc] & 0x80 != 0 { *pc += 1; }
    *pc += 1;
}

/// Find the continuation PC after the matching `end` for a block/loop/if
/// that starts at `pc` (pc points to opcode 0x02/0x03/0x04).
/// Returns the PC *after* the matching `end`.
fn find_matching_end(bc: &[u8], mut pc: usize) -> Result<usize, &'static str> {
    if pc >= bc.len() { return Err("find_end: pc OOB"); }
    pc += 1; // skip opcode
    pc += 1; // skip block type byte
    let mut depth = 1u32;

    while pc < bc.len() {
        match bc[pc] {
            // Structured control: increase depth
            0x02 | 0x03 | 0x04 => { depth += 1; pc += 1; pc += 1; } // block/loop/if + blocktype
            // End: decrease depth
            0x0B => {
                depth -= 1;
                pc += 1;
                if depth == 0 { return Ok(pc); }
            }
            // Else: if in an if at depth 1, it's the else for our block
            0x05 => { pc += 1; }
            // br / br_if: skip label (LEB128)
            0x0C | 0x0D => { pc += 1; skip_leb128(bc, &mut pc); }
            // call: skip funcidx (LEB128)
            0x10 => { pc += 1; skip_leb128(bc, &mut pc); }
            // local.get / local.set: skip localidx (LEB128)
            0x20 | 0x21 => { pc += 1; skip_leb128(bc, &mut pc); }
            // i32.const / i64.const: skip immediate (LEB)
            0x41 | 0x42 => { pc += 1; skip_leb128(bc, &mut pc); }
            // Memory load/store: skip align + offset (2x LEB128)
            0x28 | 0x29 | 0x2C | 0x2D | 0x2E | 0x2F
            | 0x30 | 0x31 | 0x32 | 0x33 | 0x34 | 0x35 => {
                pc += 1;
                skip_leb128(bc, &mut pc); // align
                skip_leb128(bc, &mut pc); // offset
            }
            0x36 | 0x37 | 0x3A | 0x3B | 0x3C | 0x3D | 0x3E => {
                pc += 1;
                skip_leb128(bc, &mut pc); // align
                skip_leb128(bc, &mut pc); // offset
            }
            // memory.size / memory.grow: skip 0x00
            0x3F | 0x40 => { pc += 2; }
            // return / nop / unreachable
            0x00 | 0x01 | 0x05 => { pc += 1; }
            // Comparisons
            0x45 | 0x46 | 0x47 | 0x48 | 0x49 | 0x4A | 0x4B | 0x4C | 0x4D | 0x4E | 0x4F => { pc += 1; }
            // Arithmetic / bitwise
            0x6A | 0x6B | 0x6C | 0x6D | 0x6E | 0x6F | 0x70
            | 0x71 | 0x72 | 0x73 | 0x74 | 0x75 | 0x76 | 0x77 | 0x78 => { pc += 1; }
            // Type conversion
            0xA7 | 0xAC | 0xAD => { pc += 1; }
            // Any other single-byte opcode
            _ => { pc += 1; }
        }
    }
    Err("find_end: end not found")
}

/// Find both else and end positions for an `if` at `pc`.
/// Returns (else_pc, end_pc) where else_pc may be 0 if no else block.
fn find_if_else_end(bc: &[u8], mut pc: usize) -> Result<(usize, usize), &'static str> {
    if pc >= bc.len() { return Err("if_else_end: pc OOB"); }
    pc += 1; // skip opcode
    pc += 1; // skip block type
    let mut depth = 1u32;
    let mut else_pc: usize = 0;

    while pc < bc.len() {
        match bc[pc] {
            0x02 | 0x03 | 0x04 => { depth += 1; pc += 1; pc += 1; }
            0x05 => {
                if depth == 1 && else_pc == 0 { else_pc = pc; }
                pc += 1;
            }
            0x0B => {
                depth -= 1;
                pc += 1;
                if depth == 0 { return Ok((else_pc, pc)); }
            }
            0x0C | 0x0D => { pc += 1; skip_leb128(bc, &mut pc); }
            0x10 => { pc += 1; skip_leb128(bc, &mut pc); }
            0x20 | 0x21 => { pc += 1; skip_leb128(bc, &mut pc); }
            0x41 | 0x42 => { pc += 1; skip_leb128(bc, &mut pc); }
            0x28 | 0x29 | 0x2C | 0x2D | 0x2E | 0x2F
            | 0x30 | 0x31 | 0x32 | 0x33 | 0x34 | 0x35 => {
                pc += 1; skip_leb128(bc, &mut pc); skip_leb128(bc, &mut pc);
            }
            0x36 | 0x37 | 0x3A | 0x3B | 0x3C | 0x3D | 0x3E => {
                pc += 1; skip_leb128(bc, &mut pc); skip_leb128(bc, &mut pc);
            }
            0x3F | 0x40 => { pc += 2; }
            0x00 | 0x01 | 0x05 => { pc += 1; }
            0x45..=0x4F => { pc += 1; }
            0x6A..=0x78 => { pc += 1; }
            0xA7 | 0xAC | 0xAD => { pc += 1; }
            _ => { pc += 1; }
        }
    }
    Err("if_else_end: end not found")
}

// ══════════════════════════════════════════════════════════════════════════
//  Public API — Module Management
// ══════════════════════════════════════════════════════════════════════════

/// Validate WASM magic number and version.
pub fn wasm_validate(bytecode: &[u8]) -> bool {
    bytecode.len() >= 8
        && bytecode[0] == 0x00 && bytecode[1] == 0x61
        && bytecode[2] == 0x73 && bytecode[3] == 0x6D
        && bytecode[4] == 0x01 && bytecode[5] == 0x00
        && bytecode[6] == 0x00 && bytecode[7] == 0x00
}

/// Load a WASM module. Parses sections and allocates linear memory.
/// Returns the module id on success.
pub fn wasm_load(pid: u32, name: &[u8], bytecode: &[u8]) -> Option<usize> {
    if !wasm_validate(bytecode) {
        crate::println!("  WASM: invalid magic/version");
        return None;
    }
    unsafe {
        if WASM_COUNT >= MAX_WASM_MODULES { return None; }
        let id = WASM_COUNT;

        // Copy name
        let nlen = name.len().min(31);
        for i in 0..nlen { WASM_MODULES[id].name[i] = name[i]; }
        WASM_MODULES[id].name[nlen] = 0;

        // Copy bytecode
        let len = bytecode.len().min(MAX_WASM_SIZE);
        for i in 0..len { WASM_MODULES[id].bytecode[i] = bytecode[i]; }
        WASM_MODULES[id].bc_len = len;
        WASM_MODULES[id].pid = pid;
        WASM_MODULES[id].loaded = true;

        // Parse sections
        parse_module(id);

        // Allocate initial linear memory
        if !alloc_module_memory(id) {
            // Memory allocation failed, mark as not loaded
            WASM_MODULES[id].loaded = false;
            return None;
        }

        WASM_COUNT += 1;
        crate::println!("  WASM: loaded module id={} name={} funcs={} exports={}",
            id, core::str::from_utf8(name).unwrap_or("?"),
            WASM_MODULES[id].num_functions, WASM_MODULES[id].num_exports);
        Some(id)
    }
}

/// Unload a WASM module, freeing its memory.
pub fn wasm_unload(module_id: usize) -> bool {
    unsafe {
        if module_id >= MAX_WASM_MODULES || !WASM_MODULES[module_id].loaded {
            return false;
        }
        free_module_memory(module_id);
        WASM_MODULES[module_id].loaded = false;
        WASM_MODULES[module_id].bc_len = 0;
        true
    }
}

/// List loaded WASM modules into a buffer.
/// Format per entry: [pid:4][name:32] = 36 bytes each.
pub fn wasm_list(buf: &mut [u8]) -> usize {
    unsafe {
        let mut pos = 0;
        for i in 0..WASM_COUNT {
            if WASM_MODULES[i].loaded && pos + 36 <= buf.len() {
                let pid = WASM_MODULES[i].pid;
                buf[pos] = pid as u8;
                buf[pos+1] = (pid >> 8) as u8;
                buf[pos+2] = (pid >> 16) as u8;
                buf[pos+3] = (pid >> 24) as u8;
                for j in 0..32 { buf[pos+4+j] = WASM_MODULES[i].name[j]; }
                pos += 36;
            }
        }
        pos
    }
}

// ══════════════════════════════════════════════════════════════════════════
//  Public API — Host Function Registration
// ══════════════════════════════════════════════════════════════════════════

/// Register a native Rust function as a host function callable from WASM.
pub fn wasm_register_host_func(name: &[u8], func: HostFunc) -> bool {
    unsafe {
        if HOST_FUNC_COUNT >= MAX_HOST_FUNCS { return false; }
        let nlen = name.len().min(31);
        for i in 0..nlen { HOST_FUNCS[HOST_FUNC_COUNT].name[i] = name[i]; }
        HOST_FUNCS[HOST_FUNC_COUNT].name[nlen] = 0;
        HOST_FUNCS[HOST_FUNC_COUNT].name_len = nlen as u8;
        HOST_FUNCS[HOST_FUNC_COUNT].func = Some(func);
        HOST_FUNC_COUNT += 1;
        true
    }
}

// ══════════════════════════════════════════════════════════════════════════
//  Public API — Memory Access
// ══════════════════════════════════════════════════════════════════════════

/// Read from a module's linear memory into a buffer.
/// All accesses are bounds-checked against the module's current memory size.
pub fn wasm_memory_read(module_id: usize, offset: usize, buf: &mut [u8]) -> Result<(), &'static str> {
    unsafe {
        if module_id >= MAX_WASM_MODULES || !WASM_MODULES[module_id].loaded {
            return Err("invalid module");
        }
        let mem_ptr = WASM_MODULES[module_id].mem_ptr;
        let mem_size = WASM_MODULES[module_id].mem_pages * MEM_PAGE_SIZE;
        if offset + buf.len() > mem_size {
            return Err("memory read out of bounds");
        }
        if mem_ptr == 0 {
            return Err("no memory allocated");
        }
        let src = core::slice::from_raw_parts(mem_ptr as *const u8, mem_size);
        buf.copy_from_slice(&src[offset..offset + buf.len()]);
        Ok(())
    }
}

/// Write to a module's linear memory from a buffer.
/// All accesses are bounds-checked against the module's current memory size.
pub fn wasm_memory_write(module_id: usize, offset: usize, data: &[u8]) -> Result<(), &'static str> {
    unsafe {
        if module_id >= MAX_WASM_MODULES || !WASM_MODULES[module_id].loaded {
            return Err("invalid module");
        }
        let mem_ptr = WASM_MODULES[module_id].mem_ptr;
        let mem_size = WASM_MODULES[module_id].mem_pages * MEM_PAGE_SIZE;
        if offset + data.len() > mem_size {
            return Err("memory write out of bounds");
        }
        if mem_ptr == 0 {
            return Err("no memory allocated");
        }
        let dst = core::slice::from_raw_parts_mut(mem_ptr as *mut u8, mem_size);
        dst[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }
}

// ══════════════════════════════════════════════════════════════════════════
//  Public API — Execute Exported Function
// ══════════════════════════════════════════════════════════════════════════

/// Look up an exported function by name and return its (global) function index.
/// Returns None if not found.
pub(crate) fn find_export(module_id: usize, name: &str) -> Option<u8> {
    unsafe {
        for i in 0..WASM_MODULES[module_id].num_exports {
            let ename = &WASM_MODULES[module_id].export_names[i];
            let mut matches = true;
            for (j, &c1) in name.as_bytes().iter().enumerate() {
                if ename[j] != c1 { matches = false; break; }
                if c1 == 0 { break; }
            }
            if matches && name.as_bytes().len() <= 32 {
                // Also check that the name in the struct is null-terminated at the right place
                if name.as_bytes().len() < 32 && ename[name.as_bytes().len()] != 0 {
                    // Name in module is longer, only match if it's exactly the same length
                    let mut longer = false;
                    for j in name.as_bytes().len()..32 {
                        if ename[j] != 0 { longer = true; break; }
                    }
                    if longer { matches = false; }
                }
            }
            if matches {
                return Some(WASM_MODULES[module_id].export_funcs[i]);
            }
        }
    }
    None
}

/// Execute an exported WASM function by name.
/// `args` are i64 values pushed onto the value stack before entry.
/// Returns the top-of-stack i32 result.
pub fn wasm_execute(module_id: usize, function_name: &str, args: &[i64]) -> Result<i32, &'static str> {
    unsafe {
        if module_id >= MAX_WASM_MODULES || !WASM_MODULES[module_id].loaded {
            return Err("invalid module id");
        }
        let module_ref = &WASM_MODULES[module_id];

        // Look up the export
        let global_func_idx = match find_export(module_id, function_name) {
            Some(idx) => idx as usize,
            None => return Err("export not found"),
        };
        let num_imports = module_ref.num_imports;

        if global_func_idx < num_imports {
            return Err("cannot execute imported function directly");
        }
        let def_idx = global_func_idx - num_imports;
        if def_idx >= module_ref.num_functions {
            return Err("function index out of range");
        }

        // Set up value stack
        let mut value_stack: [i64; VALUE_STACK_SIZE] = [0i64; VALUE_STACK_SIZE];
        let mut sp: usize = 0;

        // Push arguments
        for &a in args {
            if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
            value_stack[sp] = a;
            sp += 1;
        }

        // Set up call stack
        let mut call_stack: [Frame; CALL_STACK_SIZE] = [
            Frame { func_idx: 0, pc: 0, sp: 0, locals: [0i64; MAX_LOCALS] };
            CALL_STACK_SIZE
        ];
        let mut fp: usize = 0;

        // Set up control stack
        let mut ctrl_stack: [CtrlFrame; CALL_STACK_SIZE] = [
            CtrlFrame { kind: 0, pc_cont: 0, sp: 0 };
            CALL_STACK_SIZE
        ];
        let mut cp: usize = 1;

        // Initialize root frame
        let func_type_idx = module_ref.func_types[def_idx] as usize;
        let num_params = module_ref.type_params.get(func_type_idx).copied().unwrap_or(0) as usize;
        let num_locals = module_ref.num_locals.get(def_idx).copied().unwrap_or(0) as usize;

        call_stack[0] = Frame {
            func_idx: def_idx,
            pc: module_ref.code_offsets[def_idx],
            sp: 0,
            locals: [0i64; MAX_LOCALS],
        };

        // Set parameters as first locals, zero-initialize remaining locals
        for i in 0..num_params.min(MAX_LOCALS) {
            call_stack[0].locals[i] = if i < args.len() { args[i] } else { 0 };
        }
        for i in num_params.min(MAX_LOCALS)..num_locals.min(MAX_LOCALS) {
            call_stack[0].locals[i] = 0;
        }

        ctrl_stack[0] = CtrlFrame { kind: CTRL_FUNC, pc_cont: 0, sp: 0 };

        let mut cycle_count: u32 = 0;

        // ── Interpreter Loop ───────────────────────────────────────────
        loop {
            if cycle_count >= CYCLE_BUDGET {
                return Err("cycle budget exceeded");
            }
            cycle_count += 1;

            let pc = call_stack[fp].pc;
            let bc = &module_ref.bytecode[..module_ref.bc_len];
            // Consider pc may be past the end after a return/br
            if pc >= bc.len() {
                // If we're at or past the end, treat as end of function
                if fp == 0 { break; }
                // Pop frame
                let ret_val = if sp > call_stack[fp].sp { value_stack[sp - 1] } else { 0 };
                sp = call_stack[fp].sp;
                fp -= 1;
                if fp == 0 { value_stack[sp] = ret_val; sp += 1; break; }
                value_stack[sp] = ret_val;
                sp += 1;
                continue;
            }

            let opcode = bc[pc];
            let mut new_pc = pc + 1;

            match opcode {
                // ── Control Flow ────────────────────────────────────────
                0x00 => { return Err("unreachable"); }
                0x01 => {} // nop

                0x02 => { // block
                    if new_pc >= bc.len() { return Err("block: OOB"); }
                    let _block_type = bc[new_pc]; new_pc += 1;
                    let cont = find_matching_end(bc, pc)?;
                    if cp >= CALL_STACK_SIZE { return Err("ctrl stack overflow"); }
                    ctrl_stack[cp] = CtrlFrame { kind: CTRL_BLOCK, pc_cont: cont, sp };
                    cp += 1;
                    // new_pc points to start of block body
                }

                0x03 => { // loop
                    if new_pc >= bc.len() { return Err("loop: OOB"); }
                    let _block_type = bc[new_pc]; new_pc += 1;
                    let loop_start = new_pc;
                    // We need the end position for when loop is exited via br N where N
                    // targets an enclosing construct; but for br 0 targeting this loop,
                    // we jump to loop_start.
                    if cp >= CALL_STACK_SIZE { return Err("ctrl stack overflow"); }
                    ctrl_stack[cp] = CtrlFrame { kind: CTRL_LOOP, pc_cont: loop_start, sp };
                    cp += 1;
                }

                0x04 => { // if
                    if new_pc >= bc.len() { return Err("if: OOB"); }
                    let _block_type = bc[new_pc]; new_pc += 1;
                    // Pop condition
                    if sp == 0 { return Err("if: empty stack"); }
                    let cond = value_stack[sp - 1] as i32;
                    sp -= 1;

                    let (else_pc, end_pc) = find_if_else_end(bc, pc)?;
                    if cond != 0 {
                        // Execute if body
                        if cp >= CALL_STACK_SIZE { return Err("ctrl stack overflow"); }
                        ctrl_stack[cp] = CtrlFrame { kind: CTRL_IF, pc_cont: end_pc, sp };
                        cp += 1;
                        // new_pc already points to start of if body
                    } else {
                        // Skip to else (if present) or end
                        if else_pc > 0 {
                            // Skip if body, land at else body
                            if cp >= CALL_STACK_SIZE { return Err("ctrl stack overflow"); }
                            ctrl_stack[cp] = CtrlFrame { kind: CTRL_IF, pc_cont: end_pc, sp };
                            cp += 1;
                            new_pc = else_pc + 1; // skip `else` opcode
                        } else {
                            // No else, skip entirely to end
                            new_pc = end_pc;
                        }
                    }
                }

                0x05 => { // else
                    // We encountered `else` during execution of an if body.
                    // Skip to the matching end.
                    let target_pc = find_matching_end(bc, pc)?;
                    new_pc = target_pc;
                }

                0x0B => { // end
                    // End of current block/loop/if/function.
                    // Pop the control frame and determine continuation.
                    if cp == 0 {
                        // End of main function
                        break;
                    }
                    let entry = ctrl_stack[cp - 1];
                    cp -= 1;

                    match entry.kind {
                        CTRL_FUNC => { // end of function
                            break;
                        }
                        CTRL_LOOP => {
                            // End of loop body — execution continues after the loop.
                            // (br 0 at end of the body would jump back to loop start.)
                            // Nothing special to do.
                        }
                        CTRL_BLOCK | CTRL_IF => {
                            // End of block/if — execution continues after the block.
                            new_pc = entry.pc_cont;
                        }
                        _ => {}
                    }
                }

                0x0C => { // br (branch)
                    let label_idx = leb128_u(bc, &mut new_pc) as usize;
                    if label_idx >= cp { return Err("br: label out of range"); }
                    let target_idx = cp - 1 - label_idx;
                    let target = ctrl_stack[target_idx];

                    if target.kind == CTRL_LOOP {
                        // Branch to loop start
                        cp = target_idx + 1; // keep the loop's own frame
                        // sp = target.sp; // loop does NOT reset sp in WASM semantics
                        // Actually, for a loop, br to the loop start restarts execution.
                        // The operand stack is not reset — values from before the br remain.
                        new_pc = target.pc_cont;
                    } else {
                        // Branch to after the construct (exit block/if/func)
                        cp = target_idx; // pop down to and including target
                        // Reset stack to target's sp (discard values from inside block)
                        sp = target.sp;
                        new_pc = target.pc_cont;
                    }
                }

                0x0D => { // br_if (conditional branch)
                    let label_idx = leb128_u(bc, &mut new_pc) as usize;
                    if sp == 0 { return Err("br_if: empty stack"); }
                    let cond = value_stack[sp - 1] as i32;
                    sp -= 1; // pop condition

                    if cond != 0 {
                        if label_idx >= cp { return Err("br_if: label out of range"); }
                        let target_idx = cp - 1 - label_idx;
                        let target = ctrl_stack[target_idx];

                        if target.kind == CTRL_LOOP {
                            cp = target_idx + 1;
                            new_pc = target.pc_cont;
                        } else {
                            cp = target_idx;
                            sp = target.sp;
                            new_pc = target.pc_cont;
                        }
                    }
                }

                0x10 => { // call
                    let func_idx = leb128_u(bc, &mut new_pc) as usize;

                    if func_idx < num_imports {
                        // ── Host function call ──
                        let type_idx = module_ref.import_types[func_idx] as usize;
                        let nparams = module_ref.type_params.get(type_idx).copied().unwrap_or(0) as usize;
                        let nresults = module_ref.type_results.get(type_idx).copied().unwrap_or(0) as usize;

                        if sp < nparams { return Err("call: not enough args"); }

                        // Collect args from stack
                        let mut host_args = [0i64; MAX_LOCALS];
                        for i in 0..nparams.min(MAX_LOCALS) {
                            host_args[i] = value_stack[sp - nparams + i];
                        }
                        sp -= nparams;

                        // Look up host function by import name
                        let import_name = &module_ref.import_names[func_idx];
                        let mut found = false;
                        let mut result: i64 = 0;

                        for h in 0..HOST_FUNC_COUNT {
                            let entry = &HOST_FUNCS[h];
                            let mut matches = true;
                            for j in 0..32 {
                                if entry.name[j] != import_name[j] { matches = false; break; }
                                if entry.name[j] == 0 { break; }
                            }
                            if matches {
                                if let Some(f) = entry.func {
                                    result = f(&host_args[..nparams.min(MAX_LOCALS)]);
                                    found = true;
                                }
                                break;
                            }
                        }

                        if !found {
                            // V32: Check syscall table for automatic dispatch
                            let func_slice = crate::wasm::hostcall::name_to_slice(import_name);
                            if let Some(nr) = crate::wasm::hostcall::lookup_syscall_by_func(func_slice) {
                                result = crate::wasm::hostcall::dispatch_syscall(
                                    nr, &host_args[..nparams.min(MAX_LOCALS)], module_ref.pid,
                                );
                                found = true;
                                // Track syscall forward for perf stats
                                WASM_PERF_STATS[module_id].syscall_forward_count =
                                    WASM_PERF_STATS[module_id].syscall_forward_count.wrapping_add(1);
                            }
                        }

                        if !found { return Err("host function not found"); }

                        if nresults > 0 {
                            if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                            value_stack[sp] = result;
                            sp += 1;
                        }
                    } else {
                        // ── Defined function call ──
                        let def_call_idx = func_idx - num_imports;
                        if def_call_idx >= module_ref.num_functions {
                            return Err("call: func index OOB");
                        }
                        let ft_idx = module_ref.func_types[def_call_idx] as usize;
                        let nparams = module_ref.type_params.get(ft_idx).copied().unwrap_or(0) as usize;
                        let nresults = module_ref.type_results.get(ft_idx).copied().unwrap_or(0) as usize;

                        if sp < nparams { return Err("call: not enough args on stack"); }

                        // Save return address
                        call_stack[fp].pc = new_pc;

                        // Push new frame
                        fp += 1;
                        if fp >= CALL_STACK_SIZE { return Err("call stack overflow"); }

                        let nlocals = module_ref.num_locals.get(def_call_idx).copied().unwrap_or(0) as usize;
                        call_stack[fp] = Frame {
                            func_idx: def_call_idx,
                            pc: module_ref.code_offsets[def_call_idx],
                            sp: sp - nparams,
                            locals: [0i64; MAX_LOCALS],
                        };

                        // Copy parameters as first locals, zero-initialize rest
                        for i in 0..nparams.min(MAX_LOCALS) {
                            call_stack[fp].locals[i] = value_stack[sp - nparams + i];
                        }
                        for i in nparams.min(MAX_LOCALS)..nlocals.min(MAX_LOCALS) {
                            call_stack[fp].locals[i] = 0;
                        }
                        sp = sp - nparams;

                        // Push function control frame
                        if cp >= CALL_STACK_SIZE { return Err("ctrl stack overflow"); }
                        ctrl_stack[cp] = CtrlFrame { kind: CTRL_FUNC, pc_cont: 0, sp };
                        cp += 1;

                        new_pc = module_ref.code_offsets[def_call_idx];
                    }
                }

                0x20 => { // local.get
                    let idx = leb128_u(bc, &mut new_pc) as usize;
                    if idx >= MAX_LOCALS { return Err("local.get: index OOB"); }
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = call_stack[fp].locals[idx];
                    sp += 1;
                }

                0x21 => { // local.set
                    let idx = leb128_u(bc, &mut new_pc) as usize;
                    if idx >= MAX_LOCALS { return Err("local.set: index OOB"); }
                    if sp == 0 { return Err("local.set: empty stack"); }
                    sp -= 1;
                    call_stack[fp].locals[idx] = value_stack[sp];
                }

                0x22 => { // local.tee
                    let idx = leb128_u(bc, &mut new_pc) as usize;
                    if idx >= MAX_LOCALS { return Err("local.tee: index OOB"); }
                    if sp == 0 { return Err("local.tee: empty stack"); }
                    call_stack[fp].locals[idx] = value_stack[sp - 1];
                }

                // ── Constants ───────────────────────────────────────────
                0x41 => { // i32.const
                    let val = leb128_s(bc, &mut new_pc);
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val as i64;
                    sp += 1;
                }

                0x42 => { // i64.const
                    let val = leb128_s64(bc, &mut new_pc);
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val;
                    sp += 1;
                }

                // ── Type Conversion ──────────────────────────────────────
                0xA7 => { // i32.wrap_i64
                    if sp == 0 { return Err("i32.wrap_i64: empty stack"); }
                    value_stack[sp - 1] = (value_stack[sp - 1] as i32) as i64;
                }

                0xAC => { // i64.extend_i32_s
                    if sp == 0 { return Err("i64.extend_i32_s: empty stack"); }
                    value_stack[sp - 1] = (value_stack[sp - 1] as i32) as i64;
                }

                0xAD => { // i64.extend_i32_u
                    if sp == 0 { return Err("i64.extend_i32_u: empty stack"); }
                    value_stack[sp - 1] = (value_stack[sp - 1] as u32) as i64;
                }

                // ── Comparison ──────────────────────────────────────────
                0x45 => { // i32.eqz
                    if sp == 0 { return Err("i32.eqz: empty stack"); }
                    value_stack[sp - 1] = if value_stack[sp - 1] as i32 == 0 { 1 } else { 0 };
                }

                0x46 => { // i32.eq
                    if sp < 2 { return Err("i32.eq: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = if value_stack[sp] as i32 == value_stack[sp + 1] as i32 { 1 } else { 0 };
                    sp += 1;
                }

                0x47 => { // i32.ne
                    if sp < 2 { return Err("i32.ne: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = if value_stack[sp] as i32 != value_stack[sp + 1] as i32 { 1 } else { 0 };
                    sp += 1;
                }

                0x48 => { // i32.lt_s
                    if sp < 2 { return Err("i32.lt_s: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = if (value_stack[sp] as i32) < (value_stack[sp + 1] as i32) { 1 } else { 0 };
                    sp += 1;
                }

                0x49 => { // i32.lt_u
                    if sp < 2 { return Err("i32.lt_u: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = if (value_stack[sp] as u32) < (value_stack[sp + 1] as u32) { 1 } else { 0 };
                    sp += 1;
                }

                0x4A => { // i32.gt_s
                    if sp < 2 { return Err("i32.gt_s: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = if (value_stack[sp] as i32) > (value_stack[sp + 1] as i32) { 1 } else { 0 };
                    sp += 1;
                }

                0x4B => { // i32.gt_u
                    if sp < 2 { return Err("i32.gt_u: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = if (value_stack[sp] as u32) > (value_stack[sp + 1] as u32) { 1 } else { 0 };
                    sp += 1;
                }

                0x4C => { // i32.le_s
                    if sp < 2 { return Err("i32.le_s: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = if (value_stack[sp] as i32) <= (value_stack[sp + 1] as i32) { 1 } else { 0 };
                    sp += 1;
                }

                0x4D => { // i32.le_u
                    if sp < 2 { return Err("i32.le_u: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = if (value_stack[sp] as u32) <= (value_stack[sp + 1] as u32) { 1 } else { 0 };
                    sp += 1;
                }

                0x4E => { // i32.ge_s
                    if sp < 2 { return Err("i32.ge_s: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = if (value_stack[sp] as i32) >= (value_stack[sp + 1] as i32) { 1 } else { 0 };
                    sp += 1;
                }

                0x4F => { // i32.ge_u
                    if sp < 2 { return Err("i32.ge_u: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = if (value_stack[sp] as u32) >= (value_stack[sp + 1] as u32) { 1 } else { 0 };
                    sp += 1;
                }

                // ── Arithmetic ──────────────────────────────────────────
                0x6A => { // i32.add
                    if sp < 2 { return Err("i32.add: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = (value_stack[sp] as i32).wrapping_add(value_stack[sp + 1] as i32) as i64;
                    sp += 1;
                }

                0x6B => { // i32.sub
                    if sp < 2 { return Err("i32.sub: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = (value_stack[sp] as i32).wrapping_sub(value_stack[sp + 1] as i32) as i64;
                    sp += 1;
                }

                0x6C => { // i32.mul
                    if sp < 2 { return Err("i32.mul: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = (value_stack[sp] as i32).wrapping_mul(value_stack[sp + 1] as i32) as i64;
                    sp += 1;
                }

                0x6D => { // i32.div_s
                    if sp < 2 { return Err("i32.div_s: need 2 operands"); }
                    sp -= 2;
                    let rhs = value_stack[sp + 1] as i32;
                    if rhs == 0 { return Err("i32.div_s: divide by zero"); }
                    if value_stack[sp] as i32 == i32::MIN && rhs == -1 { return Err("i32.div_s: overflow"); }
                    value_stack[sp] = ((value_stack[sp] as i32) / rhs) as i64;
                    sp += 1;
                }

                0x6E => { // i32.div_u
                    if sp < 2 { return Err("i32.div_u: need 2 operands"); }
                    sp -= 2;
                    let rhs = value_stack[sp + 1] as u32;
                    if rhs == 0 { return Err("i32.div_u: divide by zero"); }
                    value_stack[sp] = ((value_stack[sp] as u32) / rhs) as i64;
                    sp += 1;
                }

                0x6F => { // i32.rem_s
                    if sp < 2 { return Err("i32.rem_s: need 2 operands"); }
                    sp -= 2;
                    let rhs = value_stack[sp + 1] as i32;
                    if rhs == 0 { return Err("i32.rem_s: divide by zero"); }
                    if value_stack[sp] as i32 == i32::MIN && rhs == -1 {
                        value_stack[sp] = 0;
                    } else {
                        value_stack[sp] = ((value_stack[sp] as i32) % rhs) as i64;
                    }
                    sp += 1;
                }

                0x70 => { // i32.rem_u
                    if sp < 2 { return Err("i32.rem_u: need 2 operands"); }
                    sp -= 2;
                    let rhs = value_stack[sp + 1] as u32;
                    if rhs == 0 { return Err("i32.rem_u: divide by zero"); }
                    value_stack[sp] = ((value_stack[sp] as u32) % rhs) as i64;
                    sp += 1;
                }

                0x71 => { // i32.and
                    if sp < 2 { return Err("i32.and: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = ((value_stack[sp] as i32) & (value_stack[sp + 1] as i32)) as i64;
                    sp += 1;
                }

                0x72 => { // i32.or
                    if sp < 2 { return Err("i32.or: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = ((value_stack[sp] as i32) | (value_stack[sp + 1] as i32)) as i64;
                    sp += 1;
                }

                0x73 => { // i32.xor
                    if sp < 2 { return Err("i32.xor: need 2 operands"); }
                    sp -= 2;
                    value_stack[sp] = ((value_stack[sp] as i32) ^ (value_stack[sp + 1] as i32)) as i64;
                    sp += 1;
                }

                0x74 => { // i32.shl
                    if sp < 2 { return Err("i32.shl: need 2 operands"); }
                    sp -= 2;
                    let shift = (value_stack[sp + 1] as u32) & 31;
                    value_stack[sp] = ((value_stack[sp] as u32) << shift) as i64;
                    sp += 1;
                }

                0x75 => { // i32.shr_s
                    if sp < 2 { return Err("i32.shr_s: need 2 operands"); }
                    sp -= 2;
                    let shift = (value_stack[sp + 1] as u32) & 31;
                    value_stack[sp] = ((value_stack[sp] as i32) >> shift) as i64;
                    sp += 1;
                }

                0x76 => { // i32.shr_u
                    if sp < 2 { return Err("i32.shr_u: need 2 operands"); }
                    sp -= 2;
                    let shift = (value_stack[sp + 1] as u32) & 31;
                    value_stack[sp] = ((value_stack[sp] as u32) >> shift) as i64;
                    sp += 1;
                }

                0x77 => { // i32.rotl
                    if sp < 2 { return Err("i32.rotl: need 2 operands"); }
                    sp -= 2;
                    let val = value_stack[sp] as u32;
                    let shift = (value_stack[sp + 1] as u32) & 31;
                    value_stack[sp] = val.rotate_left(shift) as i64;
                    sp += 1;
                }

                0x78 => { // i32.rotr
                    if sp < 2 { return Err("i32.rotr: need 2 operands"); }
                    sp -= 2;
                    let val = value_stack[sp] as u32;
                    let shift = (value_stack[sp + 1] as u32) & 31;
                    value_stack[sp] = val.rotate_right(shift) as i64;
                    sp += 1;
                }

                // ── Memory Load ─────────────────────────────────────────
                0x28 => { // i32.load
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp == 0 { return Err("i32.load: empty stack"); }
                    let addr = value_stack[sp - 1] as u32 as usize;
                    sp -= 1;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 4 > mem_size { return Err("i32.load: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i32.load: no memory"); }
                    let ptr = module_ref.mem_ptr as *const u8;
                    let val = unsafe {
                        (ptr.add(ea).read() as u32)
                        | ((ptr.add(ea + 1).read() as u32) << 8)
                        | ((ptr.add(ea + 2).read() as u32) << 16)
                        | ((ptr.add(ea + 3).read() as u32) << 24)
                    };
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val as i64;
                    sp += 1;
                }

                0x29 => { // i64.load
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp == 0 { return Err("i64.load: empty stack"); }
                    let addr = value_stack[sp - 1] as u32 as usize;
                    sp -= 1;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 8 > mem_size { return Err("i64.load: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i64.load: no memory"); }
                    let ptr = module_ref.mem_ptr as *const u8;
                    let val = unsafe {
                        (ptr.add(ea).read() as u64)
                        | ((ptr.add(ea + 1).read() as u64) << 8)
                        | ((ptr.add(ea + 2).read() as u64) << 16)
                        | ((ptr.add(ea + 3).read() as u64) << 24)
                        | ((ptr.add(ea + 4).read() as u64) << 32)
                        | ((ptr.add(ea + 5).read() as u64) << 40)
                        | ((ptr.add(ea + 6).read() as u64) << 48)
                        | ((ptr.add(ea + 7).read() as u64) << 56)
                    };
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val as i64;
                    sp += 1;
                }

                0x2C => { // i32.load8_s
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp == 0 { return Err("i32.load8_s: empty stack"); }
                    let addr = value_stack[sp - 1] as u32 as usize;
                    sp -= 1;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 1 > mem_size { return Err("i32.load8_s: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i32.load8_s: no memory"); }
                    let val = unsafe { (module_ref.mem_ptr as *const i8).add(ea).read() as i64 };
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val;
                    sp += 1;
                }

                0x2D => { // i32.load8_u
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp == 0 { return Err("i32.load8_u: empty stack"); }
                    let addr = value_stack[sp - 1] as u32 as usize;
                    sp -= 1;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 1 > mem_size { return Err("i32.load8_u: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i32.load8_u: no memory"); }
                    let val = unsafe { (module_ref.mem_ptr as *const u8).add(ea).read() as i64 };
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val;
                    sp += 1;
                }

                0x2E => { // i32.load16_s
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp == 0 { return Err("i32.load16_s: empty stack"); }
                    let addr = value_stack[sp - 1] as u32 as usize;
                    sp -= 1;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 2 > mem_size { return Err("i32.load16_s: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i32.load16_s: no memory"); }
                    let val = unsafe {
                        let p = module_ref.mem_ptr as *const u8;
                        ((p.add(ea).read() as i16) | ((p.add(ea + 1).read() as i16) << 8)) as i64
                    };
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val;
                    sp += 1;
                }

                0x2F => { // i32.load16_u
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp == 0 { return Err("i32.load16_u: empty stack"); }
                    let addr = value_stack[sp - 1] as u32 as usize;
                    sp -= 1;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 2 > mem_size { return Err("i32.load16_u: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i32.load16_u: no memory"); }
                    let val = unsafe {
                        let p = module_ref.mem_ptr as *const u8;
                        ((p.add(ea).read() as u16) | ((p.add(ea + 1).read() as u16) << 8)) as i64
                    };
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val;
                    sp += 1;
                }

                0x30 => { // i64.load8_s
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp == 0 { return Err("i64.load8_s: empty stack"); }
                    let addr = value_stack[sp - 1] as u32 as usize;
                    sp -= 1;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 1 > mem_size { return Err("i64.load8_s: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i64.load8_s: no memory"); }
                    let val = unsafe { (module_ref.mem_ptr as *const i8).add(ea).read() as i64 };
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val;
                    sp += 1;
                }

                0x31 => { // i64.load8_u
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp == 0 { return Err("i64.load8_u: empty stack"); }
                    let addr = value_stack[sp - 1] as u32 as usize;
                    sp -= 1;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 1 > mem_size { return Err("i64.load8_u: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i64.load8_u: no memory"); }
                    let val = unsafe { (module_ref.mem_ptr as *const u8).add(ea).read() as i64 };
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val;
                    sp += 1;
                }

                0x32 => { // i64.load16_s
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp == 0 { return Err("i64.load16_s: empty stack"); }
                    let addr = value_stack[sp - 1] as u32 as usize;
                    sp -= 1;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 2 > mem_size { return Err("i64.load16_s: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i64.load16_s: no memory"); }
                    let val = unsafe {
                        let p = module_ref.mem_ptr as *const u8;
                        ((p.add(ea).read() as i16) | ((p.add(ea + 1).read() as i16) << 8)) as i64
                    };
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val;
                    sp += 1;
                }

                0x33 => { // i64.load16_u
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp == 0 { return Err("i64.load16_u: empty stack"); }
                    let addr = value_stack[sp - 1] as u32 as usize;
                    sp -= 1;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 2 > mem_size { return Err("i64.load16_u: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i64.load16_u: no memory"); }
                    let val = unsafe {
                        let p = module_ref.mem_ptr as *const u8;
                        ((p.add(ea).read() as u16) | ((p.add(ea + 1).read() as u16) << 8)) as i64
                    };
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val;
                    sp += 1;
                }

                0x34 => { // i64.load32_s
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp == 0 { return Err("i64.load32_s: empty stack"); }
                    let addr = value_stack[sp - 1] as u32 as usize;
                    sp -= 1;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 4 > mem_size { return Err("i64.load32_s: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i64.load32_s: no memory"); }
                    let val = unsafe {
                        let p = module_ref.mem_ptr as *const u8;
                        let v = (p.add(ea).read() as u32)
                            | ((p.add(ea + 1).read() as u32) << 8)
                            | ((p.add(ea + 2).read() as u32) << 16)
                            | ((p.add(ea + 3).read() as u32) << 24);
                        v as i32 as i64
                    };
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val;
                    sp += 1;
                }

                0x35 => { // i64.load32_u
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp == 0 { return Err("i64.load32_u: empty stack"); }
                    let addr = value_stack[sp - 1] as u32 as usize;
                    sp -= 1;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 4 > mem_size { return Err("i64.load32_u: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i64.load32_u: no memory"); }
                    let val = unsafe {
                        let p = module_ref.mem_ptr as *const u8;
                        let v = (p.add(ea).read() as u32)
                            | ((p.add(ea + 1).read() as u32) << 8)
                            | ((p.add(ea + 2).read() as u32) << 16)
                            | ((p.add(ea + 3).read() as u32) << 24);
                        v as i64
                    };
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = val;
                    sp += 1;
                }

                // ── Memory Store ────────────────────────────────────────
                0x36 => { // i32.store
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp < 2 { return Err("i32.store: need 2 operands"); }
                    let addr = value_stack[sp - 2] as u32 as usize;
                    let val = value_stack[sp - 1] as u32;
                    sp -= 2;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 4 > mem_size { return Err("i32.store: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i32.store: no memory"); }
                    unsafe {
                        let p = module_ref.mem_ptr as *mut u8;
                        p.add(ea).write(val as u8);
                        p.add(ea + 1).write((val >> 8) as u8);
                        p.add(ea + 2).write((val >> 16) as u8);
                        p.add(ea + 3).write((val >> 24) as u8);
                    }
                }

                0x37 => { // i64.store
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp < 2 { return Err("i64.store: need 2 operands"); }
                    let addr = value_stack[sp - 2] as u32 as usize;
                    let val = value_stack[sp - 1] as u64;
                    sp -= 2;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 8 > mem_size { return Err("i64.store: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i64.store: no memory"); }
                    unsafe {
                        let p = module_ref.mem_ptr as *mut u8;
                        p.add(ea).write(val as u8);
                        p.add(ea + 1).write((val >> 8) as u8);
                        p.add(ea + 2).write((val >> 16) as u8);
                        p.add(ea + 3).write((val >> 24) as u8);
                        p.add(ea + 4).write((val >> 32) as u8);
                        p.add(ea + 5).write((val >> 40) as u8);
                        p.add(ea + 6).write((val >> 48) as u8);
                        p.add(ea + 7).write((val >> 56) as u8);
                    }
                }

                0x3A => { // i32.store8
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp < 2 { return Err("i32.store8: need 2 operands"); }
                    let addr = value_stack[sp - 2] as u32 as usize;
                    let val = value_stack[sp - 1] as u8;
                    sp -= 2;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 1 > mem_size { return Err("i32.store8: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i32.store8: no memory"); }
                    unsafe { (module_ref.mem_ptr as *mut u8).add(ea).write(val); }
                }

                0x3B => { // i32.store16
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp < 2 { return Err("i32.store16: need 2 operands"); }
                    let addr = value_stack[sp - 2] as u32 as usize;
                    let val = value_stack[sp - 1] as u16;
                    sp -= 2;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 2 > mem_size { return Err("i32.store16: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i32.store16: no memory"); }
                    unsafe {
                        let p = module_ref.mem_ptr as *mut u8;
                        p.add(ea).write(val as u8);
                        p.add(ea + 1).write((val >> 8) as u8);
                    }
                }

                0x3C => { // i64.store8
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp < 2 { return Err("i64.store8: need 2 operands"); }
                    let addr = value_stack[sp - 2] as u32 as usize;
                    let val = value_stack[sp - 1] as u8;
                    sp -= 2;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 1 > mem_size { return Err("i64.store8: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i64.store8: no memory"); }
                    unsafe { (module_ref.mem_ptr as *mut u8).add(ea).write(val); }
                }

                0x3D => { // i64.store16
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp < 2 { return Err("i64.store16: need 2 operands"); }
                    let addr = value_stack[sp - 2] as u32 as usize;
                    let val = value_stack[sp - 1] as u16;
                    sp -= 2;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 2 > mem_size { return Err("i64.store16: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i64.store16: no memory"); }
                    unsafe {
                        let p = module_ref.mem_ptr as *mut u8;
                        p.add(ea).write(val as u8);
                        p.add(ea + 1).write((val >> 8) as u8);
                    }
                }

                0x3E => { // i64.store32
                    let _align = leb128_u(bc, &mut new_pc);
                    let offset = leb128_u(bc, &mut new_pc) as usize;
                    if sp < 2 { return Err("i64.store32: need 2 operands"); }
                    let addr = value_stack[sp - 2] as u32 as usize;
                    let val = value_stack[sp - 1] as u32;
                    sp -= 2;
                    let ea = addr.wrapping_add(offset);
                    let mem_size = module_ref.mem_pages * MEM_PAGE_SIZE;
                    if ea + 4 > mem_size { return Err("i64.store32: OOB"); }
                    if module_ref.mem_ptr == 0 { return Err("i64.store32: no memory"); }
                    unsafe {
                        let p = module_ref.mem_ptr as *mut u8;
                        p.add(ea).write(val as u8);
                        p.add(ea + 1).write((val >> 8) as u8);
                        p.add(ea + 2).write((val >> 16) as u8);
                        p.add(ea + 3).write((val >> 24) as u8);
                    }
                }

                // ── Memory Management ───────────────────────────────────
                0x3F => { // memory.size
                    new_pc += 1; // skip 0x00 reserve byte
                    if sp >= VALUE_STACK_SIZE { return Err("value stack overflow"); }
                    value_stack[sp] = module_ref.mem_pages as i64;
                    sp += 1;
                }

                0x40 => { // memory.grow
                    new_pc += 1; // skip 0x00 reserve byte
                    if sp == 0 { return Err("memory.grow: empty stack"); }
                    let request = value_stack[sp - 1] as usize;
                    sp -= 1;

                    let current_pages = module_ref.mem_pages;
                    let max_pages = module_ref.mem_max_pages;
                    if request == 0 {
                        value_stack[sp] = current_pages as i64;
                        sp += 1;
                    } else if current_pages + request <= max_pages {
                        // Grow memory: allocate new, copy old, free old
                        let new_pages = current_pages + request;
                        let new_size = new_pages * MEM_PAGE_SIZE;
                        let new_layout = match Layout::from_size_align(new_size, 16) {
                            Ok(l) => l,
                            Err(_) => { value_stack[sp] = -1i64; sp += 1; continue; }
                        };
                        let new_mem = unsafe { alloc_zeroed(new_layout) };
                        if new_mem.is_null() {
                            value_stack[sp] = -1i64;
                            sp += 1;
                        } else {
                            // Copy old content
                            if module_ref.mem_ptr != 0 {
                                let old_size = current_pages * MEM_PAGE_SIZE;
                                unsafe {
                                    core::ptr::copy_nonoverlapping(
                                        module_ref.mem_ptr as *const u8,
                                        new_mem,
                                        old_size,
                                    );
                                }
                            }
                            // Free old memory
                            if module_ref.mem_ptr != 0 {
                                let old_layout = match Layout::from_size_align(current_pages * MEM_PAGE_SIZE, 16) {
                                    Ok(l) => l,
                                    Err(_) => {
                                        // Can't free properly, leak it
                                        value_stack[sp] = current_pages as i64;
                                        sp += 1;
                                        // Update module memory pointer
                                        // Need mutable access to module
                                        // ... handle this
                                        continue;
                                    }
                                };
                                unsafe { dealloc(module_ref.mem_ptr as *mut u8, old_layout); }
                            }
                            // Update module state
                            // Since module_ref is immutable, we need to work around it
                            // We use raw pointer access
                            let mod_ptr = &WASM_MODULES[module_id] as *const WasmModule as *mut WasmModule;
                            unsafe {
                                (*mod_ptr).mem_ptr = new_mem as usize;
                                (*mod_ptr).mem_pages = new_pages;
                            }
                            value_stack[sp] = current_pages as i64;
                            sp += 1;
                        }
                    } else {
                        value_stack[sp] = -1i64; // grow failed
                        sp += 1;
                    }
                }

                // ── Select ─────────────────────────────────────────────
                0x1B => { // select (conditional select)
                    if sp < 3 { return Err("select: need 3 operands"); }
                    let cond = value_stack[sp - 1] as i32;
                    sp -= 1;
                    let val2 = value_stack[sp - 1];
                    sp -= 1;
                    let val1 = value_stack[sp - 1];
                    // sp now points to val1 position
                    value_stack[sp] = if cond != 0 { val1 } else { val2 };
                    sp += 1;
                }

                // ── Return ─────────────────────────────────────────────
                0x0F => { // return (opcode 0x0F is return in some encodings, but WASM uses 0x05)
                    // Already handled - but some tools emit 0x0F for return
                    // Actually in WASM, return is opcode 0x0F (multi-byte) or 0x05?
                    // Standard WASM: return = 0x0F
                    let ret_val = if sp > call_stack[fp].sp { value_stack[sp - 1] } else { 0 };
                    // Pop to function level
                    while cp > 0 && ctrl_stack[cp - 1].kind != CTRL_FUNC {
                        cp -= 1;
                    }
                    if cp > 0 { cp -= 1; } // pop func frame
                    if fp == 0 { return Err("return from main function"); }
                    let saved_sp = call_stack[fp].sp;
                    fp -= 1;
                    sp = saved_sp;
                    value_stack[sp] = ret_val;
                    sp += 1;
                    new_pc = call_stack[fp].pc; // resume caller
                }

                _ => {
                    return Err("unknown opcode");
                }
            }

            // Update PC in current frame
            call_stack[fp].pc = new_pc;
        }

        // V32: Record performance statistics
        WASM_PERF_STATS[module_id].record_invocation(cycle_count as u64);

        // Return the top of the value stack (or 0 if empty)
        let result = if sp > 0 { value_stack[sp - 1] as i32 } else { 0 };
        Ok(result)
    }
}

/// Convenience: call an exported function without arguments.
pub fn wasm_call_export(module_id: usize, name: &str) -> Result<i32, &'static str> {
    wasm_execute(module_id, name, &[])
}

// ══════════════════════════════════════════════════════════════════════════
//  Debug / Introspection
// ══════════════════════════════════════════════════════════════════════════

/// Return a summary of modules for debug printing.
pub fn wasm_debug_info() -> core::fmt::Result {
    unsafe {
        for i in 0..WASM_COUNT {
            if WASM_MODULES[i].loaded {
                let name_slice = &WASM_MODULES[i].name;
                let name_str = core::str::from_utf8(name_slice).unwrap_or("?");
                crate::println!("  WASM[{}]: name={} pid={} funcs={} exports={} mem={}KB",
                    i, name_str, WASM_MODULES[i].pid,
                    WASM_MODULES[i].num_functions, WASM_MODULES[i].num_exports,
                    WASM_MODULES[i].mem_pages * MEM_PAGE_SIZE / 1024);
            }
        }
    }
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════
//  V32: Module Status & Hot Reload
// ══════════════════════════════════════════════════════════════════════════

/// Check whether a WASM module is currently loaded.
pub fn is_module_loaded(module_id: usize) -> bool {
    unsafe {
        module_id < MAX_WASM_MODULES && WASM_MODULES[module_id].loaded
    }
}

/// Hot-reload a WASM module: replace bytecode without restarting services.
///
/// 1. Validates the new bytecode's magic/version header.
/// 2. Resets parse state (types, imports, functions, exports).
/// 3. Copies in new bytecode and re-parses all sections.
/// 4. Preserves existing linear memory allocation.
/// 5. Resets performance statistics.
///
/// Returns Ok(()) on success, or an error string on failure.
pub fn wasm_hot_reload(module_id: usize, new_bytecode: &[u8]) -> Result<(), &'static str> {
    unsafe {
        if module_id >= MAX_WASM_MODULES || !WASM_MODULES[module_id].loaded {
            return Err("invalid module id");
        }
        if !wasm_validate(new_bytecode) {
            return Err("invalid magic/version in new bytecode");
        }
        if new_bytecode.len() > MAX_WASM_SIZE {
            return Err("new bytecode exceeds maximum size");
        }

        // Save old counters for debug output
        let old_funcs = WASM_MODULES[module_id].num_functions;
        let old_exports = WASM_MODULES[module_id].num_exports;

        // Reset parse state (preserve memory allocation, name, pid)
        WASM_MODULES[module_id].num_types = 0;
        WASM_MODULES[module_id].type_params = [0; MAX_TYPES];
        WASM_MODULES[module_id].type_results = [0; MAX_TYPES];
        WASM_MODULES[module_id].num_imports = 0;
        WASM_MODULES[module_id].import_names = [[0; 32]; MAX_IMPORTS];
        WASM_MODULES[module_id].import_types = [0; MAX_IMPORTS];
        WASM_MODULES[module_id].num_functions = 0;
        WASM_MODULES[module_id].func_types = [0; MAX_FUNCTIONS];
        WASM_MODULES[module_id].code_offsets = [0; MAX_FUNCTIONS];
        WASM_MODULES[module_id].code_sizes = [0; MAX_FUNCTIONS];
        WASM_MODULES[module_id].num_locals = [0; MAX_FUNCTIONS];
        WASM_MODULES[module_id].num_exports = 0;
        WASM_MODULES[module_id].export_names = [[0; 32]; MAX_EXPORTS];
        WASM_MODULES[module_id].export_funcs = [0; MAX_EXPORTS];

        // Copy new bytecode
        let len = new_bytecode.len().min(MAX_WASM_SIZE);
        for i in 0..len {
            WASM_MODULES[module_id].bytecode[i] = new_bytecode[i];
        }
        WASM_MODULES[module_id].bc_len = len;

        // Re-parse sections
        parse_module(module_id);

        crate::println!("  WASM: hot-reloaded module {} (funcs: {} -> {}, exports: {} -> {})",
            module_id, old_funcs, WASM_MODULES[module_id].num_functions,
            old_exports, WASM_MODULES[module_id].num_exports);

        // Reset performance counters
        WASM_PERF_STATS[module_id].reset();

        Ok(())
    }
}

/// Get performance statistics for a loaded WASM module.
pub fn wasm_perf_stats(module_id: usize) -> Option<WasmPerfStats> {
    unsafe {
        if module_id >= MAX_WASM_MODULES || !WASM_MODULES[module_id].loaded {
            return None;
        }
        Some(WASM_PERF_STATS[module_id])
    }
}

/// Reset performance counters for a WASM module.
pub fn wasm_reset_perf_stats(module_id: usize) -> bool {
    unsafe {
        if module_id >= MAX_WASM_MODULES || !WASM_MODULES[module_id].loaded {
            return false;
        }
        WASM_PERF_STATS[module_id].reset();
        true
    }
}
