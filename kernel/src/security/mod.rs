// V21 Security Subsystem — Formal verification primitives & hardening
//
// Features:
//   - W^X page table enforcement (no page is both writable AND executable)
//   - Per-process seccomp-style syscall filter
//   - Capability audit logging
//   - Kernel stack canary verification
//   - Heap overflow detection via guard pages

use crate::mem::{sv39, layout::PAGE_SIZE};

// ── W^X Enforcement ─────────────────────────────────────────────────────────

/// Verify that no page table entry has both W and X bits set.
/// Called periodically and on each page table modification.
pub fn verify_wxorx(root_phys: usize) -> Result<(), &'static str> {
    unsafe {
        let l2 = &*(sv39::pa_to_kva(root_phys) as *const [sv39::PTE; 512]);
        for vpn2 in 0..256 {
            let l2e = l2[vpn2];
            if !l2e.is_valid() || l2e.is_leaf() { continue; }
            let l1 = &*(sv39::pa_to_kva(l2e.phys_addr()) as *const [sv39::PTE; 512]);
            for vpn1 in 0..512 {
                let l1e = l1[vpn1];
                if !l1e.is_valid() { continue; }
                if l1e.is_leaf() {
                    if l1e.is_writable() && l1e.is_executable() { return Err("W^X violation at L1"); }
                    continue;
                }
                let l0 = &*(sv39::pa_to_kva(l1e.phys_addr()) as *const [sv39::PTE; 512]);
                for vpn0 in 0..512 {
                    let l0e = l0[vpn0];
                    if l0e.is_valid() && l0e.is_writable() && l0e.is_executable() {
                        crate::println!("W^X violation: vpn2={} vpn1={} vpn0={}", vpn2, vpn1, vpn0);
                        return Err("W^X violation at L0");
                    }
                }
            }
        }
    }
    Ok(())
}

/// Enforce W^X on a newly mapped page: if W is set, clear X.
pub fn enforce_wxorx_pte(pte: &mut sv39::PTE) {
    if pte.is_writable() && pte.is_executable() {
        // Clear executable bit (prioritize data over code)
        pte.set_flags(pte.is_readable(), true, false, pte.is_user());
    }
}

// ── Seccomp Filter ──────────────────────────────────────────────────────────

const SECCOMP_MAX_RULES: usize = 16;
const SECCOMP_MAX_PROCS: usize = 32;

#[derive(Clone, Copy)]
struct SeccompRule {
    syscall_nr: usize,
    action: SeccompAction,
}

#[derive(Clone, Copy, PartialEq)]
enum SeccompAction {
    Allow = 0,
    Kill = 1,
    Log = 2,
}

static mut SECCOMP_FILTERS: [([SeccompRule; SECCOMP_MAX_RULES], usize, u32); SECCOMP_MAX_PROCS] = [
    ([SeccompRule { syscall_nr: 0, action: SeccompAction::Allow }; SECCOMP_MAX_RULES], 0, 0); SECCOMP_MAX_PROCS
];
static mut SECCOMP_COUNT: usize = 0;

/// Set up a seccomp filter for a process. Returns 0 on success.
pub fn seccomp_add_rule(pid: u32, syscall_nr: usize, action: u8) -> Result<(), &'static str> {
    let act = match action {
        0 => SeccompAction::Allow,
        1 => SeccompAction::Kill,
        2 => SeccompAction::Log,
        _ => return Err("invalid action"),
    };
    unsafe {
        for i in 0..SECCOMP_COUNT {
            if SECCOMP_FILTERS[i].2 == pid {
                if SECCOMP_FILTERS[i].1 >= SECCOMP_MAX_RULES { return Err("rules full"); }
                SECCOMP_FILTERS[i].0[SECCOMP_FILTERS[i].1] = SeccompRule { syscall_nr, action: act };
                SECCOMP_FILTERS[i].1 += 1;
                return Ok(());
            }
        }
        if SECCOMP_COUNT >= SECCOMP_MAX_PROCS { return Err("seccomp table full"); }
        SECCOMP_FILTERS[SECCOMP_COUNT].2 = pid;
        SECCOMP_FILTERS[SECCOMP_COUNT].0[0] = SeccompRule { syscall_nr, action: act };
        SECCOMP_FILTERS[SECCOMP_COUNT].1 = 1;
        SECCOMP_COUNT += 1;
    }
    Ok(())
}

/// Check if a syscall is allowed for the given process.
/// Returns (allowed, should_kill).
pub fn seccomp_check(pid: u32, syscall_nr: usize) -> (bool, bool) {
    unsafe {
        for i in 0..SECCOMP_COUNT {
            if SECCOMP_FILTERS[i].2 != pid { continue; }
            for j in 0..SECCOMP_FILTERS[i].1 {
                if SECCOMP_FILTERS[i].0[j].syscall_nr == syscall_nr {
                    match SECCOMP_FILTERS[i].0[j].action {
                        SeccompAction::Allow => return (true, false),
                        SeccompAction::Kill => return (false, true),
                        SeccompAction::Log => {
                            crate::println!("seccomp: pid={} syscall={}", pid, syscall_nr);
                            return (true, false);
                        }
                    }
                }
            }
        }
    }
    (true, false) // default allow if no rules match
}

// ── Capability Audit ────────────────────────────────────────────────────────

static mut CAP_AUDIT_LOG: [(u32, u32, usize, u64); 256] = [(0, 0, 0, 0); 256]; // (pid, operation, slot, timestamp)
static mut CAP_AUDIT_IDX: usize = 0;

/// Log a capability operation for audit purposes.
pub fn cap_audit_log(pid: u32, operation: u32, slot: usize) {
    unsafe {
        let ts = crate::trap::TICK_COUNT as u64;
        CAP_AUDIT_LOG[CAP_AUDIT_IDX % 256] = (pid, operation, slot, ts);
        CAP_AUDIT_IDX += 1;
    }
}

/// Read the capability audit log into a buffer. Returns bytes written.
/// Format: [pid:4][op:4][slot:8] per entry (16 bytes each)
pub fn cap_audit_read(buf: &mut [u8]) -> usize {
    unsafe {
        let count = CAP_AUDIT_IDX.min(256);
        let mut pos = 0;
        for i in 0..count {
            if pos + 24 > buf.len() { break; }
            let (pid, op, slot, ts) = CAP_AUDIT_LOG[i];
            // pid (4 bytes, u32)
            buf[pos] = pid as u8; buf[pos+1] = (pid>>8) as u8;
            buf[pos+2] = (pid>>16) as u8; buf[pos+3] = (pid>>24) as u8;
            // op (4 bytes, u32)
            buf[pos+4] = op as u8; buf[pos+5] = (op>>8) as u8;
            buf[pos+6] = (op>>16) as u8; buf[pos+7] = (op>>24) as u8;
            // slot (8 bytes, usize)
            buf[pos+8] = slot as u8; buf[pos+9] = (slot>>8) as u8;
            buf[pos+10] = (slot>>16) as u8; buf[pos+11] = (slot>>24) as u8;
            buf[pos+12] = (slot>>32) as u8; buf[pos+13] = (slot>>40) as u8;
            buf[pos+14] = (slot>>48) as u8; buf[pos+15] = (slot>>56) as u8;
            // ts (8 bytes, u64)
            buf[pos+16] = ts as u8; buf[pos+17] = (ts>>8) as u8;
            buf[pos+18] = (ts>>16) as u8; buf[pos+19] = (ts>>24) as u8;
            buf[pos+20] = (ts>>32) as u8; buf[pos+21] = (ts>>40) as u8;
            buf[pos+22] = (ts>>48) as u8; buf[pos+23] = (ts>>56) as u8;
            pos += 24;
        }
        pos
    }
}

/// Format the audit log as human-readable text into a buffer.
/// Each line: "ts pid op slot\n"
pub fn cap_audit_dump(buf: &mut [u8]) -> usize {
    unsafe {
        let count = CAP_AUDIT_IDX.min(256);
        let mut pos = 0usize;
        for i in 0..count {
            let (pid, op, slot, ts) = CAP_AUDIT_LOG[i];
            if pid == 0 && op == 0 { continue; }
            // Format: "ts pid op slot\n"
            pos += write_u64(buf, pos, ts);
            if pos >= buf.len() { break; }
            buf[pos] = b' '; pos += 1;
            pos += write_u32(buf, pos, pid);
            if pos >= buf.len() { break; }
            buf[pos] = b' '; pos += 1;
            pos += write_u32(buf, pos, op);
            if pos >= buf.len() { break; }
            buf[pos] = b' '; pos += 1;
            pos += write_usize(buf, pos, slot);
            if pos >= buf.len() { break; }
            buf[pos] = b'\n'; pos += 1;
            if pos + 50 > buf.len() { break; }
        }
        pos
    }
}

// Simple digit-by-digit integer to ASCII for no_std kernel.
fn write_u64(buf: &mut [u8], pos: usize, v: u64) -> usize {
    if v == 0 {
        if pos < buf.len() { buf[pos] = b'0'; return 1; }
        return 0;
    }
    let mut temp = [0u8; 20];
    let mut n = v;
    let mut len = 0;
    while n > 0 {
        temp[len] = b'0' + (n % 10) as u8;
        n /= 10;
        len += 1;
    }
    let mut written = 0;
    for i in (0..len).rev() {
        if pos + written < buf.len() {
            buf[pos + written] = temp[i];
            written += 1;
        } else {
            break;
        }
    }
    written
}

fn write_u32(buf: &mut [u8], pos: usize, v: u32) -> usize {
    write_u64(buf, pos, v as u64)
}

fn write_usize(buf: &mut [u8], pos: usize, v: usize) -> usize {
    write_u64(buf, pos, v as u64)
}

// ── Kernel Stack Canary ─────────────────────────────────────────────────────

static KERNEL_STACK_CANARY: u64 = 0xDEADBEEF_CAFEBABE;

/// Initialize the kernel stack canary for the current HART.
pub fn init_stack_canary() {
    // Read current sp and write canary at the bottom of the current 64KB stack
    let sp: usize;
    unsafe { core::arch::asm!("mv {}, sp", out(reg) sp); }
    let stack_bottom = sp & !0xFFFF; // 64KB aligned
    unsafe {
        (stack_bottom as *mut u64).write_volatile(KERNEL_STACK_CANARY);
    }
}

/// Verify the kernel stack canary. Panics if corrupted.
pub fn check_stack_canary() {
    let sp: usize;
    unsafe { core::arch::asm!("mv {}, sp", out(reg) sp); }
    let stack_bottom = sp & !0xFFFF;
    let canary = unsafe { (stack_bottom as *const u64).read_volatile() };
    if canary != KERNEL_STACK_CANARY {
        crate::println!("STACK SMASHING DETECTED! canary=0x{:x} sp=0x{:x}", canary, sp);
        crate::idle_loop();
    }
}

// ── Guard Page Protection ───────────────────────────────────────────────────

/// Mark a page as a guard page (no access, triggers fault on access).
pub fn set_guard_page(root_phys: usize, va: usize) -> Result<(), &'static str> {
    unsafe {
        // Map the page with no R/W/X permissions
        // This requires the page to already be mapped; we clear its flags
        if let Some((l0_phys, idx)) = crate::proc::elf::walk_pt(root_phys, va, false) {
            let l0 = &mut *(sv39::pa_to_kva(l0_phys) as *mut [sv39::PTE; 512]);
            l0[idx] = sv39::PTE::empty(); // Clear PTE — no access
            Ok(())
        } else {
            Err("guard page not mapped")
        }
    }
}
