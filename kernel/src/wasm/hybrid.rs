// V32: eBPF + WASM Hybrid Architecture
//
// Combines V24's eBPF-style kernel extensions with V28's WASM runtime:
//   - eBPF handles fast-path decisions (1-2 cycle overhead)
//   - WASM handles complex policy logic (full interpreter)
//
// Convention (shared with extension/mod.rs scratch layout):
//   eBPF scratch[16]     = delegate_to_wasm flag (1 = invoke WASM)
//   eBPF scratch[20..24] = wasm_module_id to invoke (little-endian u32)
//   eBPF scratch[24]     = wasm_action (0=allow, 1=deny, 2=log)

use crate::wasm;

// ── Scratch Layout Constants ──────────────────────────────────────────────

const SCRATCH_DELEGATE: usize = 16;
const SCRATCH_WASM_ID: usize = 20;
const SCRATCH_WASM_ACTION: usize = 24;

// ── Hybrid Policy Entry ───────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct HybridPolicyEntry {
    ebpf_ext_id: usize,
    wasm_module_id: usize,
    hook_type: u8,
    active: bool,
    name: [u8; 32],
}

impl HybridPolicyEntry {
    const fn empty() -> Self {
        HybridPolicyEntry {
            ebpf_ext_id: 0,
            wasm_module_id: 0,
            hook_type: 0,
            active: false,
            name: [0u8; 32],
        }
    }
}

// ── Public Context Structure ──────────────────────────────────────────────

pub struct HybridContext {
    pub hook_type: u8,
    pub syscall_nr: usize,
    pub pid: u32,
    pub args: [usize; 6],
}

// ── Global State ──────────────────────────────────────────────────────────

const MAX_HYBRID_POLICIES: usize = 8;

static mut HYBRID_POLICIES: [HybridPolicyEntry; MAX_HYBRID_POLICIES] = [
    HybridPolicyEntry::empty(); MAX_HYBRID_POLICIES
];
static mut HYBRID_COUNT: usize = 0;

// ── Public API ────────────────────────────────────────────────────────────

pub fn init_hybrid() {
    crate::println!("  V32: eBPF+WASM hybrid engine initialized (max {} policies)",
        MAX_HYBRID_POLICIES);
}

pub fn register_policy(
    name: &[u8],
    ebpf_ext_id: usize,
    wasm_module_id: usize,
    hook_type: u8,
) -> Option<usize> {
    unsafe {
        if HYBRID_COUNT >= MAX_HYBRID_POLICIES {
            return None;
        }
        if !wasm::is_module_loaded(wasm_module_id) {
            crate::println!("  HYBRID: WASM module {} not loaded", wasm_module_id);
            return None;
        }
        let idx = HYBRID_COUNT;
        HYBRID_POLICIES[idx].ebpf_ext_id = ebpf_ext_id;
        HYBRID_POLICIES[idx].wasm_module_id = wasm_module_id;
        HYBRID_POLICIES[idx].hook_type = hook_type;
        HYBRID_POLICIES[idx].active = true;
        let nlen = name.len().min(31);
        for i in 0..nlen {
            HYBRID_POLICIES[idx].name[i] = name[i];
        }
        HYBRID_POLICIES[idx].name[nlen] = 0;
        HYBRID_COUNT += 1;
        Some(idx)
    }
}

pub fn unregister_policy(idx: usize) -> bool {
    unsafe {
        if idx < HYBRID_COUNT && HYBRID_POLICIES[idx].active {
            HYBRID_POLICIES[idx].active = false;
            return true;
        }
    }
    false
}

pub fn run_hybrid_hook(ctx: &HybridContext) -> i32 {
    unsafe {
        for i in 0..HYBRID_COUNT {
            if !HYBRID_POLICIES[i].active { continue; }
            if HYBRID_POLICIES[i].hook_type != ctx.hook_type { continue; }

            let ebpf_id = HYBRID_POLICIES[i].ebpf_ext_id;
            let wasm_id = HYBRID_POLICIES[i].wasm_module_id;

            let context0: u64 = ((ctx.hook_type as u64) << 56)
                | (ctx.syscall_nr as u64 & 0x00FF_FFFF_FFFF_FFFF);
            let context1: u64 = (ctx.pid as u64) | ((ctx.args[0] as u64) << 32);

            let ebpf_ok = crate::extension::execute_single(
                ebpf_id, ctx.hook_type, context0, context1);

            if ebpf_ok {
                let delegate = crate::extension::read_scratch(ebpf_id, SCRATCH_DELEGATE);
                if delegate == 1 {
                    let wasm_mod_id = crate::extension::read_scratch_u32(
                        ebpf_id, SCRATCH_WASM_ID) as usize;
                    let target_wasm_id = if wasm_mod_id != 0 { wasm_mod_id } else { wasm_id };

                    if wasm::is_module_loaded(target_wasm_id) {
                        let wasm_args = [
                            ctx.syscall_nr as i64,
                            ctx.pid as i64,
                            ctx.args[0] as i64,
                            ctx.args[1] as i64,
                            ctx.args[2] as i64,
                            ctx.args[3] as i64,
                        ];
                        let result = wasm::wasm_execute(
                            target_wasm_id, "_policy_evaluate", &wasm_args);
                        match result {
                            Ok(0) => {}
                            Ok(1) => {
                                crate::println!("  HYBRID[{}]: DENY hook={} nr={} pid={}",
                                    i, ctx.hook_type, ctx.syscall_nr, ctx.pid);
                                return -1;
                            }
                            Ok(2) => {
                                crate::println!("  HYBRID[{}]: LOG hook={} nr={} pid={}",
                                    i, ctx.hook_type, ctx.syscall_nr, ctx.pid);
                            }
                            Ok(v) => {
                                crate::println!("  HYBRID[{}]: action={} hook={} nr={} pid={}",
                                    i, v, ctx.hook_type, ctx.syscall_nr, ctx.pid);
                            }
                            Err(e) => {
                                crate::println!("  HYBRID[{}]: WASM error: {} (disabling)",
                                    i, e);
                                HYBRID_POLICIES[i].active = false;
                            }
                        }
                    } else {
                        crate::println!("  HYBRID[{}]: WASM module {} not loaded (disabling)",
                            i, target_wasm_id);
                        HYBRID_POLICIES[i].active = false;
                    }
                }
            } else {
                HYBRID_POLICIES[i].active = false;
            }
        }
    }
    0
}

pub fn list_policies(buf: &mut [u8]) -> usize {
    unsafe {
        let mut pos = 0;
        for i in 0..HYBRID_COUNT {
            if pos + 42 > buf.len() { break; }
            for j in 0..32 { buf[pos + j] = HYBRID_POLICIES[i].name[j]; }
            pos += 32;
            let eid = HYBRID_POLICIES[i].ebpf_ext_id as u32;
            buf[pos..pos + 4].copy_from_slice(&eid.to_le_bytes());
            pos += 4;
            let wid = HYBRID_POLICIES[i].wasm_module_id as u32;
            buf[pos..pos + 4].copy_from_slice(&wid.to_le_bytes());
            pos += 4;
            buf[pos] = HYBRID_POLICIES[i].hook_type;
            pos += 1;
            buf[pos] = if HYBRID_POLICIES[i].active { 1 } else { 0 };
            pos += 1;
        }
        pos
    }
}
